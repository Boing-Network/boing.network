/**
 * Request-driven reference multi-token (game items) touch index. No cron: each items read first runs a
 * bounded catch-up (throttled by `mt_cursor.updated_at`), so the index advances only when someone asks.
 *
 * Only direct `ContractCall`s to a contract whose installed code is the pinned multi-token runtime are
 * indexed (logs carry no emitter address, so nested-call logs could be spoofed). Balances are not stored;
 * readers fetch them live from storage (`balance[item][account]` slot).
 */

import {
  createClient,
  decodeMultiTokenCalldata,
  decodeMultiTokenTransferLog,
  fetchBlocksWithReceiptsForHeightRange,
  fetchMultiTokenBalance,
  isReferenceMultiTokenBytecode,
  type BoingClient,
} from '@boing-network/boing-sdk';
import { normalizeHex64 } from './cors.js';

export type ItemTouch = {
  contractHex: string;
  itemId: string;
  accountHex: string;
  blockHeight: number;
  txId: string;
};

function contractCallBody(payload: unknown): Record<string, unknown> | null {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) return null;
  const p = payload as Record<string, unknown>;
  const keys = Object.keys(p);
  if (keys.length === 1 && keys[0]!.toLowerCase() === 'contractcall') {
    const v = p[keys[0]!];
    return v && typeof v === 'object' ? (v as Record<string, unknown>) : null;
  }
  return typeof p.contract === 'string' ? p : null;
}

function calldataHex(raw: unknown): string | null {
  if (typeof raw === 'string') return raw.trim() ? raw.trim() : null;
  if (Array.isArray(raw) && raw.every((b) => Number.isInteger(b) && b >= 0 && b <= 255)) {
    return '0x' + (raw as number[]).map((b) => b.toString(16).padStart(2, '0')).join('');
  }
  return null;
}

const ZERO = '0x' + '00'.repeat(32);

/** Pure: candidate item touches in one block (contracts still need the runtime-bytes check). */
export function extractItemTouchesFromBlock(block: unknown, height: number): ItemTouch[] {
  if (!block || typeof block !== 'object') return [];
  const b = block as { transactions?: unknown; receipts?: unknown };
  const txs = Array.isArray(b.transactions) ? b.transactions : [];
  const receipts = Array.isArray(b.receipts) ? b.receipts : [];
  const out: ItemTouch[] = [];
  for (let i = 0; i < txs.length; i++) {
    const tx = txs[i] as Record<string, unknown> | null;
    const receipt = receipts[i] as Record<string, unknown> | undefined;
    if (!tx || typeof tx !== 'object' || !receipt || receipt.success !== true) continue;
    const body = contractCallBody(tx.payload);
    const contractHex = body ? normalizeHex64(body.contract) : null;
    if (!body || !contractHex) continue;
    const txId = normalizeHex64(receipt.tx_id) ?? normalizeHex64(tx.id) ?? `h${height}:${i}`;
    const push = (account: string | null, itemId: bigint) => {
      if (!account || account === ZERO) return;
      out.push({ contractHex, itemId: itemId.toString(), accountHex: account, blockHeight: height, txId });
    };
    const logs = Array.isArray(receipt.logs) ? receipt.logs : [];
    let sawLog = false;
    for (const raw of logs) {
      if (!raw || typeof raw !== 'object') continue;
      const l = raw as { topics?: unknown; data?: unknown };
      const topics = Array.isArray(l.topics) ? l.topics.map((t) => normalizeHex64(t)) : [];
      if (topics.length !== 3 || topics.some((t) => t == null)) continue;
      const data = calldataHex(l.data);
      if (!data) continue;
      let log: ReturnType<typeof decodeMultiTokenTransferLog> = null;
      try {
        log = decodeMultiTokenTransferLog({ topics: topics as string[], data });
      } catch {
        log = null;
      }
      if (!log) continue;
      sawLog = true;
      push(normalizeHex64(log.from), log.itemId);
      push(normalizeHex64(log.to), log.itemId);
    }
    if (sawLog) continue;
    // Receipts without logs (older RPC shapes): fall back to calldata.
    const cd = calldataHex(body.calldata);
    const d = cd ? decodeMultiTokenCalldata(cd) : null;
    if (!d) continue;
    const sender = normalizeHex64(tx.sender ?? tx.from);
    if (d.kind === 'mint' || d.kind === 'transfer') {
      push(normalizeHex64(d.to), d.itemId);
      if (d.kind === 'transfer') push(sender, d.itemId);
    } else if (d.kind === 'mint_batch' || d.kind === 'transfer_batch') {
      for (const e of d.entries) {
        push(normalizeHex64(d.to), e.itemId);
        if (d.kind === 'transfer_batch') push(sender, e.itemId);
      }
    }
  }
  return out;
}

