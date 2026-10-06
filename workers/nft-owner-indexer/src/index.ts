/**
 * Cloudflare Worker: durable reference-NFT ownership index.
 *
 * Read API (Express / Observer / partners):
 * - GET /v1/nfts/by-owner?owner=0x…64[&limit=&cursor=&collection=]
 * - GET /v1/nfts/item?collection=&tokenId=
 * - GET /v1/meta | /v1/sync | /health | /
 * - POST /v1/sync  (Authorization: Bearer NFT_OWNER_SYNC_SECRET) — one ingest tick
 * - POST /v1/backfill?from=&to=  (same bearer) — force-index a height range
 *
 * Cron: catch-up behind finalized tip; OBS-1-style reorg rewind on tip hash mismatch.
 */

import {
  createClient,
  fetchBlocksWithReceiptsForHeightRange,
  nextContiguousIndexedHeightAfterOmittedFetch,
  planIndexerCatchUp,
  planIndexerChainTipsWithFallback,
  summarizeIndexerFetchGaps,
  type InclusiveHeightRange,
} from 'boing-sdk';
import { handleOptions, jsonRes, normalizeHex64, parseCorsOrigins, zeros32 } from './cors.js';
import { extractNftEventsFromBlock } from './extract.js';
import { reconcileGapRangesForWindow } from './gaps.js';
import {
  getOwnershipStats,
  listNftsByOwner,
  loadCursor,
  loadGapRanges,
  lookupOwner,
  persistBlockEvents,
  replaceGapRanges,
  upsertIngestCursor,
} from './persist.js';
import { parseMaxReorgRewindSteps, rewindStaleTipIfNeeded } from './reorg.js';

export interface Env {
  NFT_OWNER_DB: D1Database;
  BOING_RPC_URL: string;
  BOING_CHAIN_ID?: string;
  BOING_MAX_BLOCKS_PER_TICK?: string;
  BOING_MAX_CONCURRENT?: string;
  BOING_OMIT_MISSING?: string;
  BOING_CORS_ORIGINS?: string;
  BOING_APP_VERSION?: string;
  BOING_MAX_REORG_REWIND_STEPS?: string;
  BOING_DISABLE_REORG_REWIND?: string;
  BOING_BACKFILL_FROM_HEIGHT?: string;
  /** Bearer secret for POST /v1/sync and /v1/backfill */
  NFT_OWNER_SYNC_SECRET?: string;
}

const APP_VERSION_DEFAULT = '0.1.0';

function parsePositiveInt(raw: string | undefined, fallback: number): number {
  if (raw == null || !String(raw).trim()) return fallback;
  const n = parseInt(String(raw), 10);
  return Number.isFinite(n) && n > 0 ? n : fallback;
}

function authorized(req: Request, env: Env): boolean {
  const secret = env.NFT_OWNER_SYNC_SECRET;
  if (!secret) return false;
  const h = req.headers.get('Authorization') ?? '';
  return h === `Bearer ${secret}`;
}

/**
 * Index `[fromHeight, toHeight]` (inclusive), persist events/ownership for every present block,
 * then reconcile `block_height_gaps` for pruned heights and — only when this window directly
 * continues the **global** ingest cursor — advance that cursor to the longest contiguous prefix
 * actually indexed. Safe to call for historical/out-of-band windows (e.g. `POST /v1/backfill`
 * targeting an older gap): events are still persisted, gaps still reconciled, but the global
 * cursor is left untouched so it never moves backward or skips ahead of unindexed heights.
 */
