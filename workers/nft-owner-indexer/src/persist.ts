import {
  blockHeightGapRowsForInsert,
  mergeInclusiveHeightRanges,
  type InclusiveHeightRange,
} from 'boing-sdk';
import type { ExtractedNftEvent } from './extract.js';
import { normalizeHex64, zeros32 } from './cors.js';

export type OwnershipRow = {
  collection_hex: string;
  token_id_hex: string;
  owner_hex: string;
  metadata_hash_hex: string | null;
  last_block_height: number;
  last_tx_id: string;
  last_event_kind: string;
  updated_at: number;
};

export type EventRow = {
  block_height: number;
  tx_index: number;
  event_index: number;
  tx_id: string;
  collection_hex: string;
  token_id_hex: string;
  event_kind: string;
  from_owner_hex: string | null;
  to_owner_hex: string | null;
  metadata_hash_hex: string | null;
};

export async function loadCursor(
  db: D1Database,
  chainId: string
): Promise<{ lastIndexedHeight: number; lastHash: string }> {
  const row = await db
    .prepare(
      'SELECT last_committed_height, last_committed_block_hash FROM ingest_cursor WHERE chain_id = ?'
    )
    .bind(chainId)
    .first<{ last_committed_height: number; last_committed_block_hash: string }>();
  return {
    lastIndexedHeight: row?.last_committed_height ?? -1,
    lastHash: row?.last_committed_block_hash ?? zeros32(),
  };
}

export async function upsertIngestCursor(
  db: D1Database,
  chainId: string,
  height: number,
  blockHash: string,
  nowSec: number
): Promise<void> {
  await db
    .prepare(
      `INSERT INTO ingest_cursor (chain_id, last_committed_height, last_committed_block_hash, updated_at)
       VALUES (?, ?, ?, ?)
       ON CONFLICT(chain_id) DO UPDATE SET
         last_committed_height = excluded.last_committed_height,
         last_committed_block_hash = excluded.last_committed_block_hash,
         updated_at = excluded.updated_at`
    )
    .bind(chainId, height, normalizeHex64(blockHash) ?? zeros32(), nowSec)
    .run();
}

export async function getIndexedBlockAtHeight(
  db: D1Database,
  height: number
): Promise<{ height: number; block_hash: string; parent_hash: string } | null> {
  return (
    (await db
      .prepare('SELECT height, block_hash, parent_hash FROM indexed_blocks WHERE height = ?')
      .bind(height)
      .first<{ height: number; block_hash: string; parent_hash: string }>()) ?? null
  );
}

export async function getMaxIndexedHeight(db: D1Database): Promise<number> {
  const row = await db.prepare('SELECT MAX(height) AS mh FROM indexed_blocks').first<{ mh: number | null }>();
  if (row?.mh == null || !Number.isFinite(row.mh)) return -1;
  return row.mh;
}

export async function reconcileCursorToTip(
  db: D1Database,
  chainId: string,
  nowSec: number
): Promise<{ lastHeight: number; lastHash: string }> {
  const maxH = await getMaxIndexedHeight(db);
  if (maxH < 0) {
    await upsertIngestCursor(db, chainId, -1, zeros32(), nowSec);
    return { lastHeight: -1, lastHash: zeros32() };
  }
  const row = await getIndexedBlockAtHeight(db, maxH);
  const tipHash = row?.block_hash ?? zeros32();
  await upsertIngestCursor(db, chainId, maxH, tipHash, nowSec);
  return { lastHeight: maxH, lastHash: tipHash };
}

export async function lookupOwner(
  db: D1Database,
  collectionHex: string,
  tokenIdHex: string
): Promise<string | null> {
  const row = await db
    .prepare('SELECT owner_hex FROM nft_ownership WHERE collection_hex = ? AND token_id_hex = ?')
    .bind(collectionHex, tokenIdHex)
    .first<{ owner_hex: string }>();
  return row?.owner_hex ?? null;
}

export async function deleteHeightAndRebuild(
  db: D1Database,
  height: number,
  nowSec: number
): Promise<number> {
  const affected = await db
    .prepare(
      'SELECT DISTINCT collection_hex, token_id_hex FROM nft_events WHERE block_height = ?'
    )
    .bind(height)
    .all<{ collection_hex: string; token_id_hex: string }>();

  await db.prepare('DELETE FROM nft_events WHERE block_height = ?').bind(height).run();
  await db.prepare('DELETE FROM indexed_blocks WHERE height = ?').bind(height).run();

  const tokens = affected.results ?? [];
  for (const t of tokens) {
    await rebuildOwnershipForToken(db, t.collection_hex, t.token_id_hex, nowSec);
  }
  return tokens.length;
}