export interface ItemsEnv {
  NFT_OWNER_DB: D1Database;
  BOING_RPC_URL: string;
  BOING_CHAIN_ID?: string;
  BOING_MAX_CONCURRENT?: string;
  /** First height to scan on a cold items cursor (template went live on testnet at 37545). */
  BOING_ITEMS_FROM_HEIGHT?: string;
  /** Max heights scanned per items request (default 48). */
  BOING_ITEMS_MAX_BLOCKS_PER_REQUEST?: string;
  /** Min seconds between request-driven catch-ups (default 10). */
  BOING_ITEMS_MIN_INTERVAL_SEC?: string;
}

function int(raw: string | undefined, fallback: number): number {
  const n = raw == null ? NaN : parseInt(String(raw), 10);
  return Number.isFinite(n) && n >= 0 ? n : fallback;
}

async function isReferenceContract(
  db: D1Database,
  client: BoingClient,
  contractHex: string,
  nowSec: number
): Promise<boolean> {
  const row = await db
    .prepare('SELECT is_reference FROM mt_contracts WHERE contract_hex = ?')
    .bind(contractHex)
    .first<{ is_reference: number }>();
  if (row) return row.is_reference === 1;
  const code = await client.getContractCode(contractHex);
  const ok = isReferenceMultiTokenBytecode(code?.bytecode ?? '');
  await db
    .prepare('INSERT OR REPLACE INTO mt_contracts (contract_hex, is_reference, checked_at) VALUES (?, ?, ?)')
    .bind(contractHex, ok ? 1 : 0, nowSec)
    .run();
  return ok;
}

/** Workers Free plan allows 50 subrequests per invocation; keep catch-up + live reads under it. */
export const ITEMS_SUBREQUEST_BUDGET = 45;

export type ItemsCatchUp = {
  ran: boolean;
  /** RPC subrequests this catch-up used (chain height + blocks + code checks). */
  subrequests?: number;
  fromHeight?: number;
  toHeight?: number;
  scanned?: number;
  touches?: number;
  lastScannedHeight: number;
  error?: string;
};

/** Bounded catch-up of the items cursor; throttled so bursts of reads cost one scan. */
export async function catchUpItems(env: ItemsEnv, opts?: { force?: boolean; maxBlocks?: number }): Promise<ItemsCatchUp> {
  const db = env.NFT_OWNER_DB;
  const chainId = env.BOING_CHAIN_ID ?? 'boing-testnet';
  const nowSec = Math.floor(Date.now() / 1000);
  const cur = await db
    .prepare('SELECT last_scanned_height, updated_at FROM mt_cursor WHERE chain_id = ?')
    .bind(chainId)
    .first<{ last_scanned_height: number; updated_at: number }>();
  const last = cur ? cur.last_scanned_height : int(env.BOING_ITEMS_FROM_HEIGHT, 0) - 1;
  const minInterval = int(env.BOING_ITEMS_MIN_INTERVAL_SEC, 10);
  if (!opts?.force && cur && nowSec - cur.updated_at < minInterval) {
    return { ran: false, subrequests: 0, lastScannedHeight: last };
  }
  const client = createClient(env.BOING_RPC_URL);
  let tip: number;
  try {
    tip = await client.chainHeight();
  } catch (e) {
    return { ran: false, subrequests: 1, lastScannedHeight: last, error: e instanceof Error ? e.message : String(e) };
  }
  const maxBlocks = Math.max(1, opts?.maxBlocks ?? int(env.BOING_ITEMS_MAX_BLOCKS_PER_REQUEST, 16));
  const fromHeight = last + 1;
  const toHeight = Math.min(tip, fromHeight + maxBlocks - 1);
  if (toHeight < fromHeight) {
    await db
      .prepare('INSERT OR REPLACE INTO mt_cursor (chain_id, last_scanned_height, updated_at) VALUES (?, ?, ?)')
      .bind(chainId, last, nowSec)
      .run();
    return { ran: true, subrequests: 1, scanned: 0, touches: 0, lastScannedHeight: last };
  }
  const bundles = await fetchBlocksWithReceiptsForHeightRange(client, fromHeight, toHeight, {
    maxConcurrent: Math.max(1, int(env.BOING_MAX_CONCURRENT, 4)),
    onMissingBlock: 'omit',
  });
  const touches: ItemTouch[] = [];
  for (const bundle of bundles) touches.push(...extractItemTouchesFromBlock(bundle.block, bundle.height));
  const verdict = new Map<string, boolean>();
  const stmts: D1PreparedStatement[] = [];
  let codeLookups = 0;
  for (const t of touches) {
    if (!verdict.has(t.contractHex)) {
      codeLookups++;
      verdict.set(t.contractHex, await isReferenceContract(db, client, t.contractHex, nowSec));
    }
    if (!verdict.get(t.contractHex)) continue;
    stmts.push(
      db
        .prepare(
          `INSERT INTO mt_holdings (contract_hex, item_id, account_hex, last_block_height, last_tx_id)
           VALUES (?, ?, ?, ?, ?)
           ON CONFLICT (contract_hex, item_id, account_hex)
           DO UPDATE SET last_block_height = excluded.last_block_height, last_tx_id = excluded.last_tx_id`
        )
        .bind(t.contractHex, t.itemId, t.accountHex, t.blockHeight, t.txId)
    );
  }
  stmts.push(
    db
      .prepare('INSERT OR REPLACE INTO mt_cursor (chain_id, last_scanned_height, updated_at) VALUES (?, ?, ?)')
      .bind(chainId, toHeight, nowSec)
  );
  await db.batch(stmts);
  return {
    ran: true,
    subrequests: 1 + (toHeight - fromHeight + 1) + codeLookups,
    fromHeight,
    toHeight,
    scanned: bundles.length,
    touches: stmts.length - 1,
    lastScannedHeight: toHeight,
  };
}

