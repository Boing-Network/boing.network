import { describe, expect, it, vi } from 'vitest';
import type { BoingClient } from '../src/client.js';
import {
  encodeNativeDexMultihopRouterCalldata128FromRoute,
  encodeNativeDexMultihopRouterCalldata128FromRouteWithSlippage,
  encodeNativeDexMultihopRouterCalldata160FromRoute,
  fetchCpRoutingFromDirectoryLogs,
  findBestCpRoute,
  findBestCpRoutes,
  hydrateCpPoolVenuesFromRpc,
  minOutFloorAfterSlippageBps,
  minOutPerHopFromQuotedRouteSlippageBps,
  NATIVE_DEX_MULTIHOP_ROUTER_MAX_POOLS,
  NATIVE_DEX_SLIPPAGE_BPS_SCALE,
  pickFirstMultihopCpRoute,
  quoteCpEvenSplitAcrossDirectPools,
  quoteCpPoolSwap,
  rankDirectCpPools,
  uniqueSortedTokenHex32FromCpRoute,
  type CpPoolVenue,
} from '../src/nativeDexRouting.js';
import { hexToBytes } from '../src/hex.js';
import { NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX } from '../src/nativeDexFactory.js';
import { NATIVE_DEX_FACTORY_COUNT_KEY_HEX } from '../src/nativeDexFactoryPool.js';
import type { NetworkInfo } from '../src/types.js';
import {
  NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX,
} from '../src/nativeAmmPool.js';

const TA = ('0x' + '11'.repeat(32)) as `0x${string}`;
const TB = ('0x' + '22'.repeat(32)) as `0x${string}`;
const TC = ('0x' + '33'.repeat(32)) as `0x${string}`;
const TD = ('0x' + '44'.repeat(32)) as `0x${string}`;
const TE = ('0x' + '55'.repeat(32)) as `0x${string}`;
const TF = ('0x' + '66'.repeat(32)) as `0x${string}`;
const TG = ('0x' + '77'.repeat(32)) as `0x${string}`;
const P1 = ('0x' + 'aa'.repeat(32)) as `0x${string}`;
const P2 = ('0x' + 'bb'.repeat(32)) as `0x${string}`;
const P3 = ('0x' + 'c3'.repeat(32)) as `0x${string}`;
const P4 = ('0x' + 'c4'.repeat(32)) as `0x${string}`;
const P5 = ('0x' + 'c5'.repeat(32)) as `0x${string}`;
const P6 = ('0x' + 'c6'.repeat(32)) as `0x${string}`;
const ROUTER = ('0x' + 'de'.repeat(32)) as `0x${string}`;
const TRADER = ('0x' + 'f0'.repeat(32)) as `0x${string}`;
const FACTORY = ('0x' + 'dd'.repeat(32)) as `0x${string}`;

function wordU128(n: bigint): string {
  const be = new Uint8Array(16);
  let x = n;
  for (let i = 15; i >= 0; i--) {
    be[i] = Number(x & 0xffn);
    x >>= 8n;
  }
  let low = '';
  for (let i = 0; i < 16; i++) {
    low += be[i]!.toString(16).padStart(2, '0');
  }
  return `0x${'00'.repeat(16)}${low}`;
}

function venue(
  poolHex: `0x${string}`,
  tokenAHex: `0x${string}`,
  tokenBHex: `0x${string}`,
  reserveA: bigint,
  reserveB: bigint,
  feeBps = 30n
): CpPoolVenue {
  return { poolHex, tokenAHex, tokenBHex, reserveA, reserveB, feeBps };
}