/** Replay remaining events for one token into nft_ownership. */
export async function rebuildOwnershipForToken(
  db: D1Database,
  collectionHex: string,
  tokenIdHex: string,
  nowSec: number
): Promise<void> {
  const { results } = await db
    .prepare(
      `SELECT block_height, tx_index, event_index, tx_id, event_kind, to_owner_hex, metadata_hash_hex
       FROM nft_events
       WHERE collection_hex = ? AND token_id_hex = ?
       ORDER BY block_height ASC, tx_index ASC, event_index ASC`
    )
    .bind(collectionHex, tokenIdHex)
    .all<EventRow>();

  let owner: string | null = null;
  let metadata: string | null = null;
  let lastHeight = -1;
  let lastTxId = zeros32();
  let lastKind = 'mint_batch';

  for (const e of results ?? []) {
    if (e.event_kind === 'mint_batch' || e.event_kind === 'transfer_nft') {
      if (e.to_owner_hex) {
        owner = e.to_owner_hex;
        lastHeight = e.block_height;
        lastTxId = e.tx_id;
        lastKind = e.event_kind;
      }
    }
    if (e.event_kind === 'mint_batch' || e.event_kind === 'set_metadata_hash') {
      if (e.metadata_hash_hex) {
        metadata = e.metadata_hash_hex;
        if (e.event_kind === 'set_metadata_hash') {
          lastHeight = e.block_height;
          lastTxId = e.tx_id;
          lastKind = e.event_kind;
        }
      }
    }
  }

  await db
    .prepare('DELETE FROM nft_ownership WHERE collection_hex = ? AND token_id_hex = ?')
    .bind(collectionHex, tokenIdHex)
    .run();

  if (owner) {
    await db
      .prepare(
        `INSERT INTO nft_ownership (
          collection_hex, token_id_hex, owner_hex, metadata_hash_hex,
          last_block_height, last_tx_id, last_event_kind, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
      )
      .bind(collectionHex, tokenIdHex, owner, metadata, lastHeight, lastTxId, lastKind, nowSec)
      .run();
  }
}

export async function persistBlockEvents(
  db: D1Database,
  height: number,
  blockHash: string,
  parentHash: string,
  events: ExtractedNftEvent[],
  nowSec: number
): Promise<void> {
  // Idempotent re-index: capture prior tokens at this height, wipe, write, rebuild union.
  const prior = await db
    .prepare(
      'SELECT DISTINCT collection_hex, token_id_hex FROM nft_events WHERE block_height = ?'
    )
    .bind(height)
    .all<{ collection_hex: string; token_id_hex: string }>();
  const rebuildKeys = new Map<string, { collection_hex: string; token_id_hex: string }>();
  for (const t of prior.results ?? []) {
    rebuildKeys.set(`${t.collection_hex}:${t.token_id_hex}`, t);
  }
  for (const e of events) {
    rebuildKeys.set(`${e.collectionHex}:${e.tokenIdHex}`, {
      collection_hex: e.collectionHex,
      token_id_hex: e.tokenIdHex,
    });
  }

  await db.prepare('DELETE FROM nft_events WHERE block_height = ?').bind(height).run();
  await db.prepare('DELETE FROM indexed_blocks WHERE height = ?').bind(height).run();

  await db
    .prepare(`INSERT INTO indexed_blocks (height, block_hash, parent_hash) VALUES (?, ?, ?)`)
    .bind(height, normalizeHex64(blockHash) ?? zeros32(), normalizeHex64(parentHash) ?? zeros32())
    .run();

  for (const e of events) {
    await db
      .prepare(
        `INSERT INTO nft_events (
          block_height, tx_index, event_index, tx_id, collection_hex, token_id_hex,
          event_kind, from_owner_hex, to_owner_hex, metadata_hash_hex
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
      )
      .bind(
        e.blockHeight,
        e.txIndex,
        e.eventIndex,
        e.txId,
        e.collectionHex,
        e.tokenIdHex,
        e.eventKind,
        e.fromOwnerHex,
        e.toOwnerHex,
        e.metadataHashHex
      )
      .run();
  }

  for (const t of rebuildKeys.values()) {
    await rebuildOwnershipForToken(db, t.collection_hex, t.token_id_hex, nowSec);
  }
}

/** Merged, chain-scoped pruned-height ranges recorded by `indexHeightRange`/backfill. */
export async function loadGapRanges(
  db: D1Database,
  chainId: string
): Promise<InclusiveHeightRange[]> {
  const { results } = await db
    .prepare(
      'SELECT from_height, to_height FROM block_height_gaps WHERE chain_id = ? ORDER BY from_height ASC'
    )
    .bind(chainId)
    .all<{ from_height: number; to_height: number }>();
  return mergeInclusiveHeightRanges(
    (results ?? []).map((r) => ({ fromHeight: r.from_height, toHeight: r.to_height }))
  );
}

/** Wipe and rewrite `block_height_gaps` for `chainId` from merged inclusive ranges. */
export async function replaceGapRanges(
  db: D1Database,
  chainId: string,
  ranges: readonly InclusiveHeightRange[],
  nowSec: number
): Promise<void> {
  await db.prepare('DELETE FROM block_height_gaps WHERE chain_id = ?').bind(chainId).run();
  const rows = blockHeightGapRowsForInsert({ chainId, ranges, recordedAtSec: nowSec });
  if (rows.length === 0) return;
  const stmts = rows.map((r) =>
    db
      .prepare(
        `INSERT INTO block_height_gaps (chain_id, from_height, to_height, reason, recorded_at)
         VALUES (?, ?, ?, ?, ?)`
      )
      .bind(r.chain_id, r.from_height, r.to_height, r.reason, r.recorded_at)
  );
  await db.batch(stmts);
}

export type OwnedNftItem = {
  collection: string;
  tokenId: string;
  owner: string;
  metadataHash: string | null;
  lastBlockHeight: number;
  lastTxId: string;
  lastEventKind: string;
};

export async function listNftsByOwner(
  db: D1Database,
  ownerHex: string,
  opts: {
    limit: number;
    cursorCollection?: string | null;
    cursorTokenId?: string | null;
    collectionFilter?: string | null;
  }
): Promise<{ items: OwnedNftItem[]; nextCursor: string | null }> {
  const limit = Math.min(200, Math.max(1, opts.limit));
  const owner = normalizeHex64(ownerHex);
  if (!owner) return { items: [], nextCursor: null };

  let sql = `SELECT collection_hex, token_id_hex, owner_hex, metadata_hash_hex,
                    last_block_height, last_tx_id, last_event_kind
             FROM nft_ownership WHERE owner_hex = ?`;
  const binds: unknown[] = [owner];

  if (opts.collectionFilter) {
    const c = normalizeHex64(opts.collectionFilter);
    if (c) {
      sql += ' AND collection_hex = ?';
      binds.push(c);
    }
  }

  if (opts.cursorCollection && opts.cursorTokenId) {
    sql +=
      ' AND (collection_hex > ? OR (collection_hex = ? AND token_id_hex > ?))';
    binds.push(opts.cursorCollection, opts.cursorCollection, opts.cursorTokenId);
  }

  sql += ' ORDER BY collection_hex ASC, token_id_hex ASC LIMIT ?';
  binds.push(limit + 1);

  const { results } = await db
    .prepare(sql)
    .bind(...binds)
    .all<{
      collection_hex: string;
      token_id_hex: string;
      owner_hex: string;
      metadata_hash_hex: string | null;
      last_block_height: number;
      last_tx_id: string;
      last_event_kind: string;
    }>();

  const rows = results ?? [];
  const page = rows.slice(0, limit);
  const items: OwnedNftItem[] = page.map((r) => ({
    collection: r.collection_hex,
    tokenId: r.token_id_hex,
    owner: r.owner_hex,
    metadataHash: r.metadata_hash_hex,
    lastBlockHeight: r.last_block_height,
    lastTxId: r.last_tx_id,
    lastEventKind: r.last_event_kind,
  }));

  let nextCursor: string | null = null;
  if (rows.length > limit && page.length > 0) {
    const last = page[page.length - 1]!;
    nextCursor = `${last.collection_hex}:${last.token_id_hex}`;
  }
  return { items, nextCursor };
}

export async function getOwnershipStats(db: D1Database): Promise<{
  ownershipRows: number;
  eventRows: number;
  indexedBlocks: number;
  minHeight: number | null;
  maxHeight: number | null;
}> {
  const o = await db.prepare('SELECT COUNT(*) AS c FROM nft_ownership').first<{ c: number }>();
  const e = await db.prepare('SELECT COUNT(*) AS c FROM nft_events').first<{ c: number }>();
  const b = await db
    .prepare('SELECT COUNT(*) AS c, MIN(height) AS mn, MAX(height) AS mx FROM indexed_blocks')
    .first<{ c: number; mn: number | null; mx: number | null }>();
  return {
    ownershipRows: o?.c ?? 0,
    eventRows: e?.c ?? 0,
    indexedBlocks: b?.c ?? 0,
    minHeight: b?.mn ?? null,
    maxHeight: b?.mx ?? null,
  };
}
