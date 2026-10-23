import type { BlockWithReceiptsBundle } from '@boing-network/boing-sdk';

/** Normalize block / tx id hex for comparisons and D1 storage. */
export function normalizeObserverBlockHash(h: string | undefined): string {
  if (h == null || h === '') return '0x' + '00'.repeat(32);
  const s = String(h).trim();
  if (s.startsWith('0x') || s.startsWith('0X')) return s.toLowerCase();
  if (/^[0-9a-fA-F]{64}$/.test(s)) return `0x${s.toLowerCase()}`;
  return s;
}

function normHash(h: string | undefined): string {
  return normalizeObserverBlockHash(h);
}

/** Normalize emitting contract id on receipt logs when the node provides **`address`**. */
export function normalizeLogContractAddress(raw: unknown): string | null {
  if (raw == null || typeof raw !== 'string') return null;
  const s = raw.trim();
  const hex = s.startsWith('0x') || s.startsWith('0X') ? s.slice(2) : s;
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) return null;
  return `0x${hex.toLowerCase()}`;
}

function resolveTxId(rec: unknown, tx: unknown): string | null {
  if (rec && typeof rec === 'object' && typeof (rec as { tx_id?: string }).tx_id === 'string') {
    return (rec as { tx_id: string }).tx_id;
  }
  if (tx && typeof tx === 'object') {
    const o = tx as { tx_id?: string; id?: string };
    if (typeof o.tx_id === 'string') return o.tx_id;
    if (typeof o.id === 'string') return o.id;
  }
  return null;
}

/** Remove one height and dependent rows (reorg rewind). */
export async function deleteBlockAtHeightD1(db: D1Database, height: number): Promise<void> {
  await db.prepare('DELETE FROM logs WHERE block_height = ?').bind(height).run();
  const txRows = await db
    .prepare('SELECT tx_id FROM transactions WHERE block_height = ?')
    .bind(height)
    .all<{ tx_id: string }>();
  const txIds = (txRows.results ?? []).map((r) => r.tx_id);
  for (const tid of txIds) {
    await db.prepare('DELETE FROM receipts WHERE tx_id = ?').bind(tid).run();
  }
  await db.prepare('DELETE FROM transactions WHERE block_height = ?').bind(height).run();
  await db.prepare('DELETE FROM blocks WHERE height = ?').bind(height).run();
}

export async function getBlockRowAtHeight(
  db: D1Database,
  height: number
): Promise<{ height: number; block_hash: string; parent_hash: string } | null> {
  const row = await db
    .prepare('SELECT height, block_hash, parent_hash FROM blocks WHERE height = ?')
    .bind(height)
    .first<{ height: number; block_hash: string; parent_hash: string }>();
  return row ?? null;
}

/** Highest **`blocks.height`**, or **`-1`** when empty. */
export async function getMaxBlockHeightD1(db: D1Database): Promise<number> {
  const row = await db.prepare('SELECT MAX(height) AS mh FROM blocks').first<{ mh: number | null }>();
  const mh = row?.mh;
  if (mh == null || !Number.isFinite(mh)) return -1;
  return mh;
}

/** Set ingest cursor to the current tip of **`blocks`** (repair / partial rewind). */
export async function reconcileIngestCursorToBlocksTipD1(
  db: D1Database,
  chainId: string,
  nowSec: number
): Promise<{ lastHeight: number; lastHash: string }> {
  const maxH = await getMaxBlockHeightD1(db);
  const zeros = '0x' + '00'.repeat(32);
  if (maxH < 0) {
    await upsertIngestCursorD1(db, chainId, -1, zeros, nowSec);
    return { lastHeight: -1, lastHash: zeros };
  }
  const row = await getBlockRowAtHeight(db, maxH);
  const tipHash = row?.block_hash != null ? normHash(row.block_hash) : zeros;
  await upsertIngestCursorD1(db, chainId, maxH, tipHash, nowSec);
  return { lastHeight: maxH, lastHash: tipHash };
}