async function indexHeightRange(
  env: Env,
  fromHeight: number,
  toHeight: number
): Promise<{
  indexed: number;
  events: number;
  fromHeight: number;
  toHeight: number;
  /** Highest height such that `[fromHeight, H]` is fully indexed with no pruned gap. */
  contiguousThrough: number;
  /** Whether the global ingest cursor was advanced (only when `fromHeight` continues it). */
  cursorAdvanced: boolean;
  /** Newly-discovered pruned ranges inside this window (already merged into `block_height_gaps`). */
  newGapRanges: InclusiveHeightRange[];
  gapRangeCount: number;
  aborted: boolean;
}> {
  const client = createClient(env.BOING_RPC_URL);
  const db = env.NFT_OWNER_DB;
  const maxConcurrent = parsePositiveInt(env.BOING_MAX_CONCURRENT, 4);
  const omitMissing = env.BOING_OMIT_MISSING === '1' || env.BOING_OMIT_MISSING === 'true';
  const nowSec = Math.floor(Date.now() / 1000);
  const chainId = env.BOING_CHAIN_ID ?? 'boing-testnet';

  const bundles = await fetchBlocksWithReceiptsForHeightRange(client, fromHeight, toHeight, {
    maxConcurrent,
    onMissingBlock: omitMissing ? 'omit' : 'throw',
  });

  const { lastHash: tipHash, lastIndexedHeight: tipHeight } = await loadCursor(db, chainId);
  const windowContinuesGlobalCursor = fromHeight === tipHeight + 1;

  let eventsTotal = 0;
  let lastHash = tipHeight >= 0 ? tipHash : zeros32();
  let lastHeight = tipHeight >= 0 ? tipHeight : fromHeight - 1;
  const processedHeights: number[] = [];
  const heightHashMap = new Map<number, string>();
  let aborted = false;

  for (const bundle of bundles) {
    const height = bundle.height;
    const block = bundle.block as {
      hash?: string;
      header?: { parent_hash?: string };
    };
    const blockHash = normalizeHex64(block.hash) ?? zeros32();
    const parentHash = normalizeHex64(block.header?.parent_hash) ?? zeros32();

    if (lastHeight >= 0 && height === lastHeight + 1) {
      const expectedParent = lastHash;
      if (parentHash !== expectedParent && expectedParent !== zeros32()) {
        console.log(
          JSON.stringify({
            ok: false,
            action: 'ingest_abort_parent_mismatch',
            height,
            parentHash,
            expectedParent,
          })
        );
        aborted = true;
        break;
      }
    }

    const primed = await extractWithOwnerLookup(db, bundle.block, height);
    await persistBlockEvents(db, height, blockHash, parentHash, primed, nowSec);
    eventsTotal += primed.length;
    lastHash = blockHash;
    lastHeight = height;
    processedHeights.push(height);
    heightHashMap.set(height, blockHash);
  }

  const attemptedThrough = aborted
    ? (processedHeights[processedHeights.length - 1] ?? fromHeight - 1)
    : toHeight;

  let contiguousThrough = fromHeight - 1;
  let newGapRanges: InclusiveHeightRange[] = [];
  if (attemptedThrough >= fromHeight) {
    const fetchGaps = summarizeIndexerFetchGaps(fromHeight, attemptedThrough, processedHeights);
    contiguousThrough = nextContiguousIndexedHeightAfterOmittedFetch(fromHeight - 1, fetchGaps);
    newGapRanges = fetchGaps.missingHeightRangesInclusive;

    const stored = await loadGapRanges(db, chainId);
    const reconciled = reconcileGapRangesForWindow(
      stored,
      { fromHeight, toHeight: attemptedThrough },
      newGapRanges
    );
    await replaceGapRanges(db, chainId, reconciled, nowSec);
  }

  let cursorAdvanced = false;
  if (windowContinuesGlobalCursor && contiguousThrough > fromHeight - 1) {
    const hash = heightHashMap.get(contiguousThrough);
    if (hash) {
      await upsertIngestCursor(db, chainId, contiguousThrough, hash, nowSec);
      cursorAdvanced = true;
    }
  }

  const gapRangeCount = (await loadGapRanges(db, chainId)).length;

  return {
    indexed: processedHeights.length,
    events: eventsTotal,
    fromHeight,
    toHeight: attemptedThrough,
    contiguousThrough,
    cursorAdvanced,
    newGapRanges,
    gapRangeCount,
    aborted,
  };
}

