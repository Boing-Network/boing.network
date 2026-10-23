/**
 * Compare persisted blocks to canonical RPC headers and rewind mismatched tips (OBS-1 reorg tail).
 * Uses lightweight **`getBlockByHeight(h, false)`** (no receipts).
 */

import type { BoingClient } from '@boing-network/boing-sdk';
import {
  deleteBlockAtHeightD1,
  getBlockRowAtHeight,
  normalizeObserverBlockHash,
  reconcileIngestCursorToBlocksTipD1,
  upsertIngestCursorD1,
} from './persist-d1.js';

export const DEFAULT_MAX_REORG_REWIND_STEPS = 4096;
/** Hard cap so env cannot request unbounded RPC work per tick. */
export const ABSOLUTE_MAX_REORG_REWIND_STEPS = 65_536;

export function parseMaxReorgRewindSteps(raw: string | undefined): number {
  if (raw == null || raw.trim() === '') return DEFAULT_MAX_REORG_REWIND_STEPS;
  const n = parseInt(raw, 10);
  if (!Number.isFinite(n) || n < 1) return DEFAULT_MAX_REORG_REWIND_STEPS;
  return Math.min(n, ABSOLUTE_MAX_REORG_REWIND_STEPS);
}

/** Exposed for unit tests — RPC vs D1 header hash comparison. */
export function compareCanonicalBlockHash(
  rpcHash: string | undefined | null,
  dbHash: string | undefined | null
): 'match' | 'mismatch' | 'rpc_missing' | 'db_missing' {
  if (rpcHash == null || rpcHash === '') return 'rpc_missing';
  if (dbHash == null || dbHash === '') return 'db_missing';
  return normalizeObserverBlockHash(rpcHash) === normalizeObserverBlockHash(dbHash) ? 'match' : 'mismatch';
}

export interface RewindStaleTipResult {
  rewindHeights: number[];
  lastIndexedHeight: number;
  lastHash: string;
}

export interface RewindStaleTipOptions {
  /** When true, skip RPC comparison (debug / cost control — risks stale data after reorg). */
  disabled?: boolean;
  /** Max header comparisons + deletes per tick (default {@link DEFAULT_MAX_REORG_REWIND_STEPS}). */
  maxSteps?: number;
}

/**
 * Walk down from the ingest cursor until RPC block hash matches D1, deleting mismatched heights.
 * Updates **`ingest_cursor`** when rows are removed or the cursor row was missing.
 */
export async function rewindStaleTipIfNeeded(
  db: D1Database,
  client: BoingClient,
  chainId: string,
  lastIndexedHeight: number,
  lastHash: string,
  nowSec: number,
  options?: RewindStaleTipOptions
): Promise<RewindStaleTipResult> {
  if (options?.disabled) {
    return {
      rewindHeights: [],
      lastIndexedHeight,
      lastHash: normalizeObserverBlockHash(lastHash),
    };
  }

  const maxSteps = options?.maxSteps ?? DEFAULT_MAX_REORG_REWIND_STEPS;

  if (lastIndexedHeight < 0) {
    return {
      rewindHeights: [],
      lastIndexedHeight,
      lastHash: normalizeObserverBlockHash(lastHash),
    };
  }

  let dbTip = await getBlockRowAtHeight(db, lastIndexedHeight);
  if (dbTip == null) {
    const r = await reconcileIngestCursorToBlocksTipD1(db, chainId, nowSec);
    console.log(
      JSON.stringify({
        ok: true,
        action: 'ingest_cursor_repaired',
        chainId,
        reason: 'cursor_height_missing_from_blocks',
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
    const rpcHash = rpcBlock?.hash;

    if (rpcHash == null || rpcHash === '') {
      if (rewindHeights.length > 0) {
        const r = await reconcileIngestCursorToBlocksTipD1(db, chainId, nowSec);
        console.log(
          JSON.stringify({
            ok: true,
            action: 'rewind_partial_rpc_missing',
            chainId,
            rewindHeights,
            reconciledToHeight: r.lastHeight,
          })
        );
        return { rewindHeights, lastIndexedHeight: r.lastHeight, lastHash: r.lastHash };
      }
      console.log(
        JSON.stringify({
          ok: true,
          action: 'reorg_check_rpc_unavailable',
          chainId,
          height: h,
        })
      );
      return {
        rewindHeights: [],
        lastIndexedHeight,
        lastHash: normalizeObserverBlockHash(dbTip.block_hash),
      };
    }

    const dbRow = await getBlockRowAtHeight(db, h);
    if (dbRow == null) {
      h -= 1;
      continue;
    }

    if (compareCanonicalBlockHash(rpcHash, dbRow.block_hash) === 'match') {
      if (rewindHeights.length > 0) {
        await upsertIngestCursorD1(db, chainId, h, dbRow.block_hash, nowSec);
        console.log(
          JSON.stringify({
            ok: true,
            action: 'rewind',
            chainId,
            rewindHeights,
            newLastHeight: h,
          })
        );
        return {
          rewindHeights,
          lastIndexedHeight: h,
          lastHash: normalizeObserverBlockHash(dbRow.block_hash),
        };
      }
      return {
        rewindHeights: [],
        lastIndexedHeight,
        lastHash: normalizeObserverBlockHash(dbRow.block_hash),
      };
    }

    await deleteBlockAtHeightD1(db, h);
    rewindHeights.push(h);
    h -= 1;
  }

  if (rewindHeights.length > 0) {
    const r = await reconcileIngestCursorToBlocksTipD1(db, chainId, nowSec);
    console.log(
      JSON.stringify({
        ok: true,
        action: 'rewind_reconcile',
        chainId,
        rewindHeights,
        reconciledToHeight: r.lastHeight,
        stepCap: steps >= maxSteps,
      })
    );
    return { rewindHeights, lastIndexedHeight: r.lastHeight, lastHash: r.lastHash };
  }

  return {
    rewindHeights: [],
    lastIndexedHeight,
    lastHash: normalizeObserverBlockHash(dbTip.block_hash),
  };
}
