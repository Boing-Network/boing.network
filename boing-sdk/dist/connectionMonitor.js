/**
 * Polling “connection” snapshot for dApp headers / settings screens (framework-agnostic).
 * Wire **`onUpdate`** into React/Vue/Svelte setState, or read **`getSnapshot()`** from a timer.
 */
import { mergeRpcSurface } from './rpcSurfaceUi.js';
function emptySnapshot(baseUrl) {
    return {
        baseUrl,
        updatedAtMs: 0,
        loading: false,
        health: null,
        networkInfo: null,
        chainId: null,
        headHeight: null,
        rpcSurface: null,
        lastError: undefined,
        lastRequestId: undefined,
    };
}
/**
 * Polls **`boing_health`** and optionally **`boing_getNetworkInfo`**, merges **`rpc_surface`**, and exposes **`lastRequestId`**.
 */
export class BoingConnectionMonitor {
    constructor(client, options = {}) {
        this.client = client;
        this.options = options;
        this.timer = null;
        this.pollIntervalMs = Math.max(2000, options.pollIntervalMs ?? 12000);
        this.includeNetworkInfo = options.includeNetworkInfo !== false;
        this.snap = emptySnapshot(client.getBaseUrl());
    }
    getSnapshot() {
        return { ...this.snap, loading: this.snap.loading };
    }
    /** Single refresh; safe to call manually (e.g. on “Retry”). */
    async refresh() {
        this.snap.loading = true;
        this.snap.baseUrl = this.client.getBaseUrl();
        this.options.onUpdate?.({ ...this.snap });
        let health = null;
        let networkInfo = null;
        let lastErr;
        let networkInfoAttempted = false;
        try {
            health = await this.client.health();
        }
        catch (e) {
            lastErr = e;
        }
        if (this.includeNetworkInfo) {
            networkInfoAttempted = true;
            try {
                networkInfo = await this.client.getNetworkInfo();
            }
            catch (e) {
                lastErr = lastErr ?? e;
            }
        }
        const surface = mergeRpcSurface(health, networkInfo);
        const chainId = networkInfo?.chain_id ?? health?.chain_id ?? null;
        const headHeight = networkInfo?.head_height ?? health?.head_height ?? null;
        const failed = health == null || (networkInfoAttempted && networkInfo == null);
        this.snap = {
            baseUrl: this.client.getBaseUrl(),
            updatedAtMs: Date.now(),
            loading: false,
            health,
            networkInfo,
            chainId,
            headHeight,
            rpcSurface: surface,
            lastError: failed ? lastErr : undefined,
            lastRequestId: this.client.getLastXRequestId(),
        };
        this.options.onUpdate?.(this.getSnapshot());
    }
    /** Start periodic **`refresh`**. */
    start() {
        if (this.timer != null)
            return;
        void this.refresh();
        this.timer = setInterval(() => {
            void this.refresh();
        }, this.pollIntervalMs);
    }
    /** Stop polling (does not clear last snapshot). */
    stop() {
        if (this.timer != null) {
            clearInterval(this.timer);
            this.timer = null;
        }
    }
}
