import { describe, expect, it } from 'vitest';
import { mergeNativeDexIntegrationDefaults } from '../src/dexIntegration.js';
import {
  applyNativeDexMultihopSimulationToContractCallTx,
  BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS,
  buildNativeCpPoolSwapExpressTx,
  buildNativeDexMultihopSwapExpressTxFromRoute128,
  describeNativeDexDefaultGaps,
  formatBoingNativeDexNotEvmDisclaimer,
  formatNativeDexToolkitPreflightForUi,
} from '../src/nativeDexSeamless.js';
import { findBestCpRoute, type CpPoolVenue } from '../src/nativeDexRouting.js';
import { BoingRpcPreflightError } from '../src/preflightGate.js';
import type { NetworkInfo } from '../src/types.js';

function miniInfo(): NetworkInfo {
  return {
    chain_id: 1,
    chain_name: 'x',
    head_height: 0,
    finalized_height: 0,
    latest_block_hash: '0x' + '00'.repeat(32),
    target_block_time_secs: 2,
    client_version: 't',
    consensus: { validator_count: 1, model: 'single_round_quorum_commit' },
    native_currency: { symbol: 'B', decimals: 18 },
    chain_native: {
      account_count: 0,
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
        jsonrpc_batch_max_env: '',
        websocket_max_connections_env: '',
        ready_min_peers_env: '',
      },
    },
    rpc: { not_available: [], not_available_note: '' },
    end_user: { chain_display_name: null, explorer_url: null, faucet_url: null },
  };
}

