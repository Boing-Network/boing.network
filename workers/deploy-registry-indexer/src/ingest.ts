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

// --- Reorg handling ------------------------------------------------------------------------------
//
// The cursor alone cannot tell whether already-indexed blocks are still canonical. We keep the hashes of
// the most recently indexed heights in `ingest_state.recent_hashes` and, each tick, compare the newest one
// with the chain. On mismatch we walk back until a stored hash matches again, delete deployment rows from
// the first non-canonical height up, and move the cursor there so those heights are indexed again.

export type HashEntry = { height: number; hash: string };

/** Heights whose hashes are remembered (bounds how deep a reorg can be repaired automatically). */
export const MAX_RECENT_HASHES = 128;
/** Hash probes per tick while walking back (keeps a tick inside the Workers subrequest budget). */
export const MAX_REWIND_PROBES_PER_TICK = 16;

export function parseRecentHashes(raw: string | null | undefined): HashEntry[] {
  if (!raw) return [];
  try {
    const v = JSON.parse(raw) as unknown;
    if (!Array.isArray(v)) return [];
    return v
      .filter(
        (x): x is HashEntry =>
          x != null && typeof x === 'object' && Number.isInteger((x as HashEntry).height) && typeof (x as HashEntry).hash === 'string',
      )
      .sort((a, b) => a.height - b.height);
  } catch {
    return [];
  }
}

/** Merge entries (newest wins per height), sorted ascending, bounded to the newest `max`. */
export function appendRecentHashes(ring: readonly HashEntry[], added: readonly HashEntry[], max = MAX_RECENT_HASHES): HashEntry[] {
  const by = new Map<number, string>();
  for (const e of ring) by.set(e.height, e.hash);
  for (const e of added) by.set(e.height, e.hash);
  const all = [...by.entries()].map(([height, hash]) => ({ height, hash })).sort((a, b) => a.height - b.height);
  return all.length > max ? all.slice(all.length - max) : all;
}

function str(v: unknown): string | null {
  if (typeof v === 'string' && v.trim()) return v.trim().toLowerCase();
  if (Array.isArray(v) && v.length > 0 && v.every((b) => Number.isInteger(b) && b >= 0 && b <= 255)) {
    return '0x' + (v as number[]).map((b) => b.toString(16).padStart(2, '0')).join('');
  }
  return null;
}

/** `{ height, hash, parentHash }` of an RPC block (null fields when the node omits them). */
export function blockIdentity(block: unknown): { height: number | null; hash: string | null; parentHash: string | null } {
  if (!block || typeof block !== 'object') return { height: null, hash: null, parentHash: null };
  const b = block as { hash?: unknown; header?: { height?: unknown; parent_hash?: unknown } };
  const h = b.header?.height;
  return {
    height: typeof h === 'number' && Number.isInteger(h) ? h : null,
    hash: str(b.hash),
    parentHash: str(b.header?.parent_hash),
  };
}

/**
 * Index of the first block (in height order) whose parent hash contradicts the hash we hold for the
 * height below it (from `ring` or from an earlier block in the same list), or -1 when all are consistent.
 * Blocks without a parent hash or with an unknown predecessor are accepted.
 */
export function firstParentMismatch(ring: readonly HashEntry[], blocks: readonly unknown[]): number {
  const known = new Map<number, string>(ring.map((e) => [e.height, e.hash]));
  for (let i = 0; i < blocks.length; i += 1) {
    const id = blockIdentity(blocks[i]);
    if (id.height == null) continue;
    const prev = known.get(id.height - 1);
    if (prev != null && id.parentHash != null && id.parentHash !== prev) return i;
    if (id.hash != null) known.set(id.height, id.hash);
  }
  return -1;
}

export type RewindPlan = {
  /** First height to index again (cursor moves here; rows at or above it are deleted). */
  rewindTo: number;
  /** True when a canonical ancestor was found; false when the walk hit the probe bound or the ring's end. */
  resolved: boolean;
  probes: number;
};

/**
 * Compare stored hashes below `cursor` with the chain, newest first. Returns null when the newest stored
 * hash is still canonical. `fetchHash` returns the chain's hash at a height (null when unavailable).
 */
export async function planReorgRewind(
  ring: readonly HashEntry[],
  cursor: number,
  fetchHash: (height: number) => Promise<string | null>,
  maxProbes = MAX_REWIND_PROBES_PER_TICK,
): Promise<RewindPlan | null> {
  const below = ring.filter((e) => e.height < cursor).sort((a, b) => b.height - a.height);
  if (below.length === 0) return null;
  let probes = 0;
  let lowestStale: number | null = null;
  for (const e of below) {
    if (probes >= maxProbes) break;
    const chain = await fetchHash(e.height);
    probes += 1;
    // Unknown (pruned / unavailable): stop here and treat what we checked so far as the answer.
    if (chain == null) break;
    if (chain.toLowerCase() === e.hash.toLowerCase()) {
      return lowestStale == null ? null : { rewindTo: e.height + 1, resolved: true, probes };
    }
    lowestStale = e.height;
  }
  return lowestStale == null ? null : { rewindTo: lowestStale, resolved: false, probes };
}
