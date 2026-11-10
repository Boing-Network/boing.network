/**
 * Oversize-block handling. A few early testnet blocks (~1311-1317) are 2 MB to 160 MB; the public RPC
 * gateway refuses responses over its 6 MiB cap with an explicit JSON-RPC error, and the direct Fly origin
 * is gated. Without this, the cursor stalled forever at 1313 (every tick re-requested the same window).
 *
 * Rule (same as workers/deploy-registry-indexer): skip a height ONLY on the gateway's explicit size-cap
 * rejection, and record it in `skipped_blocks` so an operator can backfill it from an own node. Timeouts,
 * 5xx, 429 and truncation stay transient: the whole window fails and the next tick retries.
 */

export function isOversizeRejection(e: unknown): boolean {
  if (e == null || typeof e !== 'object') return false;
  const msg = (e as { message?: unknown }).message;
  if (typeof msg !== 'string') return false;
  return /\bUpstream RPC response (too large|exceeded gateway limit)\b/i.test(msg);
}

/**
 * Highest height `<= contiguousThrough` (and `>= fromHeight`) with a real block hash. The ingest cursor must
 * land on an indexed (non-skipped) block, because reorg checks compare its hash with the RPC.
 */
export function cursorTargetHeight(
  fromHeight: number,
  contiguousThrough: number,
  hashes: ReadonlyMap<number, string>
): number | null {
  for (let h = contiguousThrough; h >= fromHeight; h--) {
    if (hashes.has(h)) return h;
  }
  return null;
}
