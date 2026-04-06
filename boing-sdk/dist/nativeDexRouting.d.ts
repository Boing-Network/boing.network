/**
 * Constant-product **routing** and **quote aggregation** over Boing native pools (pure math + optional RPC hydrate).
 * No external chains. Execution still uses **`contract_call`** / multihop router encoders elsewhere in the SDK.
 */
import type { BoingClient } from './client.js';
import type { NativeDexIntegrationOverrides } from './dexIntegration.js';
import { type NativeDexDirectorySnapshot } from './nativeDexDirectory.js';
/** One tradeable CP pool with oriented reserves (A/B match on-chain slot semantics). */
export type CpPoolVenue = {
    poolHex: `0x${string}`;
    tokenAHex: `0x${string}`;
    tokenBHex: `0x${string}`;
    reserveA: bigint;
    reserveB: bigint;
    /** Output-side fee bps; use **`NATIVE_CP_SWAP_FEE_BPS`** when on-chain reads **`0`**. */
    feeBps: bigint;
};
export type CpQuoteResult = {
    amountOut: bigint;
    tokenOutHex: string;
    /** Native AMM **`swap`** direction: **`0`** = A→B, **`1`** = B→A. */
    directionForSwapCalldata: bigint;
};
/**
 * Exact output quote for **`tokenIn` → opposite side** on one venue (fails if **`tokenIn`** is not **`tokenA`** or **`tokenB`**).
 */
export declare function quoteCpPoolSwap(venue: CpPoolVenue, tokenInHex: string, amountIn: bigint): CpQuoteResult;
/** Rank venues that list **`(tokenIn, tokenOut)`** by **`amountOut`** for a given **`amountIn`** (best first). */
export declare function rankDirectCpPools(venues: readonly CpPoolVenue[], tokenInHex: string, tokenOutHex: string, amountIn: bigint): Array<{
    venue: CpPoolVenue;
    amountOut: bigint;
    directionForSwapCalldata: bigint;
}>;
export type RouteHop = {
    venue: CpPoolVenue;
    tokenInHex: string;
    tokenOutHex: string;
    amountIn: bigint;
    amountOut: bigint;
    directionForSwapCalldata: bigint;
};
export type CpSwapRoute = {
    hops: RouteHop[];
    tokenInHex: string;
    tokenOutHex: string;
    amountIn: bigint;
    amountOut: bigint;
};
/**
 * Enumerate simple CP paths (**no** same-pool reuse) up to **`maxHops`** pools; returns routes sorted by **`amountOut`** (best first).
 * For production UIs, cap **`maxHops`** at **2–3** and keep **`venues`** small (indexed subgraph).
 */
export declare function findBestCpRoutes(venues: readonly CpPoolVenue[], tokenInHex: string, tokenOutHex: string, amountIn: bigint, options?: {
    maxHops?: number;
    maxRoutes?: number;
}): CpSwapRoute[];
/** Best route or **`undefined`** when none. */
export declare function findBestCpRoute(venues: readonly CpPoolVenue[], tokenInHex: string, tokenOutHex: string, amountIn: bigint, options?: {
    maxHops?: number;
    maxRoutes?: number;
}): CpSwapRoute | undefined;
/**
 * Even-split **aggregation heuristic**: divide **`totalAmountIn`** across the top **`poolCount`** direct pools (by full-size quote rank), sum outputs.
 * Not an optimal CEX splitter; useful for UI estimates and incremental liquidity use.
 */
export declare function quoteCpEvenSplitAcrossDirectPools(venues: readonly CpPoolVenue[], tokenInHex: string, tokenOutHex: string, totalAmountIn: bigint, poolCount: number): {
    rankedPools: Array<{
        venue: CpPoolVenue;
        amountOut: bigint;
        directionForSwapCalldata: bigint;
    }>;
    allocations: Array<{
        venue: CpPoolVenue;
        amountIn: bigint;
        amountOut: bigint;
        directionForSwapCalldata: bigint;
    }>;
    totalOut: bigint;
};
export type PoolTokenRow = {
    poolHex: string;
    tokenAHex: string;
    tokenBHex: string;
};
/**
 * Hydrate **`CpPoolVenue`** rows from Boing RPC (reserves + fee bps per pool). **Boing-only.**
 */
export declare function hydrateCpPoolVenuesFromRpc(client: BoingClient, rows: readonly PoolTokenRow[], options?: {
    concurrency?: number;
}): Promise<CpPoolVenue[]>;
export type FetchCpRoutingFromDirectoryLogsOptions = {
    overrides?: NativeDexIntegrationOverrides;
    registerLogs: {
        fromBlock: number;
        toBlock?: number;
    };
    maxHops?: number;
    maxRoutes?: number;
    hydrateConcurrency?: number;
};
/**
 * **Boing-only** pipeline: directory **`register_pair`** log range → hydrate venues → best CP route(s).
 * Pair with **`encodeNativeDexSwap3RouterCalldata128`** etc. when executing multihop on-chain.
 */
export declare function fetchCpRoutingFromDirectoryLogs(client: BoingClient, tokenInHex: string, tokenOutHex: string, amountIn: bigint, options: FetchCpRoutingFromDirectoryLogsOptions): Promise<{
    snapshot: NativeDexDirectorySnapshot;
    venues: CpPoolVenue[];
    routes: CpSwapRoute[];
}>;
//# sourceMappingURL=nativeDexRouting.d.ts.map