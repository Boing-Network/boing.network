/**
 * D1-backed pool directory: stable cursor pagination over last indexer sync.
 * `directory_pool_events` holds a **snapshot** of parsed pool Log2 rows for the last sync window.
 */
import type {
  BoingClient,
  NativeDexArchivedReceiptLogRow,
  NativeDexIndexerStatsPayload,
  NativeDexIndexedNftOwnerRow,
  NativeDexMaterializedPoolEvent,
} from '@boing-network/boing-sdk';

const MAX_PAGE = 100;
const DEFAULT_PAGE = 20;
const MAX_EVENTS_PAGE = 200;
const DEFAULT_EVENTS_PAGE = 50;

export async function syncDirectoryPoolsFromPayload(db: D1Database, payload: NativeDexIndexerStatsPayload): Promise<void> {
  const batchId = payload.updatedAt;
  const now = new Date().toISOString();
  const pools = payload.pools;

  if (!Array.isArray(pools) || pools.length === 0) {
    await db.prepare(`DELETE FROM directory_pools`).run();
    return;
  }

  const stmts: D1PreparedStatement[] = [];
  for (const p of pools) {
    const ph = String(p.poolHex || '').trim().toLowerCase();
    const ta = String(p.tokenAHex || '').trim().toLowerCase();
    const tb = String(p.tokenBHex || '').trim().toLowerCase();
    if (!/^0x[0-9a-f]{64}$/.test(ph) || !/^0x[0-9a-f]{64}$/.test(ta) || !/^0x[0-9a-f]{64}$/.test(tb)) continue;
    const rowJson = JSON.stringify(p);
    stmts.push(
      db
        .prepare(
          `INSERT INTO directory_pools (pool_hex, token_a_hex, token_b_hex, sync_batch_id, updated_at, row_json)
           VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT(pool_hex) DO UPDATE SET
             token_a_hex = excluded.token_a_hex,
             token_b_hex = excluded.token_b_hex,
             sync_batch_id = excluded.sync_batch_id,
             updated_at = excluded.updated_at,
             row_json = excluded.row_json`,
        )
        .bind(ph, ta, tb, batchId, now, rowJson),
    );
  }

  if (stmts.length === 0) {
    await db.prepare(`DELETE FROM directory_pools`).run();
    return;
  }

  const CHUNK = 80;
  for (let i = 0; i < stmts.length; i += CHUNK) {
    await db.batch(stmts.slice(i, i + CHUNK));
  }

  await db.prepare(`DELETE FROM directory_pools WHERE sync_batch_id != ?`).bind(batchId).run();
}

