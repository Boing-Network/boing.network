import { describe, expect, it } from 'vitest';
import {
  BOING_TESTNET_NATIVE_VALUE_FROM,
  NATIVE_CP_POOL_BYTECODE_HEX,
  WRAPPED_BOING_BYTECODE_HEX,
  boingTxToExpressRequest,
  buildBatchTransaction,
  buildContractCallWithValueTransaction,
  buildUnwrapBoingTransaction,
  buildWrapBoingTransaction,
  bytesToHex,
  encodeTransactionPayload,
  nativeValueActiveAt,
  planBoingNativeSwap,
  quoteBoingPoolAmountOut,
} from '../src/index.js';

const le = (v: bigint, n: number) => {
  let s = '';
  for (let i = 0; i < n; i++) s += Number((v >> BigInt(8 * i)) & 0xffn).toString(16).padStart(2, '0');
  return s;
};
const ONE = 10n ** 18n;
const C = '0x' + '11'.repeat(32);
const S = '0x' + 'd6'.repeat(32);
const W = '0x' + 'ee'.repeat(32);
const USDC = '0x' + '66'.repeat(32);
const POOL = '0x' + 'aa'.repeat(32);

describe('native value payloads', () => {
  it('ContractCallWithValue encodes as variant 20 (same bytes as the Rust layout test)', () => {
    const tx = buildContractCallWithValueTransaction({ nonce: 0n, senderHex: S, contractHex: C, calldata: Uint8Array.of(0xaa, 0xbb), value: 5n * ONE });
    expect(bytesToHex(encodeTransactionPayload(tx.payload))).toBe('0x' + le(20n, 4) + '11'.repeat(32) + le(2n, 8) + 'aabb' + le(5n * ONE, 16));
    expect(tx.accessList.write).toHaveLength(2);
  });

  it('batch value step encodes as BatchCall 2', () => {
    const tx = buildBatchTransaction({
      nonce: 0n,
      senderHex: S,
      calls: [{ kind: 'contractCallWithValue', contractHex: C, calldata: Uint8Array.of(0xaa, 0xbb), value: 5n * ONE }],
    });
    expect(bytesToHex(encodeTransactionPayload(tx.payload))).toBe(
      '0x' + le(9n, 4) + le(1n, 8) + le(2n, 4) + '11'.repeat(32) + le(2n, 8) + 'aabb' + le(5n * ONE, 16),
    );
  });

  it('rejects values outside u128 and batch totals that overflow', () => {
    expect(() =>
      encodeTransactionPayload({ kind: 'contractCallWithValue', contract: new Uint8Array(32), calldata: new Uint8Array(), value: 1n << 128n }),
    ).toThrow();
    const big = (1n << 127n) + 1n;
    expect(() =>
      buildBatchTransaction({
        nonce: 0n,
        senderHex: S,
        calls: [
          { kind: 'contractCallWithValue', contractHex: C, calldata: new Uint8Array(), value: big },
          { kind: 'transfer', toHex: C, amount: big },
        ],
      }),
    ).toThrow(/overflows/);
  });

  it('activation height', () => {
    expect(BOING_TESTNET_NATIVE_VALUE_FROM).toBe(1150);
    expect(nativeValueActiveAt(6913, 1149)).toBe(false);
    expect(nativeValueActiveAt(6913, 1150)).toBe(true);
    expect(nativeValueActiveAt(31337, 0)).toBe(true);
  });

  it('ships the wBOING and v10 pool bytecode', () => {
    expect(WRAPPED_BOING_BYTECODE_HEX.startsWith('0x')).toBe(true);
    expect(NATIVE_CP_POOL_BYTECODE_HEX[10]).toMatch(/^0x[0-9a-f]+$/);
    expect(NATIVE_CP_POOL_BYTECODE_HEX[10]).not.toBe(NATIVE_CP_POOL_BYTECODE_HEX[9]);
  });
});

describe('wBOING + native swaps', () => {
  it('wrap / unwrap builders', () => {
    const w = buildWrapBoingTransaction({ nonce: 3n, senderHex: S, wboingHex: W, amount: ONE });
    expect(w.payload.kind).toBe('contractCallWithValue');
    const u = buildUnwrapBoingTransaction({ nonce: 4n, senderHex: S, wboingHex: W, amount: 'all' });
    expect(u.payload.kind).toBe('contractCall');
    if (u.payload.kind === 'contractCall') expect(u.payload.calldata[31]).toBe(0x23);
  });

  it('v10 quotes 18-decimal amounts above u64 (v9 refuses)', () => {
    const q = quoteBoingPoolAmountOut(10, 1000n * ONE, 100_000n * ONE, ONE, 30n);
    expect(q).toBe(99_600_698_103_990_321_649n);
    expect(() => quoteBoingPoolAmountOut(9, 1000n * ONE, 100_000n * ONE, ONE, 30n)).toThrow();
  });

  const state = { reserveA: 10n * ONE, reserveB: 1000n, storedFeeBps: 30n, tokenAHex: W, tokenBHex: USDC };

  it('BOING -> USDC: one batch that wraps, approves and swaps, carrying the value', () => {
    const p = planBoingNativeSwap({ poolHex: POOL, wboingHex: W, state, assetIn: 'native', assetOut: USDC, amountIn: ONE, slippageBps: 100n, senderHex: S, nonce: 7n });
    expect(p.mode).toBe('wrap');
    expect(p.direction).toBe(0);
    expect(p.value).toBe(ONE);
    expect(p.amountOut).toBe(90n); // floor(1000 * 0.997e18 / (10e18 + 0.997e18))
    expect(p.tx.payload.kind).toBe('batch');
    if (p.tx.payload.kind === 'batch') {
      expect(p.tx.payload.calls.map((c) => c.kind)).toEqual(['contractCallWithValue', 'contractCall', 'contractCall']);
    }
    const r = boingTxToExpressRequest(p.tx);
    expect(r.type).toBe('batch');
    if (r.type === 'batch') {
      expect(r.calls[0]).toMatchObject({ type: 'contract_call', value: ONE.toString() });
      expect('value' in r.calls[1]).toBe(false);
    }
  });

  it('USDC -> BOING: approve, swap, withdraw_all', () => {
    const p = planBoingNativeSwap({ poolHex: POOL, wboingHex: W, state, assetIn: USDC, assetOut: 'native', amountIn: 90n, slippageBps: 100n, senderHex: S, nonce: 8n });
    expect(p.mode).toBe('unwrap');
    expect(p.direction).toBe(1);
    expect(p.value).toBe(0n);
    expect(p.steps).toEqual(['Approve pool', 'Swap', 'Unwrap to BOING']);
    expect(p.amountOut > 0n).toBe(true);
  });

  it('rejects pairs the pool does not hold and token-to-token plans', () => {
    expect(() => planBoingNativeSwap({ poolHex: POOL, wboingHex: W, state, assetIn: 'native', assetOut: C, amountIn: ONE, slippageBps: 0n, senderHex: S, nonce: 0n })).toThrow();
    expect(() => planBoingNativeSwap({ poolHex: POOL, wboingHex: W, state, assetIn: USDC, assetOut: W, amountIn: 1n, slippageBps: 0n, senderHex: S, nonce: 0n })).toThrow();
  });
});
