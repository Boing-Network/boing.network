import { describe, expect, it, vi } from 'vitest';
import type { BoingClient } from '../src/client.js';
import type { BoingHealth, NetworkInfo } from '../src/types.js';
import { BoingConnectionMonitor } from '../src/connectionMonitor.js';

describe('BoingConnectionMonitor', () => {
  it('refresh merges rpc_surface and copies request id', async () => {
    const health: BoingHealth = {
      ok: true,
      client_version: 'v',
      chain_id: 1,
      chain_name: null,
      head_height: 10,
      rpc_surface: {
        jsonrpc_batch_max: 32,
        websocket_max_connections: 0,
        http_rate_limit_requests_per_sec: 0,
        ready_min_peers: null,
        http_max_body_megabytes: 4,
        get_logs_max_block_range: 50,
        get_logs_max_results: 1000,
        max_log_topic_filters: 4,
      },
    };
    const networkInfo: NetworkInfo = {
      chain_id: 2,
      chain_name: 'net',
      head_height: 11,
      finalized_height: 11,
      latest_block_hash: `0x${'11'.repeat(32)}`,
      target_block_time_secs: 1,
      client_version: 'v',
      consensus: { validator_count: 1, model: 'm' },
      native_currency: { symbol: 'B', decimals: 18 },
      chain_native: {
        account_count: 0,
        total_balance: '0',
        total_stake: '0',
        total_native_held: '0',
        as_of_height: 11,
      },
      developer: {
        repository_url: '',
        rpc_spec_url: '',
        dapp_integration_doc_url: '',
        sdk_npm_package: '',
        websocket: {
          path: '/ws',
          handshake: { type: 'subscribe', channel: 'newHeads' },
          event_types: [],
        },
        api_discovery_methods: [],
        http: {
          live_path: '/live',
          ready_path: '/ready',
          jsonrpc_post_path: '/',
          response_header_rpc_version: 'X',
          request_id_header: 'x-request-id',
          supports_jsonrpc_batch: true,
          jsonrpc_batch_max_env: 'A',
          websocket_max_connections_env: 'B',
          ready_min_peers_env: 'C',
        },
      },
      rpc_surface: {
        jsonrpc_batch_max: 32,
        websocket_max_connections: 0,
        http_rate_limit_requests_per_sec: 0,
        ready_min_peers: null,
        http_max_body_megabytes: 4,
        get_logs_max_block_range: 77,
        get_logs_max_results: 1000,
        max_log_topic_filters: 4,
      },
      rpc: { not_available: [], not_available_note: '' },
    };

    const client = {
      getBaseUrl: () => 'http://127.0.0.1:8545',
      getLastXRequestId: () => 'abc',
      health: vi.fn().mockResolvedValue(health),
      getNetworkInfo: vi.fn().mockResolvedValue(networkInfo),
    } as unknown as BoingClient;

    const mon = new BoingConnectionMonitor(client);
    await mon.refresh();
    const s = mon.getSnapshot();
    expect(s.health).toEqual(health);
    expect(s.networkInfo).toEqual(networkInfo);
    expect(s.chainId).toBe(2);
    expect(s.headHeight).toBe(11);
    expect(s.rpcSurface?.get_logs_max_block_range).toBe(77);
    expect(s.lastRequestId).toBe('abc');
    expect(s.lastError).toBeUndefined();
    expect(s.loading).toBe(false);
  });

  it('sets lastError when health fails', async () => {
    const err = new Error('down');
    const client = {
      getBaseUrl: () => 'http://x',
      getLastXRequestId: () => undefined,
      health: vi.fn().mockRejectedValue(err),
      getNetworkInfo: vi.fn(),
    } as unknown as BoingClient;

    const mon = new BoingConnectionMonitor(client, { includeNetworkInfo: false });
    await mon.refresh();
    const s = mon.getSnapshot();
    expect(s.health).toBeNull();
    expect(s.lastError).toBe(err);
    expect(client.getNetworkInfo).not.toHaveBeenCalled();
  });
});
