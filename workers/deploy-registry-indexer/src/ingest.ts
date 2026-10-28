/**
 * Pure helpers for the cron ingest loop (no Workers / SDK imports, so they unit-test under plain Node).
 *
 * Problem this solves: a handful of test blocks (heights ~1311-1317 on testnet) are 2 MB to 160 MB. The public RPC
 * gateway refuses anything over its 6 MiB cap with a JSON-RPC error. Before this, one such error failed the whole
 * `Promise.all` chunk every minute, so the cursor never advanced and every tick re-requested the huge blocks.
 *
 * Rule: skip a height ONLY when the gateway explicitly says the response is over its size cap. Anything else
 * (timeouts, 5xx, 429, truncation, network) is transient: the cursor stops before that height and the next tick
 * retries it, so no legitimate block is ever dropped.
 */

export type SkippedBlock = {
  height: number;
  reason: string;
  /** ISO timestamp when the skip was recorded. */
  at: string;
};

/** Cap on remembered skips in D1 (oldest dropped first). */
export const MAX_SKIPPED_RECORDS = 500;

/**
 * True only for the gateway's explicit size-cap rejections:
 * - "Upstream RPC response too large (N bytes; gateway limit M)."
 * - "Upstream RPC response exceeded gateway limit (M bytes)."
 * Deliberately NOT matched: "Upstream RPC response truncated" (transient), HTTP 5xx, timeouts, rate limits.
 */
export function isOversizeRejection(e: unknown): boolean {
  if (e == null || typeof e !== 'object') return false;
  const msg = (e as { message?: unknown }).message;
  if (typeof msg !== 'string') return false;
  return /\bUpstream RPC response (too large|exceeded gateway limit)\b/i.test(msg);
}

export type ChunkPlan = {
  /** Blocks (in height order) from the contiguous prefix that was fetched or skipped; `null` entries are heights with no block. */
  blocks: unknown[];
  skipped: SkippedBlock[];
  /** How many heights from the start of the chunk the cursor may advance. */
  advanceBy: number;
  /** First non-skippable failure; the cursor stops at `heights[advanceBy]`. */
  stopError?: unknown;
};

export function planChunk(
  heights: readonly number[],
  settled: ReadonlyArray<PromiseSettledResult<unknown>>,
  nowIso: string,
): ChunkPlan {
  const blocks: unknown[] = [];
  const skipped: SkippedBlock[] = [];
  let advanceBy = 0;
  for (let i = 0; i < heights.length; i += 1) {
    const s = settled[i]!;
    if (s.status === 'fulfilled') {
      if (s.value) blocks.push(s.value);
      advanceBy += 1;
      continue;
    }
    if (isOversizeRejection(s.reason)) {
      const raw = String((s.reason as { message?: unknown }).message ?? '');
      skipped.push({ height: heights[i]!, reason: raw.slice(0, 200), at: nowIso });
      advanceBy += 1;
      continue;
    }
    return { blocks, skipped, advanceBy, stopError: s.reason };
  }
  return { blocks, skipped, advanceBy };
}

/** Merge new skips into the stored list: dedupe by height, newest wins, bounded to MAX_SKIPPED_RECORDS. */
export function mergeSkipped(existing: readonly SkippedBlock[], added: readonly SkippedBlock[]): SkippedBlock[] {
  const byHeight = new Map<number, SkippedBlock>();
  for (const s of existing) byHeight.set(s.height, s);
  for (const s of added) byHeight.set(s.height, s);
  const all = [...byHeight.values()].sort((a, b) => a.height - b.height);
  return all.length > MAX_SKIPPED_RECORDS ? all.slice(all.length - MAX_SKIPPED_RECORDS) : all;
}

export function parseSkipped(raw: string | null | undefined): SkippedBlock[] {
  if (!raw) return [];
  try {
    const v = JSON.parse(raw) as unknown;
    if (!Array.isArray(v)) return [];
    return v.filter(
      (x): x is SkippedBlock =>
        x != null &&
        typeof x === 'object' &&
        typeof (x as SkippedBlock).height === 'number' &&
        typeof (x as SkippedBlock).reason === 'string',
    );
  } catch {
    return [];
  }
}
