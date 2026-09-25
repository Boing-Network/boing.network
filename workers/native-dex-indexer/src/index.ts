/**
 * Cloudflare Worker: native DEX indexer with KV-backed reserve history + D1 directory API.
 * - GET /stats, GET / — full indexer JSON: **`schemaVersion`: 1**, **`tokens`** (alias of **`tokenDirectory`**),
 *   pools with **`reserveA` / `reserveB`** when available (aligns with `boing_listDexPools` / HANDOFF §4).
 *   Pool rows may include **`createdAtHeight`**, **`tokenADecimals`**, **`tokenBDecimals`** when stats are built with **`buildNativeDexIndexerStatsForClient`** merging **`boing_listDexPools`** for the canonical factory.
 *   Optional pools_page pools_page_size
 * - GET /v1/directory/meta — row counts + last sync batch id
 * - GET /v1/directory/pools?limit=&cursor= — cursor pagination (stable order by pool_hex)
 * - GET /v1/history/pool/{pool_hex}/events?limit=&cursor= — materialized Log2 snapshot (newest first)
 * - GET /v1/history/user/{caller}/events — caller-filtered pool Log2 snapshot
 * - GET /v1/history/receipts?limit=&cursor=&topic0= — optional bounded execution-log snapshot (D1 migration 0006 + `NATIVE_DEX_INDEXER_RECEIPT_ARCHIVE_BLOCKS` > 0)
 * - GET /v1/lp/vault/{vault}/mapping — live vault → pool/share (model A)
 * - GET /v1/lp/positions?owner= — model A share positions for configured vault list
 * - GET /v1/lp/nft/positions?owner=&contract= — ERC-721 positions from D1 (when NFT contract configured)
 * - POST /v1/directory/sync — refresh KV + D1 (requires Authorization: Bearer DIRECTORY_SYNC_SECRET)
 * - Cron: refresh KV + D1
 */

import {
  buildDexOverridesFromPlainEnv,
  buildNativeDexIndexerStatsForClient,
  CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX,
  collectArchivedReceiptLogRows,
  collectNativeDexPoolEventsForPools,
  collectNftOwnersFromErc721Transfers,
  createClient,
  fetchNativeDexLpVaultSharePositionForOwner,
  NATIVE_DEX_DIRECTORY_SCHEMA_VERSION,
  resolveNativeAmmVaultPoolMapping,
  validateHex32,
} from 'boing-sdk';
import {
  getDirectoryMeta,
  invalidateDirectoryPoolEventsIfTipReorged,
  listDirectoryNftPositionsPage,
  listDirectoryPoolEventsPage,
  listDirectoryPoolsPage,
  listDirectoryReceiptLogsPage,
  listDirectoryUserEventsPage,
  syncDirectoryIndexerTip,
  syncDirectoryNftOwnersFromRows,
  syncDirectoryPoolEventsFromPayload,
  syncDirectoryPoolsFromPayload,
  syncDirectoryReceiptLogsFromRows,
} from './directoryD1';

type Env = {
  BOING_TESTNET_RPC_URL?: string;
  NATIVE_DEX_INDEXER_RPC_URL?: string;
  NATIVE_DEX_INDEXER_KV?: KVNamespace;
  DIRECTORY_DB?: D1Database;
  DIRECTORY_SYNC_SECRET?: string;
  NATIVE_DEX_INDEXER_REGISTER_FROM_BLOCK?: string;
  NATIVE_DEX_INDEXER_LOG_SCAN_BLOCKS?: string;
  NATIVE_DEX_INDEXER_TOKEN_USD_JSON?: string;
  NATIVE_DEX_INDEXER_TOKEN_DIRECTORY_JSON?: string;
  NATIVE_DEX_INDEXER_API_DISABLE?: string;
  /** Comma-separated 32-byte vault hex ids; falls back to canonical testnet vault when unset. */
  NATIVE_DEX_INDEXER_LP_VAULT_HEXES?: string;
  /** Single vault hex (optional alternative to NATIVE_DEX_INDEXER_LP_VAULT_HEXES). */
  NATIVE_DEX_INDEXER_LP_VAULT_HEX?: string;
  /** ERC-721 LP position NFT contract — enables Transfer indexing into D1. */
  NATIVE_DEX_INDEXER_LP_NFT_CONTRACT_HEX?: string;
  /**
   * When > 0, each sync archives execution logs from **`getBlockByHeight(..., include_receipts: true)`**
   * for the last N blocks (bounded; requires D1 migration **`0006_directory_receipt_log.sql`**).
   */
  NATIVE_DEX_INDEXER_RECEIPT_ARCHIVE_BLOCKS?: string;
  /** Optional R2 binding for tiny JSON sync manifests (operator creates bucket + binding). */
  INDEXER_ARCHIVE_R2?: R2Bucket;
  [key: string]: string | KVNamespace | D1Database | R2Bucket | undefined;
};