/**
 * Prime D1 owners for transfer token ids, then extract with in-block overlay + primed cache.
 */
async function extractWithOwnerLookup(
  db: D1Database,
  block: unknown,
  height: number
): Promise<ReturnType<typeof extractNftEventsFromBlock>> {
  const cache = new Map<string, string | null>();

  // Discover transfer targets with empty prior so we know which tokens to hydrate from D1.
  const preliminary = extractNftEventsFromBlock(block, height, () => null);
  for (const e of preliminary) {
    if (e.eventKind === 'transfer_nft') {
      const k = `${e.collectionHex}:${e.tokenIdHex}`;
      if (!cache.has(k)) {
        cache.set(k, await lookupOwner(db, e.collectionHex, e.tokenIdHex));
      }
    }
  }

  return extractNftEventsFromBlock(block, height, (c, t) => cache.get(`${c}:${t}`) ?? null);
}

async function runIngestTick(env: Env): Promise<Record<string, unknown>> {
  const chainId = env.BOING_CHAIN_ID ?? 'boing-testnet';
  const db = env.NFT_OWNER_DB;
  const client = createClient(env.BOING_RPC_URL);
  const maxBlocksPerTick = parsePositiveInt(env.BOING_MAX_BLOCKS_PER_TICK, 16);
  const nowSec = Math.floor(Date.now() / 1000);

  let { lastIndexedHeight, lastHash } = await loadCursor(db, chainId);

  // Cold start: optionally jump from BOING_BACKFILL_FROM_HEIGHT (default 0 → start at genesis).
  if (lastIndexedHeight < 0) {
    const fromRaw = env.BOING_BACKFILL_FROM_HEIGHT;
    if (fromRaw != null && String(fromRaw).trim() !== '') {
      const from = parseInt(String(fromRaw), 10);
      if (Number.isFinite(from) && from > 0) {
        // Cursor at from-1 means next plan starts at `from`.
        lastIndexedHeight = from - 1;
        lastHash = zeros32();
        await upsertIngestCursor(db, chainId, lastIndexedHeight, lastHash, nowSec);
      }
    }
  }

  const disableRewind =
    env.BOING_DISABLE_REORG_REWIND === '1' || env.BOING_DISABLE_REORG_REWIND === 'true';
  const afterRewind = await rewindStaleTipIfNeeded(
    db,
    client,
    chainId,
    lastIndexedHeight,
    lastHash,
    nowSec,
    {
      disabled: disableRewind,
      maxSteps: parseMaxReorgRewindSteps(env.BOING_MAX_REORG_REWIND_STEPS),
    }
  );
  lastIndexedHeight = afterRewind.lastIndexedHeight;
  lastHash = afterRewind.lastHash;

  const plan = await planIndexerCatchUp(client, lastIndexedHeight, { maxBlocksPerTick });
  if (plan == null) {
    return {
      ok: true,
      action: 'idle',
      chainId,
      lastIndexedHeight,
      rewindHeights: afterRewind.rewindHeights,
    };
  }

  const result = await indexHeightRange(env, plan.fromHeight, plan.toHeight);
  return {
    ok: true,
    action: 'fetch',
    chainId,
    ...result,
    planFrom: plan.fromHeight,
    planTo: plan.toHeight,
    rewindHeights: afterRewind.rewindHeights,
  };
}