export type ItemHoldingRow = {
  contract: string;
  itemId: string;
  account: string;
  /** Live u128 balance (decimal), or null when this response ran out of RPC budget (read the slot yourself). */
  balance: string | null;
  lastBlockHeight: number;
  lastTxId: string;
};

async function withLiveBalances(
  env: ItemsEnv,
  budget: number,
  rows: { contract_hex: string; item_id: string; account_hex: string; last_block_height: number; last_tx_id: string }[]
): Promise<ItemHoldingRow[]> {
  const client = createClient(env.BOING_RPC_URL);
  const out: ItemHoldingRow[] = new Array(rows.length);
  let i = 0;
  let left = Math.max(0, budget);
  async function worker() {
    while (i < rows.length) {
      const idx = i++;
      const r = rows[idx]!;
      let balance: string | null = null;
      for (let attempt = 0; attempt < 2 && balance == null && left > 0; attempt++) {
        left--;
        try {
          balance = (await fetchMultiTokenBalance(client, r.contract_hex, r.account_hex, BigInt(r.item_id))).toString();
        } catch {
          await new Promise((res) => setTimeout(res, 200));
        }
      }
      out[idx] = {
        contract: r.contract_hex,
        itemId: r.item_id,
        account: r.account_hex,
        balance,
        lastBlockHeight: r.last_block_height,
        lastTxId: r.last_tx_id,
      };
    }
  }
  await Promise.all(Array.from({ length: Math.min(2, rows.length) }, () => worker()));
  return out;
}

/** Items an account has touched, with live balances (zero-balance rows dropped unless `includeEmpty`). */
export async function listItemsByOwner(
  env: ItemsEnv,
  owner: string,
  opts: { contract?: string | null; limit: number; includeEmpty?: boolean; budget: number }
): Promise<ItemHoldingRow[]> {
  const q = opts.contract
    ? env.NFT_OWNER_DB.prepare(
        `SELECT contract_hex, item_id, account_hex, last_block_height, last_tx_id FROM mt_holdings
         WHERE account_hex = ? AND contract_hex = ? ORDER BY contract_hex, CAST(item_id AS INTEGER) LIMIT ?`
      ).bind(owner, opts.contract, opts.limit)
    : env.NFT_OWNER_DB.prepare(
        `SELECT contract_hex, item_id, account_hex, last_block_height, last_tx_id FROM mt_holdings
         WHERE account_hex = ? ORDER BY contract_hex, CAST(item_id AS INTEGER) LIMIT ?`
      ).bind(owner, opts.limit);
  const { results } = await q.all<{
    contract_hex: string;
    item_id: string;
    account_hex: string;
    last_block_height: number;
    last_tx_id: string;
  }>();
  const rows = await withLiveBalances(env, opts.budget, results ?? []);
  return opts.includeEmpty ? rows : rows.filter((r) => r.balance !== '0');
}

/** Accounts that touched one item type, with live balances. */
export async function listItemHolders(
  env: ItemsEnv,
  contract: string,
  itemId: string,
  opts: { limit: number; includeEmpty?: boolean; budget: number }
): Promise<ItemHoldingRow[]> {
  const { results } = await env.NFT_OWNER_DB.prepare(
    `SELECT contract_hex, item_id, account_hex, last_block_height, last_tx_id FROM mt_holdings
     WHERE contract_hex = ? AND item_id = ? ORDER BY last_block_height DESC LIMIT ?`
  )
    .bind(contract, itemId, opts.limit)
    .all<{ contract_hex: string; item_id: string; account_hex: string; last_block_height: number; last_tx_id: string }>();
  const rows = await withLiveBalances(env, opts.budget, results ?? []);
  return opts.includeEmpty ? rows : rows.filter((r) => r.balance !== '0');
}

/** Parse a decimal u64 item id query param. */
export function parseItemIdParam(raw: string | null): string | null {
  if (raw == null || !/^\d{1,20}$/.test(raw.trim())) return null;
  const n = BigInt(raw.trim());
  return n < 1n << 64n ? n.toString() : null;
}
