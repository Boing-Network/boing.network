/**
 * Cloudflare Worker: **universal contract deploy registry** (D1 + HTTP).
 *
 * - **Cron:** scans `boing_getBlockByHeight` from a persisted cursor through tip (bounded blocks/tick).
 * - **GET /v1/deployments** — cursor pagination by monotonic `id`.
 * - **GET /v1/deployments/stream** — **SSE** (`text/event-stream`): emits JSON rows for new deploys and `ping` when idle (clients reconnect as needed).
 * - **GET /v1/status** — ingest cursor, chain tip, pending block count, effective config.
 * - **GET /v1/contract/{0x…64}** — single-row lookup by predicted contract id.
 * - **POST /v1/sync** — optional one-shot ingest (requires `DEPLOY_REGISTRY_SYNC_SECRET` + `Authorization: Bearer …`).
 * - **GET /v1/deployments/by-tx/{0x…64}**, **by-block/{height}**, **by-sender/{0x…64}** — filtered lists (indexed columns).
 *
 * Pair with **`BoingNewHeadsWs`** on the node (or poll chain height) if you want tighter client-side scheduling;
 * this worker advances on **cron** by default.
 */

import {
  createClient,
  extractUniversalContractDeploymentsFromBlock,
  type UniversalContractDeploymentRow,
} from '@boing-network/boing-sdk';

type Env = {
  DEPLOY_REGISTRY_DB: D1Database;
  DEPLOY_REGISTRY_RPC_URL?: string;
  DEPLOY_REGISTRY_FROM_HEIGHT?: string;
  DEPLOY_REGISTRY_MAX_BLOCKS_PER_TICK?: string;
  /** Concurrent `getBlockByHeight` calls per tick (1–16). */
  DEPLOY_REGISTRY_PARALLEL_FETCHES?: string;
  /** If set, `POST /v1/sync` with matching `Authorization: Bearer …` runs one bounded ingest pass. */
  DEPLOY_REGISTRY_SYNC_SECRET?: string;
};

const cors: Record<string, string> = {
  'Content-Type': 'application/json',
  'Access-Control-Allow-Origin': '*',
};

const ROW_SELECT = `id, contract_hex, block_height, tx_index, tx_id_hex, sender_hex, payload_kind, purpose_category, asset_name, asset_symbol` as const;

export type SyncTelemetry = {
  ok: boolean;
  at: string;
  durationMs: number;
  indexedBlocks?: number;
  rowsInserted?: number;
  nextHeight?: number;
  chainTip?: number | null;
  error?: string;
};

async function writeSyncTelemetry(db: D1Database, t: SyncTelemetry): Promise<void> {
  await db
    .prepare(`INSERT INTO ingest_state (k, v) VALUES ('last_sync', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v`)
    .bind(JSON.stringify(t))
    .run();
}

async function readSyncTelemetry(db: D1Database): Promise<SyncTelemetry | null> {
  const r = await db.prepare(`SELECT v FROM ingest_state WHERE k = 'last_sync'`).first<{ v: string }>();
  if (r?.v == null || r.v === '') return null;
  try {
    return JSON.parse(r.v) as SyncTelemetry;
  } catch {
    return null;
  }
}

function json(data: unknown, status = 200, extra: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(data), { status, headers: { ...cors, ...extra } });
}

function parseIntOpt(raw: string | undefined, fallback: number): number {
  if (raw == null || !String(raw).trim()) return fallback;
  const n = parseInt(String(raw), 10);
  return Number.isFinite(n) ? n : fallback;
}

async function readNextHeight(db: D1Database, fallback: number): Promise<number> {
  const r = await db.prepare(`SELECT v FROM ingest_state WHERE k = ?`).bind('next_height').first<{ v: string }>();
  if (r?.v != null) {
    const n = parseInt(r.v, 10);
    if (Number.isFinite(n) && n >= 0) return n;
  }
  return fallback;
}