export default {
  async scheduled(_controller: ScheduledController, env: Env, _ctx: ExecutionContext): Promise<void> {
    try {
      const result = await runIngestTick(env);
      console.log(JSON.stringify(result));
    } catch (e) {
      console.error(
        JSON.stringify({
          ok: false,
          action: 'ingest_error',
          error: e instanceof Error ? e.message : String(e),
        })
      );
    }
  },

  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const path = url.pathname.replace(/\/+$/, '') || '/';
    const cors = parseCorsOrigins(env.BOING_CORS_ORIGINS);
    const origin = req.headers.get('Origin');

    if (req.method === 'OPTIONS') return handleOptions(cors, origin);

    if (path === '/health') {
      return new Response('ok', {
        status: 200,
        headers: { 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' },
      });
    }

    if (path === '/' && req.method === 'GET') {
      return jsonRes(
        {
          service: 'boing-nft-owner-indexer',
          version: env.BOING_APP_VERSION ?? APP_VERSION_DEFAULT,
          endpoints: [
            'GET /v1/nfts/by-owner?owner=',
            'GET /v1/nfts/item?collection=&tokenId=',
            'GET /v1/meta',
            'GET /v1/gaps',
            'GET /v1/sync',
            'POST /v1/sync',
            'POST /v1/backfill?from=&to=',
            'GET /health',
          ],
        },
        200,
        cors,
        origin
      );
    }

    if ((path === '/v1/meta' || path === '/v1/sync') && req.method === 'GET') {
      const chainId = env.BOING_CHAIN_ID ?? 'boing-testnet';
      const cursor = await loadCursor(env.NFT_OWNER_DB, chainId);
      const stats = await getOwnershipStats(env.NFT_OWNER_DB);
      const gapRanges = await loadGapRanges(env.NFT_OWNER_DB, chainId);
      let tips: { headHeight?: number; finalizedHeight?: number } = {};
      try {
        const client = createClient(env.BOING_RPC_URL);
        const planned = await planIndexerChainTipsWithFallback(client);
        tips = {
          headHeight: planned.tips.headHeight,
          finalizedHeight: planned.tips.finalizedHeight,
        };
      } catch {
        /* ignore tip errors on meta */
      }
      const lagVsFinalized =
        tips.finalizedHeight != null && cursor.lastIndexedHeight >= 0
          ? tips.finalizedHeight - cursor.lastIndexedHeight
          : null;
      return jsonRes(
        {
          service: 'boing-nft-owner-indexer',
          version: env.BOING_APP_VERSION ?? APP_VERSION_DEFAULT,
          chainId,
          lastCommittedHeight: cursor.lastIndexedHeight,
          lastCommittedBlockHash: cursor.lastHash,
          rpcHeadHeight: tips.headHeight ?? null,
          rpcFinalizedHeight: tips.finalizedHeight ?? null,
          lagVsFinalized,
          stats,
          gapRanges,
          gapRangeCount: gapRanges.length,
          note: 'Reference NFT ownership from mint_batch / transfer_nft / set_metadata_hash. Not the DEX LP ERC-721 snapshot. gapRanges are pruned-RPC height ranges this index could not backfill yet (see GET /v1/gaps, POST /v1/backfill).',
        },
        200,
        cors,
        origin
      );
    }

    if (path === '/v1/gaps' && req.method === 'GET') {
      const chainId = env.BOING_CHAIN_ID ?? 'boing-testnet';
      const gapRanges = await loadGapRanges(env.NFT_OWNER_DB, chainId);
      return jsonRes(
        {
          chainId,
          gapRanges,
          gapRangeCount: gapRanges.length,
          note: 'Inclusive [fromHeight, toHeight] height ranges the upstream RPC could not serve (pruned). Backfill with POST /v1/backfill?from=&to= once an archive/unpruned RPC can serve them.',
        },
        200,
        cors,
        origin
      );
    }

    if (path === '/v1/sync' && req.method === 'POST') {
      if (!authorized(req, env)) {
        return jsonRes({ error: 'unauthorized' }, 401, cors, origin);
      }
      try {
        const result = await runIngestTick(env);
        return jsonRes(result, 200, cors, origin);
      } catch (e) {
        return jsonRes(
          { ok: false, error: e instanceof Error ? e.message : String(e) },
          502,
          cors,
          origin
        );
      }
    }

    if (path === '/v1/backfill' && req.method === 'POST') {
      if (!authorized(req, env)) {
        return jsonRes({ error: 'unauthorized' }, 401, cors, origin);
      }
      const from = parseInt(url.searchParams.get('from') ?? '', 10);
      const to = parseInt(url.searchParams.get('to') ?? '', 10);
      if (!Number.isFinite(from) || !Number.isFinite(to) || from < 0 || to < from) {
        return jsonRes(
          { error: 'Invalid from/to (inclusive heights, 0 <= from <= to)' },
          400,
          cors,
          origin
        );
      }
      if (to - from > 500) {
        return jsonRes(
          { error: 'Range too large; max 501 heights per backfill call (from..to inclusive)' },
          400,
          cors,
          origin
        );
      }
      try {
        const result = await indexHeightRange(env, from, to);
        return jsonRes({ ok: true, action: 'backfill', ...result }, 200, cors, origin);
      } catch (e) {
        return jsonRes(
          { ok: false, error: e instanceof Error ? e.message : String(e) },
          502,
          cors,
          origin
        );
      }
    }

    if (path === '/v1/nfts/by-owner' && req.method === 'GET') {
      const ownerRaw = url.searchParams.get('owner') ?? url.searchParams.get('id');
      const owner = normalizeHex64(ownerRaw);
      if (!owner) {
        return jsonRes(
          { error: 'Missing or invalid owner (32-byte AccountId hex)' },
          400,
          cors,
          origin
        );
      }
      const limit = parsePositiveInt(url.searchParams.get('limit') ?? undefined, 50);
      const collection = normalizeHex64(url.searchParams.get('collection'));
      let cursorCollection: string | null = null;
      let cursorTokenId: string | null = null;
      const cursorRaw = url.searchParams.get('cursor');
      if (cursorRaw) {
        const parts = cursorRaw.split(':');
        if (parts.length === 2) {
          cursorCollection = normalizeHex64(parts[0]);
          cursorTokenId = normalizeHex64(parts[1]);
        }
      }

      const { items, nextCursor } = await listNftsByOwner(env.NFT_OWNER_DB, owner, {
        limit,
        cursorCollection,
        cursorTokenId,
        collectionFilter: collection,
      });

      const chainId = env.BOING_CHAIN_ID ?? 'boing-testnet';
      const cursor = await loadCursor(env.NFT_OWNER_DB, chainId);

      return jsonRes(
        {
          owner,
          items,
          nextCursor,
          indexer: {
            lastCommittedHeight: cursor.lastIndexedHeight,
            lastCommittedBlockHash: cursor.lastHash,
            chainId,
          },
        },
        200,
        cors,
        origin
      );
    }

    if (path === '/v1/nfts/item' && req.method === 'GET') {
      const collection = normalizeHex64(url.searchParams.get('collection'));
      const tokenId = normalizeHex64(
        url.searchParams.get('tokenId') ?? url.searchParams.get('token_id')
      );
      if (!collection || !tokenId) {
        return jsonRes(
          { error: 'Missing collection and/or tokenId (32-byte hex)' },
          400,
          cors,
          origin
        );
      }
      const row = await env.NFT_OWNER_DB.prepare(
        `SELECT collection_hex, token_id_hex, owner_hex, metadata_hash_hex,
                last_block_height, last_tx_id, last_event_kind
         FROM nft_ownership WHERE collection_hex = ? AND token_id_hex = ?`
      )
        .bind(collection, tokenId)
        .first<{
          collection_hex: string;
          token_id_hex: string;
          owner_hex: string;
          metadata_hash_hex: string | null;
          last_block_height: number;
          last_tx_id: string;
          last_event_kind: string;
        }>();
      if (!row) {
        return jsonRes({ found: false, collection, tokenId }, 404, cors, origin);
      }
      return jsonRes(
        {
          found: true,
          item: {
            collection: row.collection_hex,
            tokenId: row.token_id_hex,
            owner: row.owner_hex,
            metadataHash: row.metadata_hash_hex,
            lastBlockHeight: row.last_block_height,
            lastTxId: row.last_tx_id,
            lastEventKind: row.last_event_kind,
          },
        },
        200,
        cors,
        origin
      );
    }

    return jsonRes({ error: 'not found' }, 404, cors, origin);
  },
};
