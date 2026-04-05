/**
 * Polling “connection” snapshot for dApp headers / settings screens (framework-agnostic).
 * Wire **`onUpdate`** into React/Vue/Svelte setState, or read **`getSnapshot()`** from a timer.
 */

import type { BoingClient } from './client.js';
import type { BoingHealth, BoingHealthRpcSurface, NetworkInfo } from './types.js';
import { mergeRpcSurface } from './rpcSurfaceUi.js';

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

function emptySnapshot(baseUrl: string): BoingConnectionSnapshot {
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
  private snap: BoingConnectionSnapshot;
  private timer: ReturnType<typeof setInterval> | null = null;
  private readonly pollIntervalMs: number;
  private readonly includeNetworkInfo: boolean;

  constructor(
    private readonly client: BoingClient,
    private readonly options: BoingConnectionMonitorOptions = {},
  ) {
    this.pollIntervalMs = Math.max(2000, options.pollIntervalMs ?? 12_000);
    this.includeNetworkInfo = options.includeNetworkInfo !== false;
    this.snap = emptySnapshot(client.getBaseUrl());
  }

  getSnapshot(): BoingConnectionSnapshot {
    return { ...this.snap, loading: this.snap.loading };
  }

  /** Single refresh; safe to call manually (e.g. on “Retry”). */
  async refresh(): Promise<void> {
    this.snap.loading = true;
    this.snap.baseUrl = this.client.getBaseUrl();
    this.options.onUpdate?.({ ...this.snap });

    let health: BoingHealth | null = null;
    let networkInfo: NetworkInfo | null = null;
    let lastErr: unknown;
    let networkInfoAttempted = false;

    try {
      health = await this.client.health();
    } catch (e) {
      lastErr = e;
    }

    if (this.includeNetworkInfo) {
      networkInfoAttempted = true;
      try {
        networkInfo = await this.client.getNetworkInfo();
      } catch (e) {
        lastErr = lastErr ?? e;
      }
    }

    const surface = mergeRpcSurface(health, networkInfo);
    const chainId = networkInfo?.chain_id ?? health?.chain_id ?? null;
    const headHeight = networkInfo?.head_height ?? health?.head_height ?? null;

    const failed =
      health == null || (networkInfoAttempted && networkInfo == null);

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
  start(): void {
    if (this.timer != null) return;
    void this.refresh();
    this.timer = setInterval(() => {
      void this.refresh();
    }, this.pollIntervalMs);
  }

  /** Stop polling (does not clear last snapshot). */
  stop(): void {
    if (this.timer != null) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }
}