const corsJsonHeaders: Record<string, string> = {
  'Content-Type': 'application/json',
  'Access-Control-Allow-Origin': '*',
};

function json(data: unknown, status = 200, extraHeaders: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { ...corsJsonHeaders, ...extraHeaders },
  });
}

function parseIntOpt(raw: string | undefined, fallback: number): number {
  if (raw == null || !String(raw).trim()) return fallback;
  const n = parseInt(String(raw), 10);
  return Number.isFinite(n) ? n : fallback;
}

/** Optional: `?pools_page=&pools_page_size=` (1–500) slices `pools` and adds `poolsPageMeta`. */
function sliceIndexerPoolsInPayload(payload: Record<string, unknown>, url: URL): Record<string, unknown> {
  const pageRaw = url.searchParams.get('pools_page');
  const sizeRaw = url.searchParams.get('pools_page_size');
  if (pageRaw == null || sizeRaw == null || pageRaw === '' || sizeRaw === '') return payload;
  const page = parseInt(String(pageRaw), 10);
  const pageSize = parseInt(String(sizeRaw), 10);
  if (!Number.isFinite(page) || page < 0) return payload;
  if (!Number.isFinite(pageSize) || pageSize < 1 || pageSize > 500) return payload;
  const pools = Array.isArray(payload.pools) ? (payload.pools as unknown[]) : [];
  const total = pools.length;
  const start = page * pageSize;
  const slice = pools.slice(start, start + pageSize);
  return {
    ...payload,
    pools: slice,
    poolsPageMeta: { page, pageSize, total, returned: slice.length },
  };
}

async function buildPayload(env: Env) {
  const baseUrl = rpcBaseUrl(env);

  const client = createClient({ baseUrl });
  const overrides = buildDexOverridesFromPlainEnv(env as Record<string, string | undefined>);
  const regRaw = env.NATIVE_DEX_INDEXER_REGISTER_FROM_BLOCK;
  const registerFromBlock =
    regRaw != null && String(regRaw).trim() !== '' ? parseIntOpt(regRaw, NaN) : NaN;

  const kv = env.NATIVE_DEX_INDEXER_KV;
  const historyStore = kv
    ? {
        get: () => kv.get('native_dex_indexer_state_v1'),
        put: (body: string) => kv.put('native_dex_indexer_state_v1', body),
      }
    : null;

  return buildNativeDexIndexerStatsForClient(client, {
    overrides: Object.keys(overrides).length ? overrides : undefined,
    registerFromBlock: Number.isFinite(registerFromBlock) && registerFromBlock >= 0 ? registerFromBlock : undefined,
    logScanBlocks: parseIntOpt(env.NATIVE_DEX_INDEXER_LOG_SCAN_BLOCKS, 8000),
    historyStore,
    tokenUsdJson: env.NATIVE_DEX_INDEXER_TOKEN_USD_JSON,
    tokenDirectoryExtraJson: env.NATIVE_DEX_INDEXER_TOKEN_DIRECTORY_JSON,
  });
}