async function writeNextHeight(db: D1Database, h: number): Promise<void> {
  await db
    .prepare(`INSERT INTO ingest_state (k, v) VALUES ('next_height', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v`)
    .bind(String(h))
    .run();
}

const D1_INSERT_BATCH = 100;

function prepareInsertDeployment(db: D1Database, row: UniversalContractDeploymentRow) {
  return db
    .prepare(
      `INSERT OR IGNORE INTO contract_deployments
       (contract_hex, block_height, tx_index, tx_id_hex, sender_hex, payload_kind, purpose_category, asset_name, asset_symbol)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      row.contractHex.toLowerCase(),
      row.blockHeight,
      row.txIndex,
      row.txIdHex.toLowerCase(),
      row.senderHex.toLowerCase(),
      row.payloadKind,
      row.purposeCategory ?? null,
      row.assetName ?? null,
      row.assetSymbol ?? null,
    );
}

/** Sum `changes` from batched `INSERT OR IGNORE` statements (fewer D1 round-trips than one-by-one). */
async function insertDeploymentRowsBatch(db: D1Database, rows: UniversalContractDeploymentRow[]): Promise<number> {
  if (rows.length === 0) return 0;
  let inserted = 0;
  for (let i = 0; i < rows.length; i += D1_INSERT_BATCH) {
    const slice = rows.slice(i, i + D1_INSERT_BATCH);
    const stmts = slice.map((row) => prepareInsertDeployment(db, row));
    const results = await db.batch(stmts);
    for (const res of results) {
      inserted += typeof res.meta?.changes === 'number' ? res.meta.changes : 0;
    }
  }
  return inserted;
}

export type SyncDeployRegistryResult = {
  indexedBlocks: number;
  rowsInserted: number;
  nextHeight: number;
  chainTip: number | null;
  durationMs: number;
};

export async function syncDeployRegistry(env: Env): Promise<SyncDeployRegistryResult> {
  const t0 = Date.now();
  const baseUrl = String(env.DEPLOY_REGISTRY_RPC_URL || '').trim();
  if (!baseUrl) {
    return {
      indexedBlocks: 0,
      rowsInserted: 0,
      nextHeight: 0,
      chainTip: null,
      durationMs: Date.now() - t0,
    };
  }

  try {
    const client = createClient({ baseUrl });
    const head = await client.chainHeight();
    const fromDefault = parseIntOpt(env.DEPLOY_REGISTRY_FROM_HEIGHT, 0);
    const maxPer = Math.min(256, Math.max(1, parseIntOpt(env.DEPLOY_REGISTRY_MAX_BLOCKS_PER_TICK, 8)));
    const parallel = Math.min(16, Math.max(1, parseIntOpt(env.DEPLOY_REGISTRY_PARALLEL_FETCHES, 6)));

    let cursor = await readNextHeight(env.DEPLOY_REGISTRY_DB, fromDefault);
    let indexedBlocks = 0;
    let rowsInserted = 0;

    const chainTip = typeof head === 'number' && Number.isFinite(head) ? head : null;
    const tipN = chainTip ?? 0;
    let h = cursor;
    while (indexedBlocks < maxPer && h <= tipN) {
      const room = maxPer - indexedBlocks;
      const horizon = tipN - h + 1;
      const chunk = Math.min(parallel, room, horizon);
      const heights = Array.from({ length: chunk }, (_, i) => h + i);
      const blocks = await Promise.all(heights.map((height) => client.getBlockByHeight(height, false)));

      const batchRows: UniversalContractDeploymentRow[] = [];
      for (const block of blocks) {
        if (block) batchRows.push(...extractUniversalContractDeploymentsFromBlock(block as unknown));
      }
      rowsInserted += await insertDeploymentRowsBatch(env.DEPLOY_REGISTRY_DB, batchRows);

      h += chunk;
      indexedBlocks += chunk;
    }

    await writeNextHeight(env.DEPLOY_REGISTRY_DB, h);
    const durationMs = Date.now() - t0;
    await writeSyncTelemetry(env.DEPLOY_REGISTRY_DB, {
      ok: true,
      at: new Date().toISOString(),
      durationMs,
      indexedBlocks,
      rowsInserted,
      nextHeight: h,
      chainTip,
    });
    return { indexedBlocks, rowsInserted, nextHeight: h, chainTip, durationMs };
  } catch (e) {
    const durationMs = Date.now() - t0;
    await writeSyncTelemetry(env.DEPLOY_REGISTRY_DB, {
      ok: false,
      at: new Date().toISOString(),
      durationMs,
      error: String(e).slice(0, 900),
    });
    throw e;
  }
}

async function buildStatus(env: Env): Promise<{
  ingest: { nextHeight: number; defaultFromHeight: number };
  chain: { configured: boolean; tipHeight: number | null; blocksPending: number | null };
  config: { maxBlocksPerTick: number; parallelFetches: number };
  lastSync: SyncTelemetry | null;
}> {
  const fromDefault = parseIntOpt(env.DEPLOY_REGISTRY_FROM_HEIGHT, 0);
  const [nextHeight, lastSync] = await Promise.all([
    readNextHeight(env.DEPLOY_REGISTRY_DB, fromDefault),
    readSyncTelemetry(env.DEPLOY_REGISTRY_DB),
  ]);
  const maxPer = Math.min(256, Math.max(1, parseIntOpt(env.DEPLOY_REGISTRY_MAX_BLOCKS_PER_TICK, 8)));
  const parallel = Math.min(16, Math.max(1, parseIntOpt(env.DEPLOY_REGISTRY_PARALLEL_FETCHES, 6)));
  const baseUrl = String(env.DEPLOY_REGISTRY_RPC_URL || '').trim();
  const config = { maxBlocksPerTick: maxPer, parallelFetches: parallel };
  if (!baseUrl) {
    return {
      ingest: { nextHeight, defaultFromHeight: fromDefault },
      chain: { configured: false, tipHeight: null, blocksPending: null },
      config,
      lastSync,
    };
  }
  const client = createClient({ baseUrl });
  const tip = await client.chainHeight();
  const t = typeof tip === 'number' && Number.isFinite(tip) ? tip : null;
  let blocksPending: number | null = null;
  if (t != null) {
    blocksPending = Math.max(0, t - nextHeight + 1);
  }
  return {
    ingest: { nextHeight, defaultFromHeight: fromDefault },
    chain: { configured: true, tipHeight: t, blocksPending },
    config,
    lastSync,
  };
}

function sseHeaders(): Record<string, string> {
  return {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
    'Access-Control-Allow-Origin': '*',
  };
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    if (req.method === 'OPTIONS') {
      return new Response(null, {
        status: 204,
        headers: {
          ...cors,
          'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
          'Access-Control-Allow-Headers': 'Authorization, Content-Type',
          'Access-Control-Max-Age': '86400',
        },
      });
    }

    if (url.pathname === '/health') {
      const deep = url.searchParams.get('deep') === '1' || url.searchParams.get('deep') === 'true';
      if (!deep) {
        return json({ ok: true, service: 'boing-deploy-registry-indexer' });
      }
      try {
        await env.DEPLOY_REGISTRY_DB.prepare(`SELECT 1 AS ok`).first();
        return json({ ok: true, service: 'boing-deploy-registry-indexer', db: true });
      } catch (e) {
        return json({ ok: false, service: 'boing-deploy-registry-indexer', db: false, error: String(e) }, 503);
      }
    }

    if (url.pathname === '/v1/status' && req.method === 'GET') {
      const status = await buildStatus(env);
      return json({ ok: true, schemaVersion: 2, ...status }, 200, { 'Cache-Control': 'no-store' });
    }

    if (url.pathname === '/v1/sync' && req.method === 'POST') {
      const secret = String(env.DEPLOY_REGISTRY_SYNC_SECRET || '').trim();
      if (!secret) {
        return json({ ok: false, error: 'sync_disabled', hint: 'Set secret DEPLOY_REGISTRY_SYNC_SECRET' }, 503);
      }
      const auth = req.headers.get('Authorization') || '';
      const token = auth.startsWith('Bearer ') ? auth.slice(7).trim() : '';
      if (token !== secret) return json({ ok: false, error: 'unauthorized' }, 401);
      try {
        const out = await syncDeployRegistry(env);
        return json({ ok: true, schemaVersion: 1, ...out });
      } catch (e) {
        return json({ ok: false, error: String(e) }, 500);
      }
    }

    if (req.method === 'GET' && url.pathname.startsWith('/v1/deployments/by-tx/')) {
      const raw = decodeURIComponent(url.pathname.slice('/v1/deployments/by-tx/'.length).trim());
      if (!/^0x[0-9a-f]{64}$/i.test(raw)) {
        return json({ error: 'invalid_tx_id_hex', expected: '0x + 64 hex chars' }, 400);
      }
      const hex = raw.toLowerCase();
      const limit = Math.min(500, Math.max(1, parseInt(url.searchParams.get('limit') || '50', 10)));
      const r = await env.DEPLOY_REGISTRY_DB.prepare(
        `SELECT ${ROW_SELECT} FROM contract_deployments WHERE tx_id_hex = ? ORDER BY id ASC LIMIT ?`,
      )
        .bind(hex, limit)
        .all();
      return json({ rows: r.results || [], schemaVersion: 1, tx_id_hex: hex });
    }

    if (req.method === 'GET' && url.pathname.startsWith('/v1/deployments/by-block/')) {
      const hStr = url.pathname.slice('/v1/deployments/by-block/'.length).trim();
      const bh = parseInt(hStr, 10);
      if (!Number.isFinite(bh) || bh < 0 || String(bh) !== hStr) {
        return json({ error: 'invalid_block_height', expected: 'non-negative integer' }, 400);
      }
      const limit = Math.min(500, Math.max(1, parseInt(url.searchParams.get('limit') || '200', 10)));
      const r = await env.DEPLOY_REGISTRY_DB.prepare(
        `SELECT ${ROW_SELECT} FROM contract_deployments WHERE block_height = ? ORDER BY tx_index ASC, id ASC LIMIT ?`,
      )
        .bind(bh, limit)
        .all();
      return json({ rows: r.results || [], schemaVersion: 1, block_height: bh });
    }

    if (req.method === 'GET' && url.pathname.startsWith('/v1/deployments/by-sender/')) {
      const raw = decodeURIComponent(url.pathname.slice('/v1/deployments/by-sender/'.length).trim());
      if (!/^0x[0-9a-f]{64}$/i.test(raw)) {
        return json({ error: 'invalid_sender_hex', expected: '0x + 64 hex chars' }, 400);
      }
      const sender = raw.toLowerCase();
      const limit = Math.min(500, Math.max(1, parseInt(url.searchParams.get('limit') || '50', 10)));
      const cursor = parseInt(url.searchParams.get('cursor') || '0', 10);
      const cur = Number.isFinite(cursor) ? cursor : 0;
      const r = await env.DEPLOY_REGISTRY_DB.prepare(
        `SELECT ${ROW_SELECT} FROM contract_deployments WHERE sender_hex = ? AND id > ? ORDER BY id ASC LIMIT ?`,
      )
        .bind(sender, cur, limit)
        .all();
      const rows = (r.results || []) as Record<string, unknown>[];
      const lastId = rows.length ? (rows[rows.length - 1]!.id as number) : cur;
      const nextCursor = rows.length === limit ? String(lastId) : null;
      return json({ rows, nextCursor, schemaVersion: 1, sender_hex: sender });
    }

    if (req.method === 'GET' && url.pathname.startsWith('/v1/contract/')) {
      const raw = decodeURIComponent(url.pathname.slice('/v1/contract/'.length).trim());
      if (!/^0x[0-9a-f]{64}$/i.test(raw)) {
        return json({ error: 'invalid_contract_hex', expected: '0x + 64 hex chars' }, 400);
      }
      const hex = raw.toLowerCase();
      const row = await env.DEPLOY_REGISTRY_DB.prepare(
        `SELECT ${ROW_SELECT} FROM contract_deployments WHERE contract_hex = ? LIMIT 1`,
      )
        .bind(hex)
        .first();
      return row ? json(row) : json({ error: 'not_found', contract_hex: hex }, 404);
    }

    if (url.pathname === '/v1/deployments' && req.method === 'GET') {
      const limit = Math.min(500, Math.max(1, parseInt(url.searchParams.get('limit') || '50', 10)));
      const cursor = parseInt(url.searchParams.get('cursor') || '0', 10);
      const cur = Number.isFinite(cursor) ? cursor : 0;
      const r = await env.DEPLOY_REGISTRY_DB.prepare(
        `SELECT id, contract_hex, block_height, tx_index, tx_id_hex, sender_hex, payload_kind, purpose_category, asset_name, asset_symbol
         FROM contract_deployments WHERE id > ? ORDER BY id ASC LIMIT ?`,
      )
        .bind(cur, limit)
        .all();
      const rows = (r.results || []) as Record<string, unknown>[];
      const lastId = rows.length ? (rows[rows.length - 1]!.id as number) : cur;
      const nextCursor = rows.length === limit ? String(lastId) : null;
      return json({ rows, nextCursor, schemaVersion: 1 });
    }

    if (url.pathname === '/v1/deployments/stream' && req.method === 'GET') {
      const sinceRaw = url.searchParams.get('since_id');
      let lastId = sinceRaw != null && sinceRaw !== '' ? parseInt(sinceRaw, 10) : 0;
      if (!Number.isFinite(lastId)) lastId = 0;

      const signal = req.signal;
      const stream = new ReadableStream<Uint8Array>({
        async pull(controller) {
          if (signal.aborted) {
            try {
              controller.close();
            } catch {
              /* ignore */
            }
            return;
          }
          const enc = new TextEncoder();
          let hadRows = false;
          try {
            const r = await env.DEPLOY_REGISTRY_DB.prepare(
              `SELECT ${ROW_SELECT} FROM contract_deployments WHERE id > ? ORDER BY id ASC LIMIT 50`,
            )
              .bind(lastId)
              .all();
            const rows = (r.results || []) as { id: number }[];
            if (rows.length) {
              hadRows = true;
              for (const row of rows) {
                controller.enqueue(enc.encode(`data: ${JSON.stringify(row)}\n\n`));
                lastId = row.id;
              }
            } else {
              controller.enqueue(enc.encode(`event: ping\ndata: {}\n\n`));
            }
          } catch (e) {
            controller.enqueue(
              enc.encode(`event: error\ndata: ${JSON.stringify({ message: String(e) })}\n\n`),
            );
          }
          if (signal.aborted) {
            try {
              controller.close();
            } catch {
              /* ignore */
            }
            return;
          }
          await new Promise((res) => setTimeout(res, hadRows ? 100 : 2500));
        },
      });

      return new Response(stream, { headers: sseHeaders() });
    }

    return json({ error: 'not_found', path: url.pathname }, 404);
  },

  async scheduled(_event: ScheduledEvent, env: Env, ctx: ExecutionContext): Promise<void> {
    ctx.waitUntil(
      (async () => {
        try {
          await syncDeployRegistry(env);
        } catch (e) {
          console.error('deploy-registry sync failed', e);
        }
      })(),
    );
  },
};
