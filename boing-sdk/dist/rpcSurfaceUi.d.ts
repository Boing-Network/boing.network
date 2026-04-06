/**
 * UI-oriented helpers from **`boing_health.rpc_surface`** / **`boing_getNetworkInfo.rpc_surface`**.
 */
import type { BoingHealthRpcSurface } from './types.js';
/** Prefer **`networkInfo.rpc_surface`**, then **`health.rpc_surface`**. */
export declare function mergeRpcSurface(health: {
    rpc_surface?: BoingHealthRpcSurface;
} | null, networkInfo: {
    rpc_surface?: BoingHealthRpcSurface;
} | null): BoingHealthRpcSurface | null;
/**
 * Clamp an inclusive block span to the node’s **`get_logs_max_block_range`** (inclusive pair length).
 */
export declare function clampGetLogsBlockSpan(surface: BoingHealthRpcSurface | null | undefined, fromBlock: number, toBlock: number): {
    fromBlock: number;
    toBlock: number;
    clamped: boolean;
};
/**
 * Suggested minimum delay between steady-state read RPCs when HTTP JSON-RPC rate limiting is enabled (**RPS > 0**).
 * Returns **`0`** when unlimited.
 */
export declare function suggestedReadSpacingMs(surface: BoingHealthRpcSurface | null | undefined): number;
/**
 * Short tooltip copy for logs cap (span and max rows).
 */
export declare function describeGetLogsLimits(surface: BoingHealthRpcSurface | null | undefined): string;
/**
 * Whether the node advertises a finite WebSocket subscriber cap (non-zero).
 */
export declare function websocketHasConnectionCap(surface: BoingHealthRpcSurface | null | undefined): boolean;
//# sourceMappingURL=rpcSurfaceUi.d.ts.map