function rpcBaseUrl(env: Env): string {
  return (
    env.NATIVE_DEX_INDEXER_RPC_URL ||
    env.BOING_TESTNET_RPC_URL ||
    'https://testnet-rpc.boing.network'
  ).replace(/\/$/, '');
}

function lpVaultHexesFromEnv(env: Env): string[] {
  const raw = String(env.NATIVE_DEX_INDEXER_LP_VAULT_HEXES || env.NATIVE_DEX_INDEXER_LP_VAULT_HEX || '').trim();
  const fromEnv = raw
    ? raw
        .split(',')
        .map((s) => s.trim().toLowerCase())
        .filter((h) => /^0x[0-9a-f]{64}$/.test(h))
    : [];
  if (fromEnv.length > 0) return fromEnv;
  try {
    return [String(CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX).toLowerCase()];
  } catch {
    return [];
  }
}

async function maybeWriteIndexerArchiveR2(env: Env, payload: Awaited<ReturnType<typeof buildPayload>>): Promise<void> {
  const bucket = env.INDEXER_ARCHIVE_R2;
  if (!bucket) return;
  const key = `manifests/directory-${payload.updatedAt.replace(/[:.]/g, '-')}.json`;
  const body = JSON.stringify({
    schemaVersion: NATIVE_DEX_DIRECTORY_SCHEMA_VERSION,
    updatedAt: payload.updatedAt,
    headHeight: payload.headHeight,
    poolCount: Array.isArray(payload.pools) ? payload.pools.length : 0,
  });
  await bucket.put(key, body, { httpMetadata: { contentType: 'application/json' } });
}

