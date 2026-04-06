/**
 * Polling “connection” snapshot for dApp headers / settings screens (framework-agnostic).
 * Wire **`onUpdate`** into React/Vue/Svelte setState, or read **`getSnapshot()`** from a timer.
 */
import type { BoingClient } from './client.js';
import type { BoingHealth, BoingHealthRpcSurface, NetworkInfo } from './types.js';
export interface BoingConnectionSnapshot {
    baseUrl: string;
    updatedAtMs: number;
    loading: boolean;
    health: BoingHealth | null;
    networkInfo: NetworkInfo | null;
    chainId: number | null;
    headHeight: number | null;
    rpcSurface: BoingHealthRpcSurface | null;
    lastError: unknown;
    /** From the last successful JSON-RPC **`POST /`** on the client, when exposed. */
    lastRequestId: string | undefined;
}
export interface BoingConnectionMonitorOptions {
    /** Default **12_000** ms. */
    pollIntervalMs?: number;
    /** Also fetch **`boing_getNetworkInfo`** each tick (default **true**). */
    includeNetworkInfo?: boolean;
    /** Called after each **`refresh`** completes (success or partial failure). */
    onUpdate?: (snap: BoingConnectionSnapshot) => void;
}
/**
 * Polls **`boing_health`** and optionally **`boing_getNetworkInfo`**, merges **`rpc_surface`**, and exposes **`lastRequestId`**.
 */
export declare class BoingConnectionMonitor {
    private readonly client;
    private readonly options;
    private snap;
    private timer;
    private readonly pollIntervalMs;
    private readonly includeNetworkInfo;
    constructor(client: BoingClient, options?: BoingConnectionMonitorOptions);
    getSnapshot(): BoingConnectionSnapshot;
    /** Single refresh; safe to call manually (e.g. on “Retry”). */
    refresh(): Promise<void>;
    /** Start periodic **`refresh`**. */
    start(): void;
    /** Stop polling (does not clear last snapshot). */
    stop(): void;
}
//# sourceMappingURL=connectionMonitor.d.ts.map