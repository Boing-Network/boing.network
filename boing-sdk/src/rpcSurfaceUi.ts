/**
 * UI-oriented helpers from **`boing_health.rpc_surface`** / **`boing_getNetworkInfo.rpc_surface`**.
 */

import type { BoingHealthRpcSurface } from './types.js';

/** Prefer **`networkInfo.rpc_surface`**, then **`health.rpc_surface`**. */
export function mergeRpcSurface(
  health: { rpc_surface?: BoingHealthRpcSurface } | null,
  networkInfo: { rpc_surface?: BoingHealthRpcSurface } | null,
): BoingHealthRpcSurface | null {
  return networkInfo?.rpc_surface ?? health?.rpc_surface ?? null;
}

/**
 * Clamp an inclusive block span to the node’s **`get_logs_max_block_range`** (inclusive pair length).
 */
export function clampGetLogsBlockSpan(
  surface: BoingHealthRpcSurface | null | undefined,
  fromBlock: number,
  toBlock: number,
): { fromBlock: number; toBlock: number; clamped: boolean } {
  const lo = Math.min(fromBlock, toBlock);
  const hi = Math.max(fromBlock, toBlock);
  const span = hi - lo;
  const max = surface?.get_logs_max_block_range;
  if (max == null || max <= 0 || span <= max) {
    return { fromBlock: lo, toBlock: hi, clamped: false };
  }
  const newHi = lo + max;
  return { fromBlock: lo, toBlock: newHi, clamped: true };
}

/**
 * Suggested minimum delay between steady-state read RPCs when HTTP JSON-RPC rate limiting is enabled (**RPS > 0**).
 * Returns **`0`** when unlimited.
 */
export function suggestedReadSpacingMs(surface: BoingHealthRpcSurface | null | undefined): number {
  const rps = surface?.http_rate_limit_requests_per_sec ?? 0;
  if (rps <= 0) return 0;
  return Math.ceil(1000 / rps);
}

/**
 * Short tooltip copy for logs cap (span and max rows).
 */
export function describeGetLogsLimits(surface: BoingHealthRpcSurface | null | undefined): string {
  if (surface == null) {
    return 'Log query limits depend on the node; call boing_health for rpc_surface.';
  }
  const span = surface.get_logs_max_block_range;
  const n = surface.get_logs_max_results;
  return `This RPC allows up to ${span} blocks per query and up to ${n} log rows per response.`;
}

/**
 * Whether the node advertises a finite WebSocket subscriber cap (non-zero).
 */
export function websocketHasConnectionCap(surface: BoingHealthRpcSurface | null | undefined): boolean {
  const n = surface?.websocket_max_connections ?? 0;
  return n > 0;
}