describe('nativeDexRouting', () => {
  it('pickFirstMultihopCpRoute skips single-hop routes', () => {
    const v = venue(P1, TA, TB, 10_000n, 10_000n);
    const direct = findBestCpRoutes([v], TA, TB, 100n, { maxHops: 1 });
    expect(pickFirstMultihopCpRoute(direct)).toBeUndefined();
    const v2 = venue(P2, TB, TC, 10_000n, 10_000n);
    const multi = findBestCpRoutes([v, v2], TA, TC, 100n, { maxHops: 2 });
    const picked = pickFirstMultihopCpRoute(multi);
    expect(picked).toBeDefined();
    expect(picked!.hops.length).toBe(2);
  });

  it('uniqueSortedTokenHex32FromCpRoute dedupes venue tokens', () => {
    const v1 = venue(P1, TA, TB, 10_000n, 10_000n);
    const v2 = venue(P2, TB, TC, 10_000n, 10_000n);
    const route = findBestCpRoute([v1, v2], TA, TC, 100n, { maxHops: 2 })!;
    const u = uniqueSortedTokenHex32FromCpRoute(route);
    expect(u.length).toBe(3);
    expect(u).toContain(TA.toLowerCase());
    expect(u).toContain(TB.toLowerCase());
    expect(u).toContain(TC.toLowerCase());
  });

  it('minOutFloorAfterSlippageBps scales by bps floor', () => {
    expect(minOutFloorAfterSlippageBps(10_000n, 0n)).toBe(10_000n);
    expect(minOutFloorAfterSlippageBps(10_000n, 100n)).toBe(9_900n);
    expect(minOutFloorAfterSlippageBps(10_000n, NATIVE_DEX_SLIPPAGE_BPS_SCALE)).toBe(0n);
  });

  it('encodeNativeDexMultihopRouterCalldata128FromRouteWithSlippage matches explicit minOut', () => {
    const v1 = venue(P1, TA, TB, 100_000n, 100_000n);
    const v2 = venue(P2, TB, TC, 100_000n, 100_000n);
    const route = findBestCpRoute([v1, v2], TA, TC, 1000n, { maxHops: 2 })!;
    const slip = 50n;
    const a = encodeNativeDexMultihopRouterCalldata128FromRouteWithSlippage(route, slip);
    const b = encodeNativeDexMultihopRouterCalldata128FromRoute(route, {
      minOutPerHop: minOutPerHopFromQuotedRouteSlippageBps(route, slip),
    });
    expect(a.length).toBe(b.length);
    expect([...a]).toEqual([...b]);
  });

  it('quoteCpPoolSwap A to B', () => {
    const v = venue(P1, TA, TB, 10_000n, 10_000n);
    const q = quoteCpPoolSwap(v, TA, 1000n);
    expect(q.tokenOutHex).toBe(TB.toLowerCase());
    expect(q.directionForSwapCalldata).toBe(0n);
    expect(q.amountOut).toBeGreaterThan(0n);
  });

  it('rankDirectCpPools prefers deeper liquidity', () => {
    const shallow = venue(P1, TA, TB, 100n, 100n);
    const deep = venue(P2, TA, TB, 100_000n, 100_000n);
    const r = rankDirectCpPools([shallow, deep], TA, TB, 50n);
    expect(r[0]!.venue.poolHex).toBe(P2);
  });

  it('findBestCpRoute finds two-hop path', () => {
    const v1 = venue(P1, TA, TB, 100_000n, 100_000n);
    const v2 = venue(P2, TB, TC, 100_000n, 100_000n);
    const route = findBestCpRoute([v1, v2], TA, TC, 1000n, { maxHops: 2 });
    expect(route).toBeDefined();
    expect(route!.hops.length).toBe(2);
    expect(route!.amountOut).toBeGreaterThan(0n);
    expect(route!.hops[0]!.tokenInHex.toLowerCase()).toBe(TA.toLowerCase());
    expect(route!.hops[1]!.tokenInHex.toLowerCase()).toBe(TB.toLowerCase());
  });

  it('findBestCpRoutes respects maxRoutes', () => {
    const v = venue(P1, TA, TB, 50_000n, 50_000n);
    const routes = findBestCpRoutes([v], TA, TB, 100n, { maxHops: 1, maxRoutes: 1 });
    expect(routes.length).toBe(1);
  });

  it('findBestCpRoutes rejects maxHops above multihop router cap', () => {
    const v = venue(P1, TA, TB, 10_000n, 10_000n);
    expect(() => findBestCpRoutes([v], TA, TB, 1n, { maxHops: NATIVE_DEX_MULTIHOP_ROUTER_MAX_POOLS + 1 })).toThrow(
      /cannot exceed/
    );
  });

  it('findBestCpRoute finds five-hop path when maxHops allows', () => {
    const v1 = venue(P1, TA, TB, 500_000n, 500_000n);
    const v2 = venue(P2, TB, TC, 500_000n, 500_000n);
    const v3 = venue(P3, TC, TD, 500_000n, 500_000n);
    const v4 = venue(P4, TD, TE, 500_000n, 500_000n);
    const v5 = venue(P5, TE, TF, 500_000n, 500_000n);
    const route = findBestCpRoute([v1, v2, v3, v4, v5], TA, TF, 100n, { maxHops: 6 });
    expect(route).toBeDefined();
    expect(route!.hops.length).toBe(5);
    expect(route!.amountOut).toBeGreaterThan(0n);
  });

  it('encodeNativeDexMultihopRouterCalldata128FromRoute builds swap2 outer', () => {
    const v1 = venue(P1, TA, TB, 100_000n, 100_000n);
    const v2 = venue(P2, TB, TC, 100_000n, 100_000n);
    const route = findBestCpRoute([v1, v2], TA, TC, 1000n, { maxHops: 2 })!;
    const cd = encodeNativeDexMultihopRouterCalldata128FromRoute(route, {
      minOutPerHop: [1n, 1n],
    });
    expect(cd.length).toBe(352);
    expect(cd[31]).toBe(0xe5);
  });

  it('encodeNativeDexMultihopRouterCalldata128FromRoute builds swap6 outer', () => {
    const v1 = venue(P1, TA, TB, 500_000n, 500_000n);
    const v2 = venue(P2, TB, TC, 500_000n, 500_000n);
    const v3 = venue(P3, TC, TD, 500_000n, 500_000n);
    const v4 = venue(P4, TD, TE, 500_000n, 500_000n);
    const v5 = venue(P5, TE, TF, 500_000n, 500_000n);
    const v6 = venue(P6, TF, TG, 500_000n, 500_000n);
    const route = findBestCpRoute([v1, v2, v3, v4, v5, v6], TA, TG, 50n, { maxHops: 6 })!;
    expect(route.hops.length).toBe(6);
    const zeros = Array.from({ length: 6 }, () => 0n);
    const cd = encodeNativeDexMultihopRouterCalldata128FromRoute(route, { minOutPerHop: zeros });
    expect(cd.length).toBe(992);
    expect(cd[31]).toBe(0xed);
  });

  it('encodeNativeDexMultihopRouterCalldata160FromRoute sends last hop output to final recipient', () => {
    const v1 = venue(P1, TA, TB, 100_000n, 100_000n);
    const v2 = venue(P2, TB, TC, 100_000n, 100_000n);
    const route = findBestCpRoute([v1, v2], TA, TC, 1000n, { maxHops: 2 })!;
    const cd = encodeNativeDexMultihopRouterCalldata160FromRoute(route, {
      minOutPerHop: [1n, 1n],
      routerAccountHex32: ROUTER,
      finalRecipientHex32: TRADER,
    });
    expect(cd.length).toBe(416);
    expect(cd[31]).toBe(0xe6);
    const hop0RecipientOffset = 64 + 128;
    const hop1RecipientOffset = 256 + 128;
    expect(cd.slice(hop0RecipientOffset, hop0RecipientOffset + 32)).toEqual(hexToBytes(ROUTER));
    expect(cd.slice(hop1RecipientOffset, hop1RecipientOffset + 32)).toEqual(hexToBytes(TRADER));
  });

  it('quoteCpEvenSplitAcrossDirectPools splits input', () => {
    const a = venue(P1, TA, TB, 100_000n, 100_000n);
    const b = venue(P2, TA, TB, 100_000n, 100_000n);
    const { allocations, totalOut } = quoteCpEvenSplitAcrossDirectPools([a, b], TA, TB, 100n, 2);
    expect(allocations.length).toBe(2);
    expect(allocations[0]!.amountIn + allocations[1]!.amountIn).toBe(100n);
    expect(totalOut).toBe(allocations[0]!.amountOut + allocations[1]!.amountOut);
  });

  it('hydrateCpPoolVenuesFromRpc dedupes pools and reads reserves + fee', async () => {
    const getContractStorage = vi.fn(async (_pool: string, k: string) => {
      if (k === NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX) return { value: wordU128(1000n) };
      if (k === NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX) return { value: wordU128(2000n) };
      if (k === NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX) return { value: wordU128(0n) };
      if (k === NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX) return { value: wordU128(0n) };
      throw new Error(`unexpected ${k}`);
    });
    const client = { getContractStorage } as unknown as BoingClient;
    const rows = [
      { poolHex: P1, tokenAHex: TA, tokenBHex: TB },
      { poolHex: P1, tokenAHex: TA, tokenBHex: TB },
    ];
    const venues = await hydrateCpPoolVenuesFromRpc(client, rows, { concurrency: 2 });
    expect(venues.length).toBe(1);
    expect(venues[0]!.reserveA).toBe(1000n);
    expect(venues[0]!.reserveB).toBe(2000n);
    expect(venues[0]!.feeBps).toBe(30n);
  });

  it('fetchCpRoutingFromDirectoryLogs end-to-end with mocks', async () => {
    const getNetworkInfo = vi.fn().mockResolvedValue({
      chain_id: 6913,
      chain_name: 't',
      head_height: 10,
      finalized_height: 10,
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
        as_of_height: 10,
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
          response_header_rpc_version: 'x',
          request_id_header: 'x',
          supports_jsonrpc_batch: true,
          jsonrpc_batch_max_env: '',
          websocket_max_connections_env: '',
          ready_min_peers_env: '',
        },
      },
      rpc: { not_available: [], not_available_note: '' },
      end_user: {
        chain_display_name: null,
        explorer_url: null,
        faucet_url: null,
        canonical_native_dex_factory: FACTORY,
      },
    } satisfies NetworkInfo);

    const getLogs = vi.fn().mockResolvedValue([
      {
        block_height: 1,
        tx_index: 0,
        tx_id: '0x' + 'cc'.repeat(32),
        log_index: 0,
        address: FACTORY,
        topics: [NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX, TA, TB],
        data: P1,
      },
    ]);

    const getContractStorage = vi.fn(async (contract: string, key: string) => {
      if (contract.toLowerCase() === FACTORY.toLowerCase() && key === NATIVE_DEX_FACTORY_COUNT_KEY_HEX) {
        return { value: `0x${'00'.repeat(24)}0000000000000001` };
      }
      if (contract.toLowerCase() === P1.toLowerCase()) {
        if (key === NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX) return { value: wordU128(100_000n) };
        if (key === NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX) return { value: wordU128(100_000n) };
        if (key === NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX) return { value: wordU128(0n) };
        if (key === NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX) return { value: wordU128(0n) };
      }
      throw new Error(`unexpected storage ${contract} ${key}`);
    });

    const client = { getNetworkInfo, getLogs, getContractStorage } as unknown as BoingClient;
    const { routes, venues, snapshot } = await fetchCpRoutingFromDirectoryLogs(client, TA, TB, 1000n, {
      registerLogs: { fromBlock: 0, toBlock: 10 },
      maxHops: 1,
    });
    expect(snapshot.pairsCount).toBe(1n);
    expect(venues.length).toBe(1);
    expect(routes.length).toBe(1);
    expect(routes[0]!.hops.length).toBe(1);
  });
});
