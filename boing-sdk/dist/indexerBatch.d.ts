/**
 * Indexer-oriented RPC batching: `boing_getLogs` has a max inclusive block span per call (default 128 on the node).
 * This module chunks wide ranges into compliant requests and merges results.
 */
import type { BoingClient } from './client.js';
import type { Block, ExecutionReceipt, GetLogsFilter, RpcLogEntry } from './types.js';
/** Default max inclusive block span per `boing_getLogs` (see `docs/RPC-API-SPEC.md`). */
export declare const DEFAULT_GET_LOGS_MAX_BLOCK_SPAN = 128;
export interface LogChunkFilter extends Omit<GetLogsFilter, 'fromBlock' | 'toBlock'> {
    fromBlock: number;
    toBlock: number;
}
export interface MapWithConcurrencyLimitOptions {
    signal?: AbortSignal;
}
/**
 * Map `items` with at most `limit` concurrent in-flight `fn` calls (default pattern for indexer backfill).
 * Results are in the **same order** as `items`. Throws if `signal` is aborted between tasks.
 */
export declare function mapWithConcurrencyLimit<T, R>(items: readonly T[], limit: number, fn: (item: T, index: number) => Promise<R>, options?: MapWithConcurrencyLimitOptions): Promise<R[]>;
/**
 * Plan inclusive `[from, to]` ranges each spanning at most `maxSpan` blocks.
 */
export declare function planLogBlockChunks(fromBlock: number, toBlock: number, maxSpan?: number): Array<{
    fromBlock: number;
    toBlock: number;
}>;
export interface GetLogsChunkedOptions {
    maxBlockSpan?: number;
    /**
     * Max concurrent `boing_getLogs` RPCs (default **1**). Increase only when your RPC operator allows it.
     */
    maxConcurrent?: number;
    signal?: AbortSignal;
}
/**
 * Run `boing_getLogs` over `[filter.fromBlock, filter.toBlock]` in chunks of at most `maxBlockSpan` blocks.
 * Results are merged and sorted by `(block_height, tx_index, log_index)`.
 */
export declare function getLogsChunked(client: BoingClient, filter: LogChunkFilter, options?: GetLogsChunkedOptions): Promise<RpcLogEntry[]>;
/**
 * Receipts for one block when the node returns them on `boing_getBlockByHeight(..., true)`.
 * Drops `null` slots (if any). Returns `[]` if the block is missing or has no `receipts` array.
 */
export declare function fetchReceiptsForBlockHeight(client: BoingClient, height: number): Promise<ExecutionReceipt[]>;
/** One block’s non-null receipts (same order as txs in the block when the node fills `receipts`). */
export interface BlockReceiptsBundle {
    height: number;
    receipts: ExecutionReceipt[];
}
export type FetchReceiptsHeightRangeMissing = 'throw' | 'omit';
export interface FetchReceiptsForHeightRangeOptions {
    /**
     * When `boing_getBlockByHeight` returns `null` (pruned / unknown height).
     * - `throw` (default): stop with an error naming the height.
     * - `omit`: skip that height (output heights may have gaps vs the requested range).
     */
    onMissingBlock?: FetchReceiptsHeightRangeMissing;
    /** When set, checked between tasks; throws if aborted. */
    signal?: AbortSignal;
    /**
     * Max concurrent block fetches (default **1**). Results are still returned **sorted by height**.
     */
    maxConcurrent?: number;
}
/** Same options as {@link fetchReceiptsForHeightRange} — shared height-range fetch semantics. */
export type FetchBlocksWithReceiptsForHeightRangeOptions = FetchReceiptsForHeightRangeOptions;
/** Full block payload + height for replay-style ingestion (`transactions` + `receipts`). */
export interface BlockWithReceiptsBundle {
    height: number;
    block: Block;
}
/** Concatenate receipts from ordered bundles (e.g. after `fetchReceiptsForHeightRange`). */
export declare function flattenReceiptsFromBundles(bundles: readonly BlockReceiptsBundle[]): ExecutionReceipt[];
/**
 * Summary of which heights were missing after a ranged fetch with **`onMissingBlock: 'omit'`**.
 * Persist **`missingHeightRangesInclusive`** (or **`omittedHeights`**) so indexers do not claim completeness across pruned gaps.
 */
export interface IndexerFetchGapSummary {
    requestedFromHeight: number;
    requestedToHeight: number;
    /** Sorted heights in `[requestedFromHeight, requestedToHeight]` with no block returned. */
    omittedHeights: number[];
    /** Merged inclusive ranges of {@link omittedHeights}. */
    missingHeightRangesInclusive: Array<{
        fromHeight: number;
        toHeight: number;
    }>;
    /**
     * Highest **H** such that every height in `[requestedFromHeight, H]` appears in **`fetchedHeights`**.
     * **`null`** when **`requestedFromHeight`** itself was omitted (no contiguous prefix).
     */
    lastContiguousFromStart: number | null;
}
/**
 * Given an inclusive requested range and the heights actually returned (e.g. from **`fetchBlocksWithReceiptsForHeightRange`**
 * or **`fetchReceiptsForHeightRange`** with **`onMissingBlock: 'omit'`**), compute omitted heights and a contiguous prefix watermark.
 * Duplicate entries in **`fetchedHeights`** are ignored.
 */
export declare function summarizeIndexerFetchGaps(requestedFromHeight: number, requestedToHeight: number, fetchedHeights: readonly number[]): IndexerFetchGapSummary;
/**
 * Fetch `include_receipts` blocks for each height in `[fromHeight, toHeight]` (inclusive).
 * Default **sequential** (`maxConcurrent` 1); set `maxConcurrent` > 1 to parallelize (respect RPC limits).
 */
export declare function fetchReceiptsForHeightRange(client: BoingClient, fromHeight: number, toHeight: number, options?: FetchReceiptsForHeightRangeOptions): Promise<BlockReceiptsBundle[]>;
/**
 * Fetch `boing_getBlockByHeight(h, true)` for each `h` in `[fromHeight, toHeight]` (inclusive).
 * Use this for the **canonical** indexer replay path (txs + receipts in one snapshot per height).
 */
export declare function fetchBlocksWithReceiptsForHeightRange(client: BoingClient, fromHeight: number, toHeight: number, options?: FetchBlocksWithReceiptsForHeightRangeOptions): Promise<BlockWithReceiptsBundle[]>;
//# sourceMappingURL=indexerBatch.d.ts.map