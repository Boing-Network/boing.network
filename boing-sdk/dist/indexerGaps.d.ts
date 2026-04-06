/**
 * Merge pruned / missing height ranges and compute safe contiguous cursor advancement.
 * Pair with {@link summarizeIndexerFetchGaps} and `onMissingBlock: 'omit'` fetches.
 */
import type { IndexerFetchGapSummary } from './indexerBatch.js';
/** Inclusive block height range (same shape as {@link IndexerFetchGapSummary.missingHeightRangesInclusive}). */
export interface InclusiveHeightRange {
    fromHeight: number;
    toHeight: number;
}
/**
 * Sort and merge overlapping or adjacent inclusive ranges into a minimal cover.
 */
export declare function mergeInclusiveHeightRanges(ranges: readonly InclusiveHeightRange[]): InclusiveHeightRange[];
/** Union of two range lists (merge + normalize). */
export declare function unionInclusiveHeightRanges(a: readonly InclusiveHeightRange[], b: readonly InclusiveHeightRange[]): InclusiveHeightRange[];
/**
 * Remove heights **`indexed`** from each stored gap (e.g. after archive backfill). Input gaps are normalized
 * (merged) first; the result is merged again so adjacent remnants stay minimal.
 */
export declare function subtractInclusiveRangeFromRanges(indexed: InclusiveHeightRange, gaps: readonly InclusiveHeightRange[]): InclusiveHeightRange[];
/** One row matching **`tools/observer-indexer-schema.sql`** → **`block_height_gaps`**. */
export interface BlockHeightGapInsertRow {
    chain_id: string;
    from_height: number;
    to_height: number;
    reason: string;
    recorded_at: number;
}
/**
 * Normalized **`block_height_gaps`** rows for parameterized INSERT (one row per merged contiguous run).
 */
export declare function blockHeightGapRowsForInsert(input: {
    chainId: string;
    ranges: readonly InclusiveHeightRange[];
    reason?: string;
    recordedAtSec?: number;
}): BlockHeightGapInsertRow[];
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
export declare function nextContiguousIndexedHeightAfterOmittedFetch(lastIndexedHeight: number, summary: IndexerFetchGapSummary): number;
//# sourceMappingURL=indexerGaps.d.ts.map