export async function persistBlockBundleD1(db: D1Database, bundle: BlockWithReceiptsBundle): Promise<void> {
  const { height, block } = bundle;
  const hash = normHash(block.hash);
  const parent = normHash(block.header?.parent_hash);
  const blockJson = JSON.stringify(block);

  await deleteBlockAtHeightD1(db, height);

  await db
    .prepare(
      `INSERT OR REPLACE INTO blocks (height, block_hash, parent_hash, block_json)
       VALUES (?, ?, ?, ?)`
    )
    .bind(height, hash, parent, blockJson)
    .run();

  const txs = Array.isArray(block.transactions) ? block.transactions : [];
  const receipts = Array.isArray(block.receipts) ? block.receipts : [];

  for (let i = 0; i < txs.length; i++) {
    const tx = txs[i];
    const rec = receipts[i] ?? null;
    const txId = resolveTxId(rec, tx);
    if (!txId) continue;

    const rawHex = typeof tx === 'string' ? tx : JSON.stringify(tx);
    let payloadKind: string | null = null;
    if (tx && typeof tx === 'object') {
      const keys = Object.keys(tx as object);
      if (keys.length === 1) payloadKind = keys[0] ?? null;
    }

    let sender: string | null = null;
    if (tx && typeof tx === 'object' && (tx as { Transfer?: { from?: string } }).Transfer != null) {
      const from = (tx as { Transfer: { from?: string } }).Transfer.from;
      if (typeof from === 'string') sender = from;
    }

    await db
      .prepare(
        `INSERT OR REPLACE INTO transactions (tx_id, block_height, tx_index, sender, payload_kind, raw_hex)
         VALUES (?, ?, ?, ?, ?, ?)`
      )
      .bind(txId, height, i, sender, payloadKind, rawHex)
      .run();

    if (rec && typeof rec === 'object' && typeof (rec as { tx_id: string }).tx_id === 'string') {
      const r = rec as {
        tx_id: string;
        success: boolean;
        gas_used: number;
        return_data?: string;
        error?: string | null;
        logs?: unknown[];
      };
      await db
        .prepare(
          `INSERT OR REPLACE INTO receipts (tx_id, success, gas_used, return_data, error)
           VALUES (?, ?, ?, ?, ?)`
        )
        .bind(r.tx_id, r.success ? 1 : 0, String(r.gas_used ?? 0), r.return_data ?? null, r.error ?? null)
        .run();

      const logs = Array.isArray(r.logs) ? r.logs : [];
      for (let li = 0; li < logs.length; li++) {
        const log = logs[li];
        if (!log || typeof log !== 'object') continue;
        const topics = Array.isArray((log as { topics?: unknown[] }).topics)
          ? ((log as { topics: unknown[] }).topics as string[])
          : [];
        const logAddress = normalizeLogContractAddress((log as { address?: unknown }).address);
        await db
          .prepare(
            `INSERT OR REPLACE INTO logs (tx_id, log_index, block_height, address, topics_json, data_hex)
             VALUES (?, ?, ?, ?, ?, ?)`
          )
          .bind(
            r.tx_id,
            li,
            height,
            logAddress,
            JSON.stringify(topics.map((t) => String(t))),
            (log as { data?: string }).data != null ? String((log as { data: string }).data) : '0x'
          )
          .run();
      }
    }
  }
}

export async function replaceBlockHeightGapsD1(
  db: D1Database,
  chainId: string,
  gapRanges: readonly { fromHeight: number; toHeight: number }[],
  nowSec: number
): Promise<void> {
  await db.prepare('DELETE FROM block_height_gaps WHERE chain_id = ?').bind(chainId).run();
  const stmts: D1PreparedStatement[] = [];
  for (const g of gapRanges) {
    stmts.push(
      db
        .prepare(
          `INSERT INTO block_height_gaps (chain_id, from_height, to_height, reason, recorded_at)
           VALUES (?, ?, ?, 'pruned', ?)`
        )
        .bind(chainId, g.fromHeight, g.toHeight, nowSec)
    );
  }
  if (stmts.length > 0) {
    await db.batch(stmts);
  }
}

export async function upsertIngestCursorD1(
  db: D1Database,
  chainId: string,
  lastHeight: number,
  lastHash: string,
  nowSec: number
): Promise<void> {
  await db
    .prepare(
      `INSERT INTO ingest_cursor (chain_id, last_committed_height, last_committed_block_hash, updated_at, readiness_lag_guard_armed)
       VALUES (?, ?, ?, ?, 0)
       ON CONFLICT(chain_id) DO UPDATE SET
         last_committed_height = excluded.last_committed_height,
         last_committed_block_hash = excluded.last_committed_block_hash,
         updated_at = excluded.updated_at`
    )
    .bind(chainId, lastHeight, normHash(lastHash), nowSec)
    .run();
}

/**
 * After catch-up, persist **`readiness_lag_guard_armed = 1`** so **`/api/readiness`** may enforce **`maxLagFinalized`**.
 * No-op if max lag unset, already armed, no cursor row yet, or **`lag > armWhenLagLte`**.
 */
export async function maybeArmReadinessLagGuardD1(
  db: D1Database,
  chainId: string,
  lastCommittedHeight: number,
  finalizedHeight: number,
  maxLagFinalized: number | null,
  armWhenLagLte: number,
  nowSec: number
): Promise<boolean> {
  if (maxLagFinalized == null) return false;
  if (lastCommittedHeight < 0) return false;
  const row = await db
    .prepare(
      'SELECT COALESCE(readiness_lag_guard_armed, 0) AS armed FROM ingest_cursor WHERE chain_id = ?'
    )
    .bind(chainId)
    .first<{ armed: number }>();
  if (row == null) return false;
  if ((row.armed ?? 0) === 1) return false;
  const lag = finalizedHeight - lastCommittedHeight;
  if (lag > armWhenLagLte) return false;
  await db
    .prepare(
      'UPDATE ingest_cursor SET readiness_lag_guard_armed = 1, updated_at = ? WHERE chain_id = ?'
    )
    .bind(nowSec, chainId)
    .run();
  return true;
}

export function blockHashAtHeight(bundles: readonly BlockWithReceiptsBundle[], height: number): string | null {
  const b = bundles.find((x) => x.height === height);
  if (!b) return null;
  return normHash(b.block.hash);
}
