import { describe, expect, it } from 'vitest';
import type { BoingHealth, BoingHealthRpcSurface, NetworkInfo } from '../src/types.js';
import {
  clampGetLogsBlockSpan,
  describeGetLogsLimits,
  mergeRpcSurface,
  suggestedReadSpacingMs,
  websocketHasConnectionCap,
} from '../src/rpcSurfaceUi.js';

const surface: BoingHealthRpcSurface = {
  jsonrpc_batch_max: 32,
  websocket_max_connections: 0,
  http_rate_limit_requests_per_sec: 10,
  ready_min_peers: null,
  http_max_body_megabytes: 4,
  get_logs_max_block_range: 100,
  get_logs_max_results: 2000,
  max_log_topic_filters: 4,
};

describe('rpcSurfaceUi', () => {
  it('mergeRpcSurface prefers networkInfo', () => {
    const h: BoingHealth = {
      ok: true,
      client_version: 'x',
      chain_id: 1,
      chain_name: null,
      head_height: 0,
      rpc_surface: { ...surface, get_logs_max_block_range: 1 },
    };
    const n: Pick<NetworkInfo, 'rpc_surface'> = {
      rpc_surface: { ...surface, get_logs_max_block_range: 99 },
    };
    expect(mergeRpcSurface(h, n as NetworkInfo)?.get_logs_max_block_range).toBe(99);
    expect(mergeRpcSurface(h, null)?.get_logs_max_block_range).toBe(1);
    expect(mergeRpcSurface(null, null)).toBeNull();
  });

  it('clampGetLogsBlockSpan shrinks inclusive range when over max', () => {
    const r = clampGetLogsBlockSpan(surface, 0, 500);
    expect(r.clamped).toBe(true);
    expect(r.fromBlock).toBe(0);
    expect(r.toBlock).toBe(100);
  });

  it('suggestedReadSpacingMs follows RPS', () => {
    expect(suggestedReadSpacingMs({ ...surface, http_rate_limit_requests_per_sec: 0 })).toBe(0);
    expect(suggestedReadSpacingMs({ ...surface, http_rate_limit_requests_per_sec: 5 })).toBe(200);
  });

  it('describeGetLogsLimits and websocketHasConnectionCap', () => {
    expect(describeGetLogsLimits(surface)).toContain('100');
    expect(describeGetLogsLimits(null)).toContain('boing_health');
    expect(websocketHasConnectionCap(surface)).toBe(false);
    expect(websocketHasConnectionCap({ ...surface, websocket_max_connections: 50 })).toBe(true);
  });
});