describe('nativeDexSeamless', () => {
  it('formatBoingNativeDexNotEvmDisclaimer includes tagline and bullets', () => {
    const s = formatBoingNativeDexNotEvmDisclaimer();
    expect(s).toContain('32-byte');
    expect(s).toContain('• ');
  });

  it('describeNativeDexDefaultGaps flags missing pool, factory, and multihop router', () => {
    const d = mergeNativeDexIntegrationDefaults(miniInfo());
    const g = describeNativeDexDefaultGaps(d);
    expect(g.length).toBe(3);
    expect(g.some((x) => x.includes('pool'))).toBe(true);
    expect(g.some((x) => x.includes('factory'))).toBe(true);
    expect(g.some((x) => x.includes('multihop'))).toBe(true);
  });

  it('BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS lists expected methods', () => {
    expect(BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS).toContain('boing_simulateTransaction');
    expect(BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS).toContain('boing_getLogs');
  });

  it('formatNativeDexToolkitPreflightForUi joins doctor messages', () => {
    const doctor = {
      ok: false,
      preflight: {} as import('../src/types.js').BoingRpcPreflightResult,
      capabilityProbe: {} as import('../src/rpcCapabilities.js').BoingRpcProbeBundle,
      missingRequiredMethods: [],
      messages: ['alpha', 'bravo'],
    };
    const err = new BoingRpcPreflightError(doctor);
    const u = formatNativeDexToolkitPreflightForUi(err);
    expect(u).toContain('alpha');
    expect(u).toContain('bravo');
    expect(u).toContain('protocol QA');
  });

  it('buildNativeDexMultihopSwapExpressTxFromRoute128 includeVenueTokenAccounts adds token ids', () => {
    const TA = ('0x' + '11'.repeat(32)) as `0x${string}`;
    const TB = ('0x' + '22'.repeat(32)) as `0x${string}`;
    const TC = ('0x' + '33'.repeat(32)) as `0x${string}`;
    const P1 = ('0x' + 'aa'.repeat(32)) as `0x${string}`;
    const P2 = ('0x' + 'bb'.repeat(32)) as `0x${string}`;
    const ROUTER = ('0x' + 'de'.repeat(32)) as `0x${string}`;
    const SENDER = ('0x' + '01'.repeat(32)) as `0x${string}`;
    function venue(
      poolHex: `0x${string}`,
      tokenAHex: `0x${string}`,
      tokenBHex: `0x${string}`,
      reserveA: bigint,
      reserveB: bigint
    ): CpPoolVenue {
      return { poolHex, tokenAHex, tokenBHex, reserveA, reserveB, feeBps: 30n };
    }
    const v1 = venue(P1, TA, TB, 100_000n, 100_000n);
    const v2 = venue(P2, TB, TC, 100_000n, 100_000n);
    const route = findBestCpRoute([v1, v2], TA, TC, 1000n, { maxHops: 2 })!;
    const tx = buildNativeDexMultihopSwapExpressTxFromRoute128({
      senderHex32: SENDER,
      routerHex32: ROUTER,
      route,
      slippageBps: 50n,
      includeVenueTokenAccounts: true,
    });
    expect(tx.access_list.read).toContain(TA.toLowerCase());
    expect(tx.access_list.read).toContain(TB.toLowerCase());
    expect(tx.access_list.read).toContain(TC.toLowerCase());
  });

  it('applyNativeDexMultihopSimulationToContractCallTx widens access list', () => {
    const TA = ('0x' + '11'.repeat(32)) as `0x${string}`;
    const TB = ('0x' + '22'.repeat(32)) as `0x${string}`;
    const TC = ('0x' + '33'.repeat(32)) as `0x${string}`;
    const P1 = ('0x' + 'aa'.repeat(32)) as `0x${string}`;
    const P2 = ('0x' + 'bb'.repeat(32)) as `0x${string}`;
    const ROUTER = ('0x' + 'de'.repeat(32)) as `0x${string}`;
    const SENDER = ('0x' + '01'.repeat(32)) as `0x${string}`;
    const TOKEN = ('0x' + '99'.repeat(32)) as `0x${string}`;
    function venue(
      poolHex: `0x${string}`,
      tokenAHex: `0x${string}`,
      tokenBHex: `0x${string}`,
      reserveA: bigint,
      reserveB: bigint
    ): CpPoolVenue {
      return { poolHex, tokenAHex, tokenBHex, reserveA, reserveB, feeBps: 30n };
    }
    const v1 = venue(P1, TA, TB, 100_000n, 100_000n);
    const v2 = venue(P2, TB, TC, 100_000n, 100_000n);
    const route = findBestCpRoute([v1, v2], TA, TC, 1000n, { maxHops: 2 })!;
    let tx = buildNativeDexMultihopSwapExpressTxFromRoute128({
      senderHex32: SENDER,
      routerHex32: ROUTER,
      route,
      slippageBps: 10n,
    });
    expect(tx.access_list.read).not.toContain(TOKEN.toLowerCase());
    const sim = {
      suggested_access_list: { read: [TOKEN], write: [] },
    } as import('../src/types.js').SimulateResult;
    tx = applyNativeDexMultihopSimulationToContractCallTx(tx, {
      senderHex32: SENDER,
      poolHex32List: [P1, P2],
      sim,
    });
    expect(tx.access_list.read).toContain(TOKEN.toLowerCase());
  });

  it('buildNativeDexMultihopSwapExpressTxFromRoute128 targets router with slippage', () => {
    const TA = ('0x' + '11'.repeat(32)) as `0x${string}`;
    const TB = ('0x' + '22'.repeat(32)) as `0x${string}`;
    const TC = ('0x' + '33'.repeat(32)) as `0x${string}`;
    const P1 = ('0x' + 'aa'.repeat(32)) as `0x${string}`;
    const P2 = ('0x' + 'bb'.repeat(32)) as `0x${string}`;
    const ROUTER = ('0x' + 'de'.repeat(32)) as `0x${string}`;
    const SENDER = ('0x' + '01'.repeat(32)) as `0x${string}`;
    function venue(
      poolHex: `0x${string}`,
      tokenAHex: `0x${string}`,
      tokenBHex: `0x${string}`,
      reserveA: bigint,
      reserveB: bigint
    ): CpPoolVenue {
      return { poolHex, tokenAHex, tokenBHex, reserveA, reserveB, feeBps: 30n };
    }
    const v1 = venue(P1, TA, TB, 100_000n, 100_000n);
    const v2 = venue(P2, TB, TC, 100_000n, 100_000n);
    const route = findBestCpRoute([v1, v2], TA, TC, 1000n, { maxHops: 2 })!;
    const tx = buildNativeDexMultihopSwapExpressTxFromRoute128({
      senderHex32: SENDER,
      routerHex32: ROUTER,
      route,
      slippageBps: 100n,
    });
    expect(tx.type).toBe('contract_call');
    expect(tx.contract).toBe(ROUTER.toLowerCase());
    expect(tx.access_list.read).toContain(SENDER.toLowerCase());
    expect(tx.access_list.read).toContain(ROUTER.toLowerCase());
    expect(tx.access_list.read).toContain(P1.toLowerCase());
    expect(tx.access_list.read).toContain(P2.toLowerCase());
  });

  it('buildNativeCpPoolSwapExpressTx builds contract_call with swap calldata', () => {
    const sender = '0x' + '01'.repeat(32);
    const pool = '0x' + '02'.repeat(32);
    const tx = buildNativeCpPoolSwapExpressTx({
      senderHex32: sender,
      poolHex32: pool,
      direction: 0n,
      amountIn: 100n,
      minOut: 1n,
    });
    expect(tx.type).toBe('contract_call');
    expect(tx.contract).toBe(pool.toLowerCase());
    expect(tx.calldata.startsWith('0x')).toBe(true);
    expect(tx.access_list.read).toContain(sender.toLowerCase());
    expect(tx.access_list.read).toContain(pool.toLowerCase());
  });
});
