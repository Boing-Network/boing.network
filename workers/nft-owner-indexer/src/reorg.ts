/**
 * OBS-1-style tip rewind: compare RPC headers to indexed_blocks, delete mismatched heights.
 */

import type { BoingClient } from 'boing-sdk';
import { normalizeHex64, zeros32 } from './cors.js';
import {
  deleteHeightAndRebuild,
  getIndexedBlockAtHeight,
  reconcileCursorToTip,
  upsertIngestCursor,
} from './persist.js';

export const DEFAULT_MAX_REORG_REWIND_STEPS = 4096;
export const ABSOLUTE_MAX_REORG_REWIND_STEPS = 65_536;

export function parseMaxReorgRewindSteps(raw: string | undefined): number {
  if (raw == null || raw.trim() === '') return DEFAULT_MAX_REORG_REWIND_STEPS;
  const n = parseInt(raw, 10);
  if (!Number.isFinite(n) || n < 1) return DEFAULT_MAX_REORG_REWIND_STEPS;
  return Math.min(n, ABSOLUTE_MAX_REORG_REWIND_STEPS);
}

export function compareCanonicalBlockHash(
  rpcHash: string | undefined | null,
  dbHash: string | undefined | null
): 'match' | 'mismatch' | 'rpc_missing' | 'db_missing' {
  const a = normalizeHex64(rpcHash ?? undefined);
  const b = normalizeHex64(dbHash ?? undefined);
  if (a == null) return 'rpc_missing';
  if (b == null) return 'db_missing';
  return a === b ? 'match' : 'mismatch';
}

export async function rewindStaleTipIfNeeded(
  db: D1Database,
  client: BoingClient,
  chainId: string,
  lastIndexedHeight: number,
  lastHash: string,
  nowSec: number,
  options?: { disabled?: boolean; maxSteps?: number }
): Promise<{ rewindHeights: number[]; lastIndexedHeight: number; lastHash: string }> {
  if (options?.disabled) {
    return {
      rewindHeights: [],
      lastIndexedHeight,
      lastHash: normalizeHex64(lastHash) ?? zeros32(),
    };
  }

  const maxSteps = options?.maxSteps ?? DEFAULT_MAX_REORG_REWIND_STEPS;
  if (lastIndexedHeight < 0) {
    return {
      rewindHeights: [],
      lastIndexedHeight,
      lastHash: normalizeHex64(lastHash) ?? zeros32(),
    };
  }

  let dbTip = await getIndexedBlockAtHeight(db, lastIndexedHeight);
  if (dbTip == null) {
    const r = await reconcileCursorToTip(db, chainId, nowSec);
    console.log(
      JSON.stringify({
        ok: true,
        action: 'ingest_cursor_repaired',
        chainId,
        reason: 'cursor_height_missing_from_indexed_blocks',
        newLastHeight: r.lastHeight,
      })
    );
    return { rewindHeights: [], lastIndexedHeight: r.lastHeight, lastHash: r.lastHash };
  }

  const rewindHeights: number[] = [];
  let h = lastIndexedHeight;
  let steps = 0;

  while (h >= 0 && steps < maxSteps) {
    steps += 1;
    const rpcBlock = await client.getBlockByHeight(h, false);
    const rpcHash =
      rpcBlock && typeof rpcBlock === 'object'
        ? normalizeHex64((rpcBlock as { hash?: unknown }).hash)
        : null;
    const dbRow = await getIndexedBlockAtHeight(db, h);
    const cmp = compareCanonicalBlockHash(rpcHash, dbRow?.block_hash);

    if (cmp === 'rpc_missing') {
      if (rewindHeights.length > 0) {
        const r = await reconcileCursorToTip(db, chainId, nowSec);
        return { rewindHeights, lastIndexedHeight: r.lastHeight, lastHash: r.lastHash };
      }
      break;
    }

    if (cmp === 'match') {
      if (rewindHeights.length > 0) {
        await upsertIngestCursor(db, chainId, h, dbRow!.block_hash, nowSec);
        return {
          rewindHeights,
          lastIndexedHeight: h,
          lastHash: dbRow!.block_hash,
        };
      }
      return {
        rewindHeights: [],
        lastIndexedHeight,
        lastHash: normalizeHex64(lastHash) ?? zeros32(),
      };
    }

    // mismatch or db_missing
    await deleteHeightAndRebuild(db, h, nowSec);
    rewindHeights.push(h);
    h -= 1;
  }

  if (rewindHeights.length > 0) {
    const r = await reconcileCursorToTip(db, chainId, nowSec);
    console.log(
      JSON.stringify({
        ok: true,
        action: 'reorg_rewind',
        chainId,
        rewindHeights,
        newLastHeight: r.lastHeight,
      })
    );
    return { rewindHeights, lastIndexedHeight: r.lastHeight, lastHash: r.lastHash };
  }

  return {
    rewindHeights: [],
    lastIndexedHeight,
    lastHash: normalizeHex64(lastHash) ?? zeros32(),
  };
}
