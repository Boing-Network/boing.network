/**
 * OBS-1-style tip rewind: compare RPC headers to indexed_blocks, delete mismatched heights.
 *
 * Audit #61 item 74: prefer an exponential + binary search for the fork point so a deep reorg
 * does not issue one sequential RPC call per height up to `maxSteps` (Worker CPU risk).
 */

import type { BoingClient } from '@boing-network/boing-sdk';
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

async function rpcHashAt(
  client: BoingClient,
  height: number
): Promise<string | null> {
  const rpcBlock = await client.getBlockByHeight(height, false);
  if (!rpcBlock || typeof rpcBlock !== 'object') return null;
  return normalizeHex64((rpcBlock as { hash?: unknown }).hash);
}

/**
 * Find the highest height `h` in `[lo, hi]` whose RPC hash matches the DB row (canonical ancestor).
 * Returns `null` if none match within the window (caller falls back to wiping the window).
 *
 * Uses exponential probe from `hi` then binary search — O(log N) RPC calls for the fork point.
 */
export async function findCanonicalForkHeight(
  db: D1Database,
  client: BoingClient,
  hi: number,
  maxSteps: number
): Promise<{ forkHeight: number | null; rpcCalls: number }> {
  let rpcCalls = 0;
  const lo = Math.max(0, hi - maxSteps + 1);

  const matches = async (h: number): Promise<boolean | 'rpc_missing'> => {
    rpcCalls += 1;
    const rpcHash = await rpcHashAt(client, h);
    if (rpcHash == null) return 'rpc_missing';
    const dbRow = await getIndexedBlockAtHeight(db, h);
    const cmp = compareCanonicalBlockHash(rpcHash, dbRow?.block_hash);
    return cmp === 'match';
  };

  // Tip already matching → no rewind.
  const tipMatch = await matches(hi);
  if (tipMatch === true) {
    return { forkHeight: hi, rpcCalls };
  }
  if (tipMatch === 'rpc_missing') {
    return { forkHeight: null, rpcCalls };
  }

  // Exponential search for a matching ancestor.
  let step = 1;
  let probe = hi - 1;
  let foundMatch: number | null = null;
  while (probe >= lo) {
    const m = await matches(probe);
    if (m === 'rpc_missing') break;
    if (m === true) {
      foundMatch = probe;
      break;
    }
    step = Math.min(step * 2, hi - lo);
    probe = hi - step;
    if (probe < lo) {
      // Final check at window floor.
      const floor = await matches(lo);
      if (floor === true) foundMatch = lo;
      break;
    }
  }

  if (foundMatch == null) {
    return { forkHeight: null, rpcCalls };
  }

  // Binary search highest matching height in [foundMatch, hi].
  let left = foundMatch;
  let right = hi;
  while (left < right) {
    const mid = Math.ceil((left + right) / 2);
    const m = await matches(mid);
    if (m === true) {
      left = mid;
    } else {
      // mismatch or rpc_missing → search lower
      right = mid - 1;
    }
  }
  return { forkHeight: left, rpcCalls };
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

  const tipRpcHash = await rpcHashAt(client, lastIndexedHeight);
  const tipCmp = compareCanonicalBlockHash(tipRpcHash, dbTip.block_hash);
  if (tipCmp === 'match') {
    return {
      rewindHeights: [],
      lastIndexedHeight,
      lastHash: normalizeHex64(lastHash) ?? zeros32(),
    };
  }
  if (tipCmp === 'rpc_missing') {
    // Cannot decide; leave cursor alone for the next tick.
    return {
      rewindHeights: [],
      lastIndexedHeight,
      lastHash: normalizeHex64(lastHash) ?? zeros32(),
    };
  }

  const { forkHeight, rpcCalls } = await findCanonicalForkHeight(
    db,
    client,
    lastIndexedHeight,
    maxSteps
  );

  const rewindHeights: number[] = [];
  const wipeFrom = forkHeight == null ? Math.max(0, lastIndexedHeight - maxSteps + 1) : forkHeight + 1;

  for (let h = lastIndexedHeight; h >= wipeFrom; h -= 1) {
    await deleteHeightAndRebuild(db, h, nowSec);
    rewindHeights.push(h);
  }

  if (rewindHeights.length > 0) {
    const r = await reconcileCursorToTip(db, chainId, nowSec);
    if (forkHeight != null) {
      const row = await getIndexedBlockAtHeight(db, forkHeight);
      if (row) {
        await upsertIngestCursor(db, chainId, forkHeight, row.block_hash, nowSec);
        console.log(
          JSON.stringify({
            ok: true,
            action: 'reorg_rewind',
            chainId,
            rewindHeights,
            forkHeight,
            rpcCalls,
            newLastHeight: forkHeight,
          })
        );
        return {
          rewindHeights,
          lastIndexedHeight: forkHeight,
          lastHash: row.block_hash,
        };
      }
    }
    console.log(
      JSON.stringify({
        ok: true,
        action: 'reorg_rewind',
        chainId,
        rewindHeights,
        forkHeight,
        rpcCalls,
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
