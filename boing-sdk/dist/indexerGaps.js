/**
 * Merge pruned / missing height ranges and compute safe contiguous cursor advancement.
 * Pair with {@link summarizeIndexerFetchGaps} and `onMissingBlock: 'omit'` fetches.
 */
function assertInclusiveRange(r, label) {
    if (!Number.isInteger(r.fromHeight) || !Number.isInteger(r.toHeight)) {
        throw new TypeError(`${label}: fromHeight and toHeight must be integers`);
    }
    if (r.fromHeight < 0 || r.toHeight < 0) {
        throw new RangeError(`${label}: heights must be >= 0`);
    }
    if (r.fromHeight > r.toHeight) {
        throw new RangeError(`${label}: fromHeight must be <= toHeight`);
    }
}
/**
 * Sort and merge overlapping or adjacent inclusive ranges into a minimal cover.
 */
export function mergeInclusiveHeightRanges(ranges) {
    if (ranges.length === 0)
        return [];
    for (let i = 0; i < ranges.length; i++) {
        assertInclusiveRange(ranges[i], `ranges[${i}]`);
    }
    const sorted = [...ranges].sort((a, b) => a.fromHeight - b.fromHeight);
    const out = [];
    let cur = { ...sorted[0] };
    for (let i = 1; i < sorted.length; i++) {
        const r = sorted[i];
        if (r.fromHeight > cur.toHeight + 1) {
            out.push(cur);
            cur = { ...r };
        }
        else {
            cur.toHeight = Math.max(cur.toHeight, r.toHeight);
        }
    }
    out.push(cur);
    return out;
}
/** Union of two range lists (merge + normalize). */
export function unionInclusiveHeightRanges(a, b) {
    return mergeInclusiveHeightRanges([...a, ...b]);
}
function subtractOneGapByIndexedRange(indexed, gap) {
    if (indexed.toHeight < gap.fromHeight || indexed.fromHeight > gap.toHeight) {
        return [{ ...gap }];
    }
    const out = [];
    if (gap.fromHeight < indexed.fromHeight) {
        out.push({ fromHeight: gap.fromHeight, toHeight: indexed.fromHeight - 1 });
    }
    if (indexed.toHeight < gap.toHeight) {
        out.push({ fromHeight: indexed.toHeight + 1, toHeight: gap.toHeight });
    }
    return out;
}
/**
 * Remove heights **`indexed`** from each stored gap (e.g. after archive backfill). Input gaps are normalized
 * (merged) first; the result is merged again so adjacent remnants stay minimal.
 */
export function subtractInclusiveRangeFromRanges(indexed, gaps) {
    assertInclusiveRange(indexed, 'indexed');
    if (gaps.length === 0)
        return [];
    const normalized = mergeInclusiveHeightRanges(gaps);
    const pieces = [];
    for (const g of normalized) {
        pieces.push(...subtractOneGapByIndexedRange(indexed, g));
    }
    return mergeInclusiveHeightRanges(pieces);
}
/**
 * Normalized **`block_height_gaps`** rows for parameterized INSERT (one row per merged contiguous run).
 */
export function blockHeightGapRowsForInsert(input) {
    const reason = input.reason ?? 'pruned';
    const recorded_at = input.recordedAtSec ?? Math.floor(Date.now() / 1000);
    const merged = mergeInclusiveHeightRanges(input.ranges);
    return merged.map((r) => ({
        chain_id: input.chainId,
        from_height: r.fromHeight,
        to_height: r.toHeight,
        reason,
        recorded_at,
    }));
}
function assertIndexerCursor(name, h) {
    if (!Number.isInteger(h))
        throw new TypeError(`${name} must be an integer`);
    if (h < -1)
        throw new RangeError(`${name} must be >= -1`);
}
/**
 * After fetching `[lastIndexedHeight + 1, …]` with omissions, highest height that remains **contiguous**
 * from `lastIndexedHeight` (i.e. you may set `ingest_cursor.last_committed_height` here without holes).
 *
 * - Full success (no omissions): `requestedToHeight`.
 * - Gap at the start of the tick: `lastIndexedHeight` (no forward progress on the contiguous cursor).
 * - Gap in the middle/end: `lastContiguousFromStart` from the summary (may be below `requestedToHeight`).
 *
 * @throws if `summary.requestedFromHeight !== lastIndexedHeight + 1`
 */
export function nextContiguousIndexedHeightAfterOmittedFetch(lastIndexedHeight, summary) {
    assertIndexerCursor('lastIndexedHeight', lastIndexedHeight);
    const next = lastIndexedHeight + 1;
    if (summary.requestedFromHeight !== next) {
        throw new RangeError(`summary.requestedFromHeight (${summary.requestedFromHeight}) must equal lastIndexedHeight + 1 (${next})`);
    }
    if (summary.omittedHeights.length === 0) {
        return summary.requestedToHeight;
    }
    if (summary.lastContiguousFromStart === null) {
        return lastIndexedHeight;
    }
    return summary.lastContiguousFromStart;
}
