import { describe, expect, it, vi } from 'vitest';
import { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from '../src/canonicalTestnet.js';
import {
  CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX,
} from '../src/canonicalTestnetDex.js';
import {
  fetchNativeDexIntegrationDefaults,
  mergeNativeDexIntegrationDefaults,
} from '../src/dexIntegration.js';
import type { BoingClient } from '../src/client.js';
import type { NetworkInfo } from '../src/types.js';

function baseInfo(overrides: Partial<NetworkInfo>): NetworkInfo {
  return {
    chain_id: 6913,
    chain_name: 'Boing Testnet',
    head_height: 0,
    finalized_height: 0,
    latest_block_hash: '0x' + '00'.repeat(32),
    target_block_time_secs: 2,
    client_version: 'test',
    consensus: { validator_count: 1, model: 'hotstuff_bft' },
    native_currency: { symbol: 'BOING', decimals: 18 },
    chain_native: {
      account_count: 1,
      total_balance: '0',
      total_stake: '0',
      total_native_held: '0',
      as_of_height: 0,
    },
    developer: {
      repository_url: '',
      rpc_spec_url: '',
      dapp_integration_doc_url: '',
      sdk_npm_package: 'boing-sdk',
      websocket: { path: '/ws', handshake: { type: 'subscribe', channel: 'newHeads' }, event_types: ['newHead'] },
      api_discovery_methods: [],
      http: {
        live_path: '/live',
        ready_path: '/ready',
        jsonrpc_post_path: '/',
        response_header_rpc_version: 'x-boing-rpc-version',
        request_id_header: 'x-request-id',
        supports_jsonrpc_batch: true,
        jsonrpc_batch_max_env: 'BOING_RPC_MAX_BATCH',
        websocket_max_connections_env: 'BOING_RPC_WS_MAX_CONNECTIONS',
        ready_min_peers_env: 'BOING_RPC_READY_MIN_PEERS',
      },
    },
    rpc: { not_available: [], not_available_note: '' },
    ...overrides,
  };
}

describe('dexIntegration', () => {
  it('mergeNativeDexIntegrationDefaults does not substitute historical ids when live RPC left them unset', () => {
    const d = mergeNativeDexIntegrationDefaults(
      baseInfo({
        end_user: {
          chain_display_name: null,
          explorer_url: null,
          faucet_url: null,
        },
      }),
    );
    expect(d.nativeCpPoolAccountHex).toBeNull();
    expect(d.poolSource).toBe('none');
    expect(d.nativeDexFactoryAccountHex).toBeNull();
    expect(d.factorySource).toBe('none');
    expect(d.nativeDexMultihopSwapRouterAccountHex).toBeNull();
    expect(d.nativeDexLedgerRouterV2AccountHex).toBeNull();
    expect(d.nativeDexLedgerRouterV3AccountHex).toBeNull();
    expect(d.nativeAmmLpVaultAccountHex).toBeNull();
    expect(d.nativeLpShareTokenAccountHex).toBeNull();
    expect(d.endUserExplorerUrl).toBeNull();
  });

  it('mergeNativeDexIntegrationDefaults uses embedded ids only when network info is omitted', () => {
    const d = mergeNativeDexIntegrationDefaults(null);
    expect(d.nativeCpPoolAccountHex).toBe(CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX);
    expect(d.poolSource).toBe('sdk_testnet_embedded');
    expect(d.nativeDexFactoryAccountHex).toBe(CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX);
    expect(d.factorySource).toBe('sdk_testnet_embedded');
    expect(d.nativeDexMultihopSwapRouterAccountHex).toBe(CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX);
    expect(d.nativeDexMultihopSwapRouterSource).toBe('sdk_testnet_embedded');
    expect(d.nativeDexLedgerRouterV2AccountHex).toBe(CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX);
    expect(d.nativeDexLedgerRouterV2Source).toBe('sdk_testnet_embedded');
    expect(d.nativeDexLedgerRouterV3AccountHex).toBe(CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX);
    expect(d.nativeDexLedgerRouterV3Source).toBe('sdk_testnet_embedded');
    expect(d.nativeAmmLpVaultAccountHex).toBe(CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX);
    expect(d.nativeAmmLpVaultSource).toBe('sdk_testnet_embedded');
    expect(d.nativeLpShareTokenAccountHex).toBe(CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX);
    expect(d.nativeLpShareTokenSource).toBe('sdk_testnet_embedded');
  });

  it('mergeNativeDexIntegrationDefaults prefers RPC end_user pool over embedded', () => {
    const pool =
      '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' as `0x${string}`;
    const d = mergeNativeDexIntegrationDefaults(
      baseInfo({
        chain_id: 6913,
        end_user: {
          chain_display_name: null,
          explorer_url: null,
          faucet_url: null,
          canonical_native_cp_pool: pool,
        },
      }),
    );
    expect(d.nativeCpPoolAccountHex).toBe(pool);
    expect(d.poolSource).toBe('rpc_end_user');
  });

  it('mergeNativeDexIntegrationDefaults copies end_user explorer_url when https', () => {
    const d = mergeNativeDexIntegrationDefaults(
      baseInfo({
        end_user: {
          chain_display_name: null,
          explorer_url: 'https://custom.observer/',
          faucet_url: null,
        },
      }),
    );
    expect(d.endUserExplorerUrl).toBe('https://custom.observer');
  });

  it('mergeNativeDexIntegrationDefaults override wins', () => {
    const o =
      '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' as `0x${string}`;
    const d = mergeNativeDexIntegrationDefaults(
      baseInfo({
        end_user: {
          chain_display_name: null,
          explorer_url: null,
          faucet_url: null,
          canonical_native_cp_pool:
            '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        },
      }),
      { nativeCpPoolAccountHex: o },
    );
    expect(d.nativeCpPoolAccountHex).toBe(o);
    expect(d.poolSource).toBe('override');
  });

  it('fetchNativeDexIntegrationDefaults calls getNetworkInfo', async () => {
    const getNetworkInfo = vi.fn().mockResolvedValue(
      baseInfo({
        chain_id: 6913,
        end_user: { chain_display_name: null, explorer_url: null, faucet_url: null },
      }),
    );
    const client = { getNetworkInfo } as unknown as BoingClient;
    const d = await fetchNativeDexIntegrationDefaults(client);
    expect(getNetworkInfo).toHaveBeenCalledOnce();
    expect(d.poolSource).toBe('none');
    expect(d.nativeCpPoolAccountHex).toBeNull();
  });
});
