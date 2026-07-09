import { describe, expect, it, vi } from 'vitest';
import {
  createNativeDexDiscovery,
  readNativeDexDirectoryBaseUrlFromProcessEnv,
} from '../src/nativeDexDiscovery.js';
import type { BoingClient } from '../src/client.js';
import type { NetworkInfo } from '../src/types.js';

function mockNetworkInfo(methods: string[]): NetworkInfo {
  return {
    chain_id: 6913,
    chain_name: 'Boing Testnet',
    head_height: 1,
    finalized_height: 1,
    latest_block_hash: '0x' + '11'.repeat(32),
    target_block_time_secs: 2,
    client_version: 'boing-node/test',
    consensus: { validator_count: 1, model: 'hotstuff' },
    native_currency: { symbol: 'BOING', decimals: 18 },
    chain_native: {
      account_count: 1,
      total_balance: '0',
      total_stake: '0',
      total_native_held: '0',
    },
    developer: {
      repository_url: 'https://github.com/Boing-Network/boing.network',
      rpc_spec_url: '',
      dapp_integration_doc_url: '',
      sdk_npm_package: 'boing-sdk',
      websocket: {
        path: '/ws',
        handshake: { type: 'subscribe', channel: 'newHeads' },
        event_types: ['newHead'],
      },
      api_discovery_methods: [],
      dex_discovery_methods: methods,
      http: {
        live_path: '/live',
        ready_path: '/ready',
        jsonrpc_post_path: '/',
        response_header_rpc_version: 'X-Boing-Rpc-Version',
        request_id_header: 'X-Request-Id',
        supports_jsonrpc_batch: true,
        jsonrpc_batch_max_env: 'BOING_RPC_MAX_BATCH',
        websocket_max_connections_env: 'BOING_RPC_WS_MAX_CONNECTIONS',
        ready_min_peers_env: 'BOING_RPC_READY_MIN_PEERS',
      },
    },
    end_user: {},
    rpc: { not_available: [], not_available_note: '' },
  };
}

describe('nativeDexDiscovery', () => {
  it('readNativeDexDirectoryBaseUrlFromProcessEnv reads BOING_ key', () => {
    const prev = process.env.BOING_NATIVE_DEX_DIRECTORY_BASE_URL;
    process.env.BOING_NATIVE_DEX_DIRECTORY_BASE_URL = 'https://example.com/dir/';
    expect(readNativeDexDirectoryBaseUrlFromProcessEnv()).toBe('https://example.com/dir/');
    if (prev === undefined) delete process.env.BOING_NATIVE_DEX_DIRECTORY_BASE_URL;
    else process.env.BOING_NATIVE_DEX_DIRECTORY_BASE_URL = prev;
  });

  it('createNativeDexDiscovery prefers rpc when listDexPools is advertised', async () => {
    const client = {
      getNetworkInfo: vi.fn(async () => mockNetworkInfo(['boing_listDexPools', 'boing_listDexTokens'])),
      listDexPoolsPage: vi.fn(async () => ({
        pools: [
          {
            poolHex: '0x' + 'aa'.repeat(32),
            tokenAHex: '0x' + 'bb'.repeat(32),
            tokenBHex: '0x' + 'cc'.repeat(32),
            tokenADecimals: 18,
            tokenBDecimals: 18,
            feeBps: 30,
            reserveA: '1',
            reserveB: '2',
            createdAtHeight: 1,
          },
        ],
        nextCursor: null,
      })),
      listDexTokensPage: vi.fn(),
      getDexToken: vi.fn(),
    } as unknown as BoingClient;

    const d = await createNativeDexDiscovery(client, {
      useProcessEnvOverrides: false,
      overrides: {},
    });
    expect(d.rpcPoolsSupported).toBe(true);
    expect(d.preferredPoolSource).toBe('rpc');
    const all = await d.collectAllPools();
    expect(all.source).toBe('rpc');
    expect(all.pools).toHaveLength(1);
  });

  it('createNativeDexDiscovery falls back to http when rpc missing but base URL set', async () => {
    const client = {
      getNetworkInfo: vi.fn(async () => mockNetworkInfo([])),
      listDexPoolsPage: vi.fn(),
      listDexTokensPage: vi.fn(),
      getDexToken: vi.fn(),
    } as unknown as BoingClient;

    const d = await createNativeDexDiscovery(client, {
      useProcessEnvOverrides: false,
      directoryBaseUrl: 'https://dir.example/',
    });
    expect(d.rpcPoolsSupported).toBe(false);
    expect(d.preferredPoolSource).toBe('http');
    expect(d.directoryBaseUrl).toContain('https://dir.example');
  });

  it('createNativeDexDiscovery prefers logs when neither rpc nor http', async () => {
    const client = {
      getNetworkInfo: vi.fn(async () => mockNetworkInfo([])),
      listDexPoolsPage: vi.fn(),
      listDexTokensPage: vi.fn(),
      getDexToken: vi.fn(),
    } as unknown as BoingClient;

    const d = await createNativeDexDiscovery(client, { useProcessEnvOverrides: false });
    expect(d.preferredPoolSource).toBe('logs');
    const all = await d.collectAllPools();
    expect(all.source).toBe('none');
    expect(all.pools).toEqual([]);
  });
});
