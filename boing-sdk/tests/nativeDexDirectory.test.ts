import { describe, expect, it, vi } from 'vitest';
import type { BoingClient } from '../src/client.js';
import {
  buildNativeDexRegisterLogPoolIndex,
  fetchNativeDexDirectorySnapshot,
  nativeDexPairKey,
  pickNativeDexPoolFromRegisterLogs,
  resolveNativeDexPoolForTokens,
} from '../src/nativeDexDirectory.js';
import { NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX } from '../src/nativeDexFactory.js';
import type { NativeDexFactoryRegisterRpcParsed } from '../src/nativeDexFactoryLogs.js';
import * as nativeDexFactoryPool from '../src/nativeDexFactoryPool.js';
import type { NetworkInfo } from '../src/types.js';
import type { Ed25519SecretKey32 } from '../src/transactionBuilder.js';

const T0 = '0x' + '11'.repeat(32) as `0x${string}`;
const T1 = '0x' + '22'.repeat(32) as `0x${string}`;
const POOL = '0x' + '33'.repeat(32) as `0x${string}`;
const FACTORY = '0x' + '44'.repeat(32) as `0x${string}`;

function baseNetworkInfo(overrides: Partial<NetworkInfo>): NetworkInfo {
  return {
    chain_id: 6913,
    chain_name: 'Boing Testnet',
    head_height: 100,
    finalized_height: 100,
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
      as_of_height: 100,
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

function regRow(
  tokenA: string,
  tokenB: string,
  pool: string,
  block = 1
): NativeDexFactoryRegisterRpcParsed {
  return {
    tokenAHex: tokenA,
    tokenBHex: tokenB,
    poolHex: pool,
    block_height: block,
    tx_index: 0,
    tx_id: '0x' + 'aa'.repeat(32),
    log_index: 0,
    address: FACTORY,
  };
}

describe('nativeDexDirectory', () => {
  it('nativeDexPairKey is order-independent', () => {
    expect(nativeDexPairKey(T0, T1)).toBe(nativeDexPairKey(T1, T0));
  });

  it('pickNativeDexPoolFromRegisterLogs matches either token order', () => {
    const logs = [regRow(T0, T1, POOL)];
    expect(pickNativeDexPoolFromRegisterLogs(logs, T0, T1)).toBe(POOL);
    expect(pickNativeDexPoolFromRegisterLogs(logs, T1, T0)).toBe(POOL);
  });

  it('buildNativeDexRegisterLogPoolIndex keeps last register for a pair', () => {
    const newer = '0x' + '55'.repeat(32) as `0x${string}`;
    const logs = [regRow(T0, T1, POOL, 1), regRow(T0, T1, newer, 2)];
    expect(pickNativeDexPoolFromRegisterLogs(logs, T0, T1)).toBe(newer);
    const idx = buildNativeDexRegisterLogPoolIndex(logs);
    expect(idx.get(nativeDexPairKey(T0, T1))).toBe(newer);
  });

  it('fetchNativeDexDirectorySnapshot reads count and optional logs (Boing RPC only)', async () => {
    const getNetworkInfo = vi.fn().mockResolvedValue(
      baseNetworkInfo({
        end_user: {
          chain_display_name: null,
          explorer_url: null,
          faucet_url: null,
          canonical_native_dex_factory: FACTORY,
        },
      }),
    );
    const countStorageWord = `0x${'00'.repeat(24)}0000000000000002`;
    const getContractStorage = vi.fn().mockResolvedValue({ value: countStorageWord });
    const getLogs = vi.fn().mockResolvedValue([
      {
        block_height: 10,
        tx_index: 0,
        tx_id: '0x' + 'bb'.repeat(32),
        log_index: 0,
        address: FACTORY,
        topics: [NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX, T0, T1],
        data: POOL,
      },
    ]);

    const client = { getNetworkInfo, getContractStorage, getLogs } as unknown as BoingClient;

    const snapNoLogs = await fetchNativeDexDirectorySnapshot(client);
    expect(getNetworkInfo).toHaveBeenCalled();
    expect(getContractStorage).toHaveBeenCalledWith(FACTORY, expect.any(String));
    expect(snapNoLogs.pairsCount).toBe(2n);
    expect(snapNoLogs.registerLogs).toBeNull();

    getContractStorage.mockClear();
    getNetworkInfo.mockClear();

    const snapLogs = await fetchNativeDexDirectorySnapshot(client, {
      registerLogs: { fromBlock: 0, toBlock: 50 },
    });
    expect(snapLogs.registerLogs?.length).toBe(1);
    expect(snapLogs.registerLogs?.[0]?.poolHex).toBe(POOL);
  });

  it('resolveNativeDexPoolForTokens kind=logs', async () => {
    const getNetworkInfo = vi.fn().mockResolvedValue(
      baseNetworkInfo({
        end_user: {
          chain_display_name: null,
          explorer_url: null,
          faucet_url: null,
          canonical_native_dex_factory: FACTORY,
        },
      }),
    );
    const getContractStorage = vi.fn().mockResolvedValue({ value: '0x' + '00'.repeat(32) });
    const getLogs = vi.fn().mockResolvedValue([
      {
        block_height: 10,
        tx_index: 0,
        tx_id: '0x' + 'bb'.repeat(32),
        log_index: 0,
        address: FACTORY,
        topics: [NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX, T0, T1],
        data: POOL,
      },
    ]);
    const client = { getNetworkInfo, getContractStorage, getLogs } as unknown as BoingClient;

    const r = await resolveNativeDexPoolForTokens(client, T0, T1, {
      kind: 'logs',
      fromBlock: 0,
      toBlock: 20,
    });
    expect(r.via).toBe('logs');
    expect(r.poolHex).toBe(POOL);
    expect(r.factoryHex).toBe(FACTORY);
  });

  it('resolveNativeDexPoolForTokens kind=simulate delegates to findNativeDexFactoryPoolByTokens', async () => {
    const getNetworkInfo = vi.fn().mockResolvedValue(
      baseNetworkInfo({
        end_user: {
          chain_display_name: null,
          explorer_url: null,
          faucet_url: null,
          canonical_native_dex_factory: FACTORY,
        },
      }),
    );
    const spy = vi.spyOn(nativeDexFactoryPool, 'findNativeDexFactoryPoolByTokens').mockResolvedValue(POOL);
    const client = { getNetworkInfo } as unknown as BoingClient;
    const secretKey = new Uint8Array(32) as Ed25519SecretKey32;
    try {
      const r = await resolveNativeDexPoolForTokens(client, T0, T1, {
        kind: 'simulate',
        find: { secretKey32: secretKey, senderHex: T0 },
      });
      expect(r.via).toBe('simulate');
      expect(r.poolHex).toBe(POOL);
      expect(spy).toHaveBeenCalledWith(client, FACTORY, T0, T1, expect.objectContaining({ senderHex: T0 }));
    } finally {
      spy.mockRestore();
    }
  });

  it('resolveNativeDexPoolForTokens kind=auto falls back to simulate when logs miss', async () => {
    const getNetworkInfo = vi.fn().mockResolvedValue(
      baseNetworkInfo({
        end_user: {
          chain_display_name: null,
          explorer_url: null,
          faucet_url: null,
          canonical_native_dex_factory: FACTORY,
        },
      }),
    );
    const getContractStorage = vi.fn().mockResolvedValue({ value: '0x' + '00'.repeat(32) });
    const getLogs = vi.fn().mockResolvedValue([]);
    const spy = vi.spyOn(nativeDexFactoryPool, 'findNativeDexFactoryPoolByTokens').mockResolvedValue(POOL);
    const client = { getNetworkInfo, getContractStorage, getLogs } as unknown as BoingClient;
    const secretKey = new Uint8Array(32) as Ed25519SecretKey32;
    try {
      const r = await resolveNativeDexPoolForTokens(client, T0, T1, {
        kind: 'auto',
        fromBlock: 0,
        toBlock: 20,
        find: { secretKey32: secretKey, senderHex: T0 },
      });
      expect(r.via).toBe('simulate');
      expect(r.poolHex).toBe(POOL);
    } finally {
      spy.mockRestore();
    }
  });
});
