/**
 * Finality / tip helpers for indexers using `boing_getSyncState`.
 * See `docs/RPC-API-SPEC.md` — today `head_height` and `finalized_height` match; they may diverge later.
 */
import type { BoingClient } from './client.js';
export interface IndexerChainTips {
    headHeight: number;
    finalizedHeight: number;
    /**
     * Inclusive upper bound for **durable** indexing: min(head, finalized).
     * Prefer indexing only through this height until the node exposes lagging finalized semantics.
     */
    durableIndexThrough: number;
    /** Tip block hash (32-byte hex). */
    latestBlockHash: string;
}
/**
 * Read committed tip + finalized fields from the node (single RPC).
 */
export declare function getIndexerChainTips(client: BoingClient): Promise<IndexerChainTips>;
/**
 * Clamp an inclusive `[fromHeight, toHeight]` so `toHeight` does not exceed `durableIndexThrough`.
 * Returns `null` if the range is empty after clamping or if `fromHeight > toHeight`.
 */
export declare function clampIndexerHeightRange(fromHeight: number, toHeight: number, durableIndexThrough: number): {
    fromHeight: number;
    toHeight: number;
} | null;
/** How {@link planIndexerChainTipsWithFallback} obtained {@link IndexerChainTips}. */
export type IndexerTipsSource = 'sync_state' | 'chain_height';
export interface PlanIndexerCatchUpOptions {
    /**
     * Cap one tick to at most this many block heights (inclusive count).
     * When unset, the planned range runs through the clamped durable tip.
     */
    maxBlocksPerTick?: number;
}
/** Result of {@link planIndexerCatchUp}: durable range `[fromHeight, toHeight]` to fetch next. */
export interface IndexerCatchUpPlan {
    tips: IndexerChainTips;
    tipsSource: IndexerTipsSource;
    fromHeight: number;
    toHeight: number;
}
/**
 * Like {@link getIndexerChainTips}, but when **`boing_getSyncState`** returns **-32601**,
 * builds tips from **`boing_chainHeight`** + **`boing_getBlockByHeight(tip, false)`** (needs block **`hash`**).
 */
export declare function planIndexerChainTipsWithFallback(client: BoingClient): Promise<{
    tips: IndexerChainTips;
    tipsSource: IndexerTipsSource;
}>;
/**
 * One ingestion tick: load chain tips (with **`getSyncState`** fallback), then
 * **`clampIndexerHeightRange(lastIndexedHeight + 1, headHeight, durableIndexThrough)`**.
 * Returns **`null`** when there is nothing to index yet.
 *
 * @param lastIndexedHeight — height you have fully persisted (**`-1`** before genesis).
 */
export declare function planIndexerCatchUp(client: BoingClient, lastIndexedHeight: number, options?: PlanIndexerCatchUpOptions): Promise<IndexerCatchUpPlan | null>;
//# sourceMappingURL=indexerSync.d.ts.map