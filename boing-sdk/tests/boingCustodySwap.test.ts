import { describe, expect, it, vi } from 'vitest';
import {
  BOING_CUSTODY_POOL_PROBE_CALLDATA_HEX,
  boingContractCallToExpressTx,
  discoverBoingCustodyPools,
  planBoingCustodySwap,
  probeBoingPoolCustody,
} from '../src/boingCustodySwap.js';
import { encodeBoingCustodyRouterSwapCalldata } from '../src/boingRouter.js';
import { CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX } from '../src/canonicalTestnetDex.js';
import { bytesToHex } from '../src/hex.js';

const A = '0x' + '0a'.repeat(32);
const B = '0x' + '0b'.repeat(32);
const POOL = '0x' + '11'.repeat(32);
const ROUTER = '0x' + '22'.repeat(32);
const ME = '0x' + '33'.repeat(32);
const state = { reserveA: 100_000n, reserveB: 100_000n, storedFeeBps: 30n, tokenAHex: A, tokenBHex: B };
const base = { poolHex: POOL, state, amountIn: 100n, slippageBps: 50n, senderHex: ME, routerHex: ROUTER, nonce: 5n };

describe('custody pool probe', () => {
  it('fault => custody, silent success => ledger-only, known v9 needs no RPC', async () => {
    const sim = vi.fn();
    sim.mockResolvedValueOnce({ success: false, error: 'Invalid jump destination' });
    expect(await probeBoingPoolCustody({ simulateContractCall: sim } as never, POOL)).toBe(true);
    expect(sim.mock.calls[0]![1]).toBe(BOING_CUSTODY_POOL_PROBE_CALLDATA_HEX);
    sim.mockResolvedValueOnce({ success: true, gas_used: 500 });
    expect(await probeBoingPoolCustody({ simulateContractCall: sim } as never, POOL)).toBe(false);
    const none = vi.fn();
    expect(await probeBoingPoolCustody({ simulateContractCall: none } as never, CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX)).toBe(true);
    expect(none).not.toHaveBeenCalled();
  });
  it('discovery keeps only custody pools and skips probe errors', async () => {
    const P2 = '0x' + '44'.repeat(32);
    const P3 = '0x' + '55'.repeat(32);
    const client = {
      listDexPoolsPage: vi.fn(async () => ({
        nextCursor: null,
        pools: [POOL, P2, P3].map((p) => ({ poolHex: p, tokenAHex: A, tokenBHex: B, reserveA: '10', reserveB: '20', feeBps: 30 })),
      })),
      simulateContractCall: vi.fn(async (p: string) => {
        if (p === POOL) return { success: false, error: 'fault' };
        if (p === P2) return { success: true };
        throw new Error('rpc down');
      }),
    };
    const pools = await discoverBoingCustodyPools(client as never);
    expect(pools.map((p) => p.poolHex)).toEqual([POOL]);
    expect(pools[0]!.reserveB).toBe(20n);
  });
});

describe('planBoingCustodySwap', () => {
  it('needs exact-amount approval, then routes with slippage-bounded minOut', () => {
    const p = planBoingCustodySwap({ ...base, tokenInHex: A, allowance: 0n });
    expect(p.hop.direction).toBe(0);
    expect(p.amountOut).toBe(98n); // 100 in, fee-on-input 30 bps (99 net), 100k:100k
    expect(p.minOut).toBe(97n);
    expect(p.approvalState).toBe('needs_approval');
    expect(p.approveTx!.nonce).toBe(5n);
    expect(p.swapTx.nonce).toBe(6n);
    const approve = boingContractCallToExpressTx(p.approveTx!);
    expect(approve.contract).toBe(A);
    expect(approve.calldata).toContain('22'.repeat(32)); // spender = router
    expect(approve.calldata.endsWith((100n).toString(16).padStart(32, '0'))).toBe(true); // exact amount
    const swap = boingContractCallToExpressTx(p.swapTx);
    expect(swap.contract).toBe(ROUTER);
    expect(swap.calldata).toBe(bytesToHex(encodeBoingCustodyRouterSwapCalldata([p.hop], 100n, 97n, ME)));
    for (const acct of [ME, ROUTER, POOL, A, B]) expect(swap.access_list.read).toContain(acct);
  });
  it('skips approval when the allowance covers it; B->A direction; rejects foreign tokens', () => {
    const p = planBoingCustodySwap({ ...base, tokenInHex: B, allowance: 100n });
    expect(p.approvalState).toBe('sufficient');
    expect(p.approveTx).toBeNull();
    expect(p.swapTx.nonce).toBe(5n);
    expect(p.hop).toMatchObject({ direction: 1, tokenInHex: B, tokenOutHex: A });
    expect(() => planBoingCustodySwap({ ...base, tokenInHex: ME, allowance: 0n })).toThrow(/not in this pool/);
    expect(() => planBoingCustodySwap({ ...base, tokenInHex: A, amountIn: 0n, allowance: 0n })).toThrow();
    expect(() => planBoingCustodySwap({ ...base, tokenInHex: A, slippageBps: 10_000n, allowance: 0n })).toThrow();
  });
});