export async function syncDirectoryPoolEventsFromPayload(
  db: D1Database,
  batchId: string,
  events: readonly NativeDexMaterializedPoolEvent[],
): Promise<void> {
  const now = new Date().toISOString();

  if (events.length === 0) {
    await db.prepare(`DELETE FROM directory_pool_events`).run();
    return;
  }

  const stmts: D1PreparedStatement[] = [];
  for (const ev of events) {
    const rowJson = JSON.stringify(ev);
    stmts.push(
      db
        .prepare(
          `INSERT INTO directory_pool_events (pool_hex, block_height, tx_id, log_index, sync_batch_id, updated_at, kind, caller_hex, row_json)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          ev.poolHex,
          ev.blockHeight,
          ev.txId,
          ev.logIndex,
          batchId,
          now,
          ev.kind,
          ev.callerHex,
          rowJson,
        ),
    );
  }

  const CHUNK = 80;
  for (let i = 0; i < stmts.length; i += CHUNK) {
    await db.batch(stmts.slice(i, i + CHUNK));
  }

  await db.prepare(`DELETE FROM directory_pool_events WHERE sync_batch_id != ?`).bind(batchId).run();
}

export type DirectoryIndexerTipRow = {
  tip_height: number | null;
  tip_block_hash: string | null;
  parent_block_hash: string | null;
};

export async function readDirectoryIndexerTip(db: D1Database): Promise<DirectoryIndexerTipRow | null> {
  try {
    const tip = await db
      .prepare(
        `SELECT tip_height, tip_block_hash, parent_block_hash FROM directory_indexer_tip WHERE id = 1`,
      )
      .first<DirectoryIndexerTipRow>();
    return tip ?? null;
  } catch {
    return null;
  }
}

/** Wipe pool event snapshot (used when canonical block hash at a stored height no longer matches). */
export async function deleteAllDirectoryPoolEvents(db: D1Database): Promise<void> {
  await db.prepare(`DELETE FROM directory_pool_events`).run();
}

/** Wipe optional **`directory_receipt_log`** snapshot (same reorg path as pool events). */
export async function deleteAllDirectoryReceiptLogs(db: D1Database): Promise<void> {
  try {
    await db.prepare(`DELETE FROM directory_receipt_log`).run();
  } catch {
    /* table missing before migration 0006 */
  }
}

/**
 * If a row exists for height **H** with hash **H₀**, and **`getBlockByHeight(H)`** now returns **H₁ ≠ H₀**,
 * delete all **`directory_pool_events`** rows (next sync will refill the window).
 */
export async function invalidateDirectoryPoolEventsIfTipReorged(db: D1Database, client: BoingClient): Promise<boolean> {
  const tip = await readDirectoryIndexerTip(db);
  if (tip?.tip_height == null || tip.tip_block_hash == null || !/^0x[0-9a-f]{64}$/i.test(tip.tip_block_hash)) {
    return false;
  }
  try {
    const blk = await client.getBlockByHeight(tip.tip_height, false);
    const h = blk?.hash;
    const nowHash = typeof h === 'string' && /^0x[0-9a-f]{64}$/i.test(h) ? h.toLowerCase() : null;
    if (nowHash == null) return false;
    if (nowHash === tip.tip_block_hash.toLowerCase()) return false;
    await deleteAllDirectoryPoolEvents(db);
    await deleteAllDirectoryReceiptLogs(db);
    return true;
  } catch {
    return false;
  }
}

/** Chain tip at last successful event sync — for client skew checks; shallow reorg detection uses this row. */
export async function syncDirectoryIndexerTip(
  db: D1Database,
  batchId: string,
  tipHeight: number | null,
  tipBlockHash: string | null,
  parentBlockHash: string | null,
): Promise<void> {
  const now = new Date().toISOString();
  try {
    await db
      .prepare(
        `INSERT INTO directory_indexer_tip (id, tip_height, tip_block_hash, parent_block_hash, sync_batch_id, updated_at)
         VALUES (1, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           tip_height = excluded.tip_height,
           tip_block_hash = excluded.tip_block_hash,
           parent_block_hash = excluded.parent_block_hash,
           sync_batch_id = excluded.sync_batch_id,
           updated_at = excluded.updated_at`,
      )
      .bind(tipHeight, tipBlockHash, parentBlockHash, batchId, now)
      .run();
  } catch {
    await db
      .prepare(
        `INSERT INTO directory_indexer_tip (id, tip_height, tip_block_hash, sync_batch_id, updated_at)
         VALUES (1, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           tip_height = excluded.tip_height,
           tip_block_hash = excluded.tip_block_hash,
           sync_batch_id = excluded.sync_batch_id,
           updated_at = excluded.updated_at`,
      )
      .bind(tipHeight, tipBlockHash, batchId, now)
      .run();
  }
}

export async function getDirectoryMeta(db: D1Database): Promise<{
  poolCount: number;
  eventCount: number;
  nftOwnerRowCount: number;
  receiptLogCount: number;
  latestSyncBatch: string | null;
  indexedTipHeight: number | null;
  indexedTipBlockHash: string | null;
  indexedParentBlockHash: string | null;
}> {
  const row = await db
    .prepare(
      `SELECT
         (SELECT COUNT(*) FROM directory_pools) AS c,
         (SELECT COUNT(*) FROM directory_pool_events) AS ec,
         (SELECT MAX(sync_batch_id) FROM directory_pools) AS batch`,
    )
    .first<{ c: number; ec: number; batch: string | null }>();

  let indexedTipHeight: number | null = null;
  let indexedTipBlockHash: string | null = null;
  let indexedParentBlockHash: string | null = null;
  try {
    const tip = await db
      .prepare(
        `SELECT tip_height, tip_block_hash, parent_block_hash FROM directory_indexer_tip WHERE id = 1`,
      )
      .first<{
        tip_height: number | null;
        tip_block_hash: string | null;
        parent_block_hash: string | null;
      }>();
    if (tip != null) {
      indexedTipHeight = typeof tip.tip_height === 'number' && Number.isFinite(tip.tip_height) ? tip.tip_height : null;
      indexedTipBlockHash =
        typeof tip.tip_block_hash === 'string' && /^0x[0-9a-f]{64}$/i.test(tip.tip_block_hash)
          ? tip.tip_block_hash.toLowerCase()
          : null;
      indexedParentBlockHash =
        typeof tip.parent_block_hash === 'string' && /^0x[0-9a-f]{64}$/i.test(tip.parent_block_hash)
          ? tip.parent_block_hash.toLowerCase()
          : null;
    }
  } catch {
    /* directory_indexer_tip missing before migration 0003 */
  }

  let nftOwnerRowCount = 0;
  try {
    const n = await db.prepare(`SELECT COUNT(*) AS c FROM directory_nft_owner`).first<{ c: number }>();
    nftOwnerRowCount = Number(n?.c ?? 0);
  } catch {
    nftOwnerRowCount = 0;
  }

  let receiptLogCount = 0;
  try {
    const r = await db.prepare(`SELECT COUNT(*) AS c FROM directory_receipt_log`).first<{ c: number }>();
    receiptLogCount = Number(r?.c ?? 0);
  } catch {
    receiptLogCount = 0;
  }

  return {
    poolCount: Number(row?.c ?? 0),
    eventCount: Number(row?.ec ?? 0),
    nftOwnerRowCount: Number.isFinite(nftOwnerRowCount) ? nftOwnerRowCount : 0,
    receiptLogCount: Number.isFinite(receiptLogCount) ? receiptLogCount : 0,
    latestSyncBatch: row?.batch ?? null,
    indexedTipHeight,
    indexedTipBlockHash,
    indexedParentBlockHash,
  };
}

export async function listDirectoryPoolsPage(
  db: D1Database,
  url: URL,
): Promise<{
  limit: number;
  cursor: string | null;
  nextCursor: string | null;
  hasMore: boolean;
  pools: unknown[];
}> {
  let limit = parseInt(url.searchParams.get('limit') || String(DEFAULT_PAGE), 10);
  if (!Number.isFinite(limit) || limit < 1) limit = DEFAULT_PAGE;
  if (limit > MAX_PAGE) limit = MAX_PAGE;

  let cursor = (url.searchParams.get('cursor') || '').trim().toLowerCase();
  if (cursor && !/^0x[0-9a-f]{64}$/.test(cursor)) {
    throw new Error('cursor must be 0x + 64 hex chars');
  }

  const take = limit + 1;
  const { results } = await db
    .prepare(
      `SELECT row_json FROM directory_pools
       WHERE pool_hex > ?
       ORDER BY pool_hex ASC
       LIMIT ?`,
    )
    .bind(cursor || '', take)
    .all<{ row_json: string }>();

  const parsed: unknown[] = [];
  for (const r of results ?? []) {
    try {
      parsed.push(JSON.parse(r.row_json));
    } catch {
      /* skip */
    }
  }

  const hasMore = parsed.length > limit;
  const page = hasMore ? parsed.slice(0, limit) : parsed;
  let nextCursor: string | null = null;
  if (hasMore && page.length > 0) {
    const last = page[page.length - 1] as { poolHex?: string };
    const lh = String(last?.poolHex || '').trim().toLowerCase();
    nextCursor = lh || null;
  }

  return {
    limit,
    cursor: cursor || null,
    nextCursor,
    hasMore,
    pools: page,
  };
}

export async function listDirectoryPoolEventsPage(
  db: D1Database,
  poolHexLower: string,
  url: URL,
): Promise<{
  poolHex: string;
  limit: number;
  cursor: string | null;
  nextCursor: string | null;
  hasMore: boolean;
  events: unknown[];
}> {
  let limit = parseInt(url.searchParams.get('limit') || String(DEFAULT_EVENTS_PAGE), 10);
  if (!Number.isFinite(limit) || limit < 1) limit = DEFAULT_EVENTS_PAGE;
  if (limit > MAX_EVENTS_PAGE) limit = MAX_EVENTS_PAGE;

  const cursorRaw = (url.searchParams.get('cursor') || '').trim();
  let cursorId: number | null = null;
  if (cursorRaw !== '') {
    const n = parseInt(cursorRaw, 10);
    if (!Number.isFinite(n) || n < 1) {
      throw new Error('cursor must be a positive integer row id');
    }
    cursorId = n;
  }

  const take = limit + 1;
  const { results } = await db
    .prepare(
      `SELECT id, row_json FROM directory_pool_events
       WHERE pool_hex = ? AND (? IS NULL OR id < ?)
       ORDER BY id DESC
       LIMIT ?`,
    )
    .bind(poolHexLower, cursorId, cursorId, take)
    .all<{ id: number; row_json: string }>();

  const rows = results ?? [];
  const withIds: { id: number; event: unknown }[] = [];
  for (const r of rows) {
    try {
      withIds.push({ id: r.id, event: JSON.parse(r.row_json) });
    } catch {
      /* skip */
    }
  }

  const hasMore = withIds.length > limit;
  const slice = hasMore ? withIds.slice(0, limit) : withIds;
  const page = slice.map((x) => x.event);
  let nextCursor: string | null = null;
  if (hasMore && slice.length > 0) {
    nextCursor = String(slice[slice.length - 1]!.id);
  }

  return {
    poolHex: poolHexLower,
    limit,
    cursor: cursorRaw || null,
    nextCursor,
    hasMore,
    events: page,
  };
}

export async function listDirectoryUserEventsPage(
  db: D1Database,
  callerHexLower: string,
  url: URL,
): Promise<{
  callerHex: string;
  limit: number;
  cursor: string | null;
  nextCursor: string | null;
  hasMore: boolean;
  events: unknown[];
}> {
  let limit = parseInt(url.searchParams.get('limit') || String(DEFAULT_EVENTS_PAGE), 10);
  if (!Number.isFinite(limit) || limit < 1) limit = DEFAULT_EVENTS_PAGE;
  if (limit > MAX_EVENTS_PAGE) limit = MAX_EVENTS_PAGE;

  const cursorRaw = (url.searchParams.get('cursor') || '').trim();
  let cursorId: number | null = null;
  if (cursorRaw !== '') {
    const n = parseInt(cursorRaw, 10);
    if (!Number.isFinite(n) || n < 1) {
      throw new Error('cursor must be a positive integer row id');
    }
    cursorId = n;
  }

  const take = limit + 1;
  const { results } = await db
    .prepare(
      `SELECT id, row_json FROM directory_pool_events
       WHERE caller_hex = ? AND (? IS NULL OR id < ?)
       ORDER BY id DESC
       LIMIT ?`,
    )
    .bind(callerHexLower, cursorId, cursorId, take)
    .all<{ id: number; row_json: string }>();

  const rows = results ?? [];
  const withIds: { id: number; event: unknown }[] = [];
  for (const r of rows) {
    try {
      withIds.push({ id: r.id, event: JSON.parse(r.row_json) });
    } catch {
      /* skip */
    }
  }

  const hasMore = withIds.length > limit;
  const slice = hasMore ? withIds.slice(0, limit) : withIds;
  const page = slice.map((x) => x.event);
  let nextCursor: string | null = null;
  if (hasMore && slice.length > 0) {
    nextCursor = String(slice[slice.length - 1]!.id);
  }

  return {
    callerHex: callerHexLower,
    limit,
    cursor: cursorRaw || null,
    nextCursor,
    hasMore,
    events: page,
  };
}

export async function syncDirectoryNftOwnersFromRows(
  db: D1Database,
  batchId: string,
  rows: readonly NativeDexIndexedNftOwnerRow[],
): Promise<void> {
  const now = new Date().toISOString();
  if (rows.length === 0) {
    try {
      await db.prepare(`DELETE FROM directory_nft_owner`).run();
    } catch {
      /* table missing before migration 0005 */
    }
    return;
  }

  const stmts: D1PreparedStatement[] = [];
  for (const r of rows) {
    const rowJson = JSON.stringify(r);
    stmts.push(
      db
        .prepare(
          `INSERT INTO directory_nft_owner (contract_hex, token_id_dec, owner_hex, last_block_height, tx_id, log_index, sync_batch_id, updated_at, row_json)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(contract_hex, token_id_dec) DO UPDATE SET
             owner_hex = excluded.owner_hex,
             last_block_height = excluded.last_block_height,
             tx_id = excluded.tx_id,
             log_index = excluded.log_index,
             sync_batch_id = excluded.sync_batch_id,
             updated_at = excluded.updated_at,
             row_json = excluded.row_json`,
        )
        .bind(
          r.contractHex,
          r.tokenIdDec,
          r.ownerHex,
          r.lastBlockHeight,
          r.txId,
          r.logIndex,
          batchId,
          now,
          rowJson,
        ),
    );
  }

  const CHUNK = 80;
  for (let i = 0; i < stmts.length; i += CHUNK) {
    await db.batch(stmts.slice(i, i + CHUNK));
  }

  try {
    await db.prepare(`DELETE FROM directory_nft_owner WHERE sync_batch_id != ?`).bind(batchId).run();
  } catch {
    /* missing migration */
  }
}

export async function listDirectoryNftPositionsPage(
  db: D1Database,
  contractHexLower: string,
  ownerHexLower: string,
  url: URL,
): Promise<{
  contractHex: string;
  ownerHex: string;
  limit: number;
  cursor: string | null;
  nextCursor: string | null;
  hasMore: boolean;
  positions: unknown[];
}> {
  let limit = parseInt(url.searchParams.get('limit') || String(DEFAULT_EVENTS_PAGE), 10);
  if (!Number.isFinite(limit) || limit < 1) limit = DEFAULT_EVENTS_PAGE;
  if (limit > MAX_EVENTS_PAGE) limit = MAX_EVENTS_PAGE;

  const cursorRaw = (url.searchParams.get('cursor') || '').trim();
  let cursorId: number | null = null;
  if (cursorRaw !== '') {
    const n = parseInt(cursorRaw, 10);
    if (!Number.isFinite(n) || n < 1) {
      throw new Error('cursor must be a positive integer row id');
    }
    cursorId = n;
  }

  const take = limit + 1;
  const { results } = await db
    .prepare(
      `SELECT id, row_json FROM directory_nft_owner
       WHERE contract_hex = ? AND owner_hex = ? AND (? IS NULL OR id < ?)
       ORDER BY id DESC
       LIMIT ?`,
    )
    .bind(contractHexLower, ownerHexLower, cursorId, cursorId, take)
    .all<{ id: number; row_json: string }>();

  const rows = results ?? [];
  const withIds: { id: number; row: unknown }[] = [];
  for (const r of rows) {
    try {
      withIds.push({ id: r.id, row: JSON.parse(r.row_json) });
    } catch {
      /* skip */
    }
  }

  const hasMore = withIds.length > limit;
  const slice = hasMore ? withIds.slice(0, limit) : withIds;
  const page = slice.map((x) => x.row);
  let nextCursor: string | null = null;
  if (hasMore && slice.length > 0) {
    nextCursor = String(slice[slice.length - 1]!.id);
  }

  return {
    contractHex: contractHexLower,
    ownerHex: ownerHexLower,
    limit,
    cursor: cursorRaw || null,
    nextCursor,
    hasMore,
    positions: page,
  };
}

export async function syncDirectoryReceiptLogsFromRows(
  db: D1Database,
  batchId: string,
  rows: readonly NativeDexArchivedReceiptLogRow[],
): Promise<void> {
  const now = new Date().toISOString();
  if (rows.length === 0) {
    try {
      await db.prepare(`DELETE FROM directory_receipt_log`).run();
    } catch {
      /* table missing before migration 0006 */
    }
    return;
  }

  const stmts: D1PreparedStatement[] = [];
  for (const r of rows) {
    const rowJson = JSON.stringify(r);
    stmts.push(
      db
        .prepare(
          `INSERT INTO directory_receipt_log (block_height, block_hash, tx_id, log_index, topic0_hex, topics_json, data_hex, sync_batch_id, updated_at, row_json)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          r.blockHeight,
          r.blockHash,
          r.txId,
          r.logIndex,
          r.topic0Hex,
          r.topicsJson,
          r.dataHex,
          batchId,
          now,
          rowJson,
        ),
    );
  }

  const CHUNK = 80;
  for (let i = 0; i < stmts.length; i += CHUNK) {
    await db.batch(stmts.slice(i, i + CHUNK));
  }

  try {
    await db.prepare(`DELETE FROM directory_receipt_log WHERE sync_batch_id != ?`).bind(batchId).run();
  } catch {
    /* missing migration */
  }
}

/**
 * Paginated **`directory_receipt_log`** rows (newest **`id`** first).
 * Optional **`topic0`** query: `0x` + 64 hex (log **`topic0`** filter).
 */
export async function listDirectoryReceiptLogsPage(
  db: D1Database,
  url: URL,
): Promise<{
  limit: number;
  cursor: string | null;
  nextCursor: string | null;
  hasMore: boolean;
  topic0: string | null;
  logs: unknown[];
}> {
  let limit = parseInt(url.searchParams.get('limit') || String(DEFAULT_EVENTS_PAGE), 10);
  if (!Number.isFinite(limit) || limit < 1) limit = DEFAULT_EVENTS_PAGE;
  if (limit > MAX_EVENTS_PAGE) limit = MAX_EVENTS_PAGE;

  const topic0Raw = (url.searchParams.get('topic0') || '').trim().toLowerCase();
  const topic0Filter =
    topic0Raw !== '' && /^0x[0-9a-f]{64}$/.test(topic0Raw) ? topic0Raw : null;
  if (topic0Raw !== '' && topic0Filter == null) {
    throw new Error('topic0 must be empty or 0x + 64 hex chars');
  }

  const cursorRaw = (url.searchParams.get('cursor') || '').trim();
  let cursorId: number | null = null;
  if (cursorRaw !== '') {
    const n = parseInt(cursorRaw, 10);
    if (!Number.isFinite(n) || n < 1) {
      throw new Error('cursor must be a positive integer row id');
    }
    cursorId = n;
  }

  const take = limit + 1;
  const { results } = await db
    .prepare(
      `SELECT id, row_json FROM directory_receipt_log
       WHERE (? IS NULL OR topic0_hex = ?) AND (? IS NULL OR id < ?)
       ORDER BY id DESC
       LIMIT ?`,
    )
    .bind(topic0Filter, topic0Filter, cursorId, cursorId, take)
    .all<{ id: number; row_json: string }>();

  const rows = results ?? [];
  const withIds: { id: number; row: unknown }[] = [];
  for (const r of rows) {
    try {
      withIds.push({ id: r.id, row: JSON.parse(r.row_json) });
    } catch {
      /* skip */
    }
  }

  const hasMore = withIds.length > limit;
  const slice = hasMore ? withIds.slice(0, limit) : withIds;
  const page = slice.map((x) => x.row);
  let nextCursor: string | null = null;
  if (hasMore && slice.length > 0) {
    nextCursor = String(slice[slice.length - 1]!.id);
  }

  return {
    limit,
    cursor: cursorRaw || null,
    nextCursor,
    hasMore,
    topic0: topic0Filter,
    logs: page,
  };
}