async function persistDirectoryIfBound(env: Env, payload: Awaited<ReturnType<typeof buildPayload>>): Promise<void> {
  const db = env.DIRECTORY_DB;
  if (!db) return;
  await syncDirectoryPoolsFromPayload(db, payload);

  const logScanBlocks = parseIntOpt(env.NATIVE_DEX_INDEXER_LOG_SCAN_BLOCKS, 8000);
  const head = payload.headHeight;
  const pools = Array.isArray(payload.pools) ? payload.pools : [];
  const poolHexes = pools
    .map((p) => String((p as { poolHex?: string }).poolHex || '').trim().toLowerCase())
    .filter((h) => /^0x[0-9a-f]{64}$/.test(h));

  try {
    const client = createClient({ baseUrl: rpcBaseUrl(env) });
    await invalidateDirectoryPoolEventsIfTipReorged(db, client);

    if (head == null || !Number.isFinite(head) || poolHexes.length === 0) {
      await syncDirectoryPoolEventsFromPayload(db, payload.updatedAt, []);
    } else {
      const toB = Math.floor(head);
      const fromB = Math.max(0, toB - logScanBlocks + 1);
      const events = await collectNativeDexPoolEventsForPools(client, poolHexes, { fromBlock: fromB, toBlock: toB });
      await syncDirectoryPoolEventsFromPayload(db, payload.updatedAt, events);
    }

    const nftContract = String(env.NATIVE_DEX_INDEXER_LP_NFT_CONTRACT_HEX || '').trim().toLowerCase();
    if (head != null && Number.isFinite(head) && /^0x[0-9a-f]{64}$/.test(nftContract)) {
      try {
        const toB = Math.floor(head);
        const fromB = Math.max(0, toB - logScanBlocks + 1);
        const nftRows = await collectNftOwnersFromErc721Transfers(client, nftContract, { fromBlock: fromB, toBlock: toB });
        await syncDirectoryNftOwnersFromRows(db, payload.updatedAt, nftRows);
      } catch {
        /* NFT sync optional */
      }
    } else {
      try {
        await syncDirectoryNftOwnersFromRows(db, payload.updatedAt, []);
      } catch {
        /* table missing */
      }
    }

    try {
      const receiptArchiveBlocks = parseIntOpt(env.NATIVE_DEX_INDEXER_RECEIPT_ARCHIVE_BLOCKS, 0);
      if (head != null && Number.isFinite(head) && receiptArchiveBlocks > 0) {
        const toB = Math.floor(head);
        const fromB = Math.max(0, toB - receiptArchiveBlocks + 1);
        const receiptRows = await collectArchivedReceiptLogRows(client, fromB, toB, {
          maxConcurrent: 1,
          maxRows: 5000,
        });
        await syncDirectoryReceiptLogsFromRows(db, payload.updatedAt, receiptRows);
      } else {
        await syncDirectoryReceiptLogsFromRows(db, payload.updatedAt, []);
      }
    } catch {
      /* optional receipt archive; do not fail pool/NFT sync */
    }

    try {
      if (head == null || !Number.isFinite(head) || poolHexes.length === 0) {
        await syncDirectoryIndexerTip(db, payload.updatedAt, null, null, null);
      } else {
        const toB = Math.floor(head);
        let tipHash: string | null = null;
        let parentHash: string | null = null;
        try {
          const blk = await client.getBlockByHeight(toB, false);
          const h = blk?.hash;
          tipHash = typeof h === 'string' && /^0x[0-9a-f]{64}$/i.test(h) ? h.toLowerCase() : null;
          const ph = blk?.header?.parent_hash;
          parentHash =
            typeof ph === 'string' && /^0x[0-9a-f]{64}$/i.test(ph) ? ph.toLowerCase() : null;
        } catch {
          /* optional */
        }
        await syncDirectoryIndexerTip(db, payload.updatedAt, toB, tipHash, parentHash);
      }
    } catch {
      /* directory_indexer_tip missing until migration 0003 */
    }

    await maybeWriteIndexerArchiveR2(env, payload);
  } catch {
    /* keep previous directory_pool_events rows on RPC failure */
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        status: 204,
        headers: {
          'Access-Control-Allow-Origin': '*',
          'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
          'Access-Control-Allow-Headers': 'Content-Type, Authorization',
        },
      });
    }

    const disable = String(env.NATIVE_DEX_INDEXER_API_DISABLE || '').trim();
    if (disable === '1' || disable.toLowerCase() === 'true') {
      return json({ error: 'disabled' }, 503);
    }

    const url = new URL(request.url);
    const path = url.pathname.replace(/\/$/, '') || '/';

    try {
      if (path === '/v1/directory/meta' && request.method === 'GET') {
        const db = env.DIRECTORY_DB;
        if (!db) {
          return json({ error: 'D1 not configured (DIRECTORY_DB binding missing)' }, 503);
        }
        const meta = await getDirectoryMeta(db);
        const emptyDirectoryNote =
          meta.poolCount === 0
            ? 'Empty until ops: set REACT_APP_BOING_NATIVE_AMM_POOL / BOING_CANONICAL_NATIVE_* (or end_user.canonical_native_*) on the RPC, then wait for cron or POST /v1/directory/sync. poolCount 0 before the first successful sync is expected.'
            : null;
        return json({
          api: 'boing-native-dex-directory/v1',
          resource: 'meta',
          schemaVersion: NATIVE_DEX_DIRECTORY_SCHEMA_VERSION,
          ...meta,
          emptyDirectory: meta.poolCount === 0,
          emptyDirectoryNote,
        });
      }

      if (path === '/v1/directory/pools' && request.method === 'GET') {
        const db = env.DIRECTORY_DB;
        if (!db) {
          return json({ error: 'D1 not configured (DIRECTORY_DB binding missing)' }, 503);
        }
        const page = await listDirectoryPoolsPage(db, url);
        return json({
          api: 'boing-native-dex-directory/v1',
          resource: 'pools',
          ...page,
        });
      }

      const historyMatch = /^\/v1\/history\/pool\/(0x[0-9a-f]{64})\/events$/i.exec(path);
      if (historyMatch && request.method === 'GET') {
        const db = env.DIRECTORY_DB;
        if (!db) {
          return json({ error: 'D1 not configured (DIRECTORY_DB binding missing)' }, 503);
        }
        const poolHex = historyMatch[1]!.toLowerCase();
        const page = await listDirectoryPoolEventsPage(db, poolHex, url);
        return json({
          api: 'boing-native-dex-directory/v1',
          resource: 'pool_events',
          ...page,
        });
      }

      const userHistoryMatch = /^\/v1\/history\/user\/(0x[0-9a-f]{64})\/events$/i.exec(path);
      if (userHistoryMatch && request.method === 'GET') {
        const db = env.DIRECTORY_DB;
        if (!db) {
          return json({ error: 'D1 not configured (DIRECTORY_DB binding missing)' }, 503);
        }
        const callerHex = userHistoryMatch[1]!.toLowerCase();
        const page = await listDirectoryUserEventsPage(db, callerHex, url);
        return json({
          api: 'boing-native-dex-directory/v1',
          resource: 'user_pool_events',
          ...page,
        });
      }

      if (path === '/v1/history/receipts' && request.method === 'GET') {
        const db = env.DIRECTORY_DB;
        if (!db) {
          return json({ error: 'D1 not configured (DIRECTORY_DB binding missing)' }, 503);
        }
        const page = await listDirectoryReceiptLogsPage(db, url);
        return json({
          api: 'boing-native-dex-directory/v1',
          resource: 'receipt_logs',
          schemaVersion: NATIVE_DEX_DIRECTORY_SCHEMA_VERSION,
          ...page,
        });
      }

      const vaultMapMatch = /^\/v1\/lp\/vault\/(0x[0-9a-f]{64})\/mapping$/i.exec(path);
      if (vaultMapMatch && request.method === 'GET') {
        const client = createClient({ baseUrl: rpcBaseUrl(env) });
        const vaultHex = vaultMapMatch[1]!.toLowerCase() as `0x${string}`;
        const mapping = await resolveNativeAmmVaultPoolMapping(client, vaultHex);
        return json({
          api: 'boing-native-dex-directory/v1',
          resource: 'lp_vault_mapping',
          ...mapping,
        });
      }

      if (path === '/v1/lp/positions' && request.method === 'GET') {
        const ownerRaw = (url.searchParams.get('owner') || '').trim().toLowerCase();
        if (!/^0x[0-9a-f]{64}$/.test(ownerRaw)) {
          return json({ error: 'bad_request', detail: 'owner query param must be 0x + 64 hex' }, 400);
        }
        const client = createClient({ baseUrl: rpcBaseUrl(env) });
        const vaults = lpVaultHexesFromEnv(env);
        const positions: unknown[] = [];
        for (const v of vaults) {
          try {
            validateHex32(v);
            const row = await fetchNativeDexLpVaultSharePositionForOwner(client, {
              vaultHex32: v,
              ownerHex32: ownerRaw,
            });
            positions.push(row);
          } catch {
            /* skip vault */
          }
        }
        return json({
          api: 'boing-native-dex-directory/v1',
          resource: 'lp_positions',
          schemaVersion: NATIVE_DEX_DIRECTORY_SCHEMA_VERSION,
          ownerHex: ownerRaw,
          vaultsQueried: vaults,
          positions,
        });
      }

      if (path === '/v1/lp/nft/positions' && request.method === 'GET') {
        const db = env.DIRECTORY_DB;
        if (!db) {
          return json({ error: 'D1 not configured (DIRECTORY_DB binding missing)' }, 503);
        }
        const ownerRaw = (url.searchParams.get('owner') || '').trim().toLowerCase();
        const contractRaw = (url.searchParams.get('contract') || env.NATIVE_DEX_INDEXER_LP_NFT_CONTRACT_HEX || '')
          .trim()
          .toLowerCase();
        if (!/^0x[0-9a-f]{64}$/.test(ownerRaw)) {
          return json({ error: 'bad_request', detail: 'owner must be 0x + 64 hex' }, 400);
        }
        if (!/^0x[0-9a-f]{64}$/.test(contractRaw)) {
          return json(
            {
              error: 'bad_request',
              detail: 'contract query param or NATIVE_DEX_INDEXER_LP_NFT_CONTRACT_HEX must be set to a 32-byte hex id',
            },
            400,
          );
        }
        const page = await listDirectoryNftPositionsPage(db, contractRaw, ownerRaw, url);
        return json({
          api: 'boing-native-dex-directory/v1',
          resource: 'lp_nft_positions',
          schemaVersion: NATIVE_DEX_DIRECTORY_SCHEMA_VERSION,
          ...page,
        });
      }

      if (path === '/v1/directory/sync' && request.method === 'POST') {
        const secret = String(env.DIRECTORY_SYNC_SECRET || '').trim();
        if (!secret) {
          return json({ error: 'DIRECTORY_SYNC_SECRET not set on worker' }, 503);
        }
        const authHeader = (request.headers.get('Authorization') || '').trim();
        const bearerMatch = /^Bearer\s+(\S.*)$/i.exec(authHeader);
        const token = bearerMatch ? bearerMatch[1]!.trim() : '';
        if (token !== secret) {
          return json({ error: 'unauthorized' }, 401);
        }
        const payload = await buildPayload(env);
        await persistDirectoryIfBound(env, payload);
        const meta = env.DIRECTORY_DB
          ? await getDirectoryMeta(env.DIRECTORY_DB)
          : {
              poolCount: 0,
              eventCount: 0,
              nftOwnerRowCount: 0,
              receiptLogCount: 0,
              latestSyncBatch: null,
              indexedTipHeight: null,
              indexedTipBlockHash: null,
              indexedParentBlockHash: null,
            };
        return json({
          ok: true,
          schemaVersion: NATIVE_DEX_DIRECTORY_SCHEMA_VERSION,
          updatedAt: payload.updatedAt,
          poolCount: meta.poolCount,
          eventCount: meta.eventCount,
          nftOwnerRowCount: meta.nftOwnerRowCount,
          receiptLogCount: meta.receiptLogCount,
          latestSyncBatch: meta.latestSyncBatch,
          indexedTipHeight: meta.indexedTipHeight,
          indexedTipBlockHash: meta.indexedTipBlockHash,
          indexedParentBlockHash: meta.indexedParentBlockHash,
        });
      }

      if (request.method === 'GET' && (path === '/stats' || path === '/')) {
        const payload = await buildPayload(env);
        const sliced = sliceIndexerPoolsInPayload(payload as Record<string, unknown>, url);
        return new Response(JSON.stringify(sliced), {
          headers: {
            'Content-Type': 'application/json',
            'Access-Control-Allow-Origin': '*',
            'Cache-Control': 'public, max-age=30',
          },
        });
      }

      return new Response(
        JSON.stringify({
          error: 'not_found',
          hint:
            'GET /stats, GET /v1/directory/*, GET /v1/history/pool|user/{hex}/events, GET /v1/history/receipts, GET /v1/lp/vault/{vault}/mapping, GET /v1/lp/positions?owner=, GET /v1/lp/nft/positions?owner=&contract=, POST /v1/directory/sync',
        }),
        { status: 404, headers: { 'Content-Type': 'application/json', ...corsJsonHeaders } },
      );
    } catch (e) {
      return json(
        {
          error: e instanceof Error ? e.message : String(e),
        },
        500,
      );
    }
  },

  async scheduled(_event: ScheduledEvent, env: Env, ctx: ExecutionContext): Promise<void> {
    const disable = String(env.NATIVE_DEX_INDEXER_API_DISABLE || '').trim();
    if (disable === '1' || disable.toLowerCase() === 'true') return;
    ctx.waitUntil(
      (async () => {
        try {
          const payload = await buildPayload(env);
          await persistDirectoryIfBound(env, payload);
        } catch {
          /* ignore */
        }
      })(),
    );
  },
};
