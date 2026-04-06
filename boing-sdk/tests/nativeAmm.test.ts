import { describe, expect, it } from 'vitest';
import {
  SELECTOR_NATIVE_AMM_SWAP,
  SELECTOR_NATIVE_AMM_SWAP_TO,
  SELECTOR_NATIVE_AMM_ADD_LIQUIDITY,
  SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY,
  SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY_TO,
  SELECTOR_NATIVE_AMM_SET_SWAP_FEE_BPS,
  encodeNativeAmmSwapCalldata,
  encodeNativeAmmSwapToCalldata,
  encodeNativeAmmAddLiquidityCalldata,
  encodeNativeAmmRemoveLiquidityCalldata,
  encodeNativeAmmRemoveLiquidityToCalldata,
  encodeNativeAmmRemoveLiquidityCalldataHex,
  encodeNativeAmmSetSwapFeeBpsCalldata,
  constantProductAmountOut,
  constantProductAmountOutNoFee,
  constantProductAmountOutWithFeeBps,
  NATIVE_CP_SWAP_FEE_BPS,
  NATIVE_AMM_TOPIC_SWAP_HEX,
  NATIVE_AMM_TOPIC_ADD_LIQUIDITY_HEX,
  NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY_HEX,
  nativeAmmLogTopic0Utf8,
} from '../src/nativeAmm.js';
import { bytesToHex } from '../src/hex.js';

describe('nativeAmm', () => {
  it('encodeNativeAmmSwapCalldata length and selector', () => {
    const c = encodeNativeAmmSwapCalldata(0n, 1_000_000n, 900_000n);
    expect(c.length).toBe(128);
    expect(c[31]).toBe(SELECTOR_NATIVE_AMM_SWAP);
  });

  it('encodeNativeAmmSwapToCalldata is 160 bytes with selector 0x15 and recipient word', () => {
    const recip = '0x' + '42'.repeat(32);
    const c = encodeNativeAmmSwapToCalldata(0n, 1n, 1n, recip);
    expect(c.length).toBe(160);
    expect(c[31]).toBe(SELECTOR_NATIVE_AMM_SWAP_TO);
    expect(bytesToHex(c.slice(128, 160))).toBe(recip);
  });

  it('constantProductAmountOutNoFee matches raw CP step', () => {
    expect(constantProductAmountOutNoFee(1000n, 2000n, 100n)).toBe(181n);
  });

  it('constantProductAmountOut applies output fee (30 bps)', () => {
    expect(NATIVE_CP_SWAP_FEE_BPS).toBe(30);
    expect(constantProductAmountOut(1000n, 2000n, 100n)).toBe(180n);
    expect(constantProductAmountOutWithFeeBps(1000n, 2000n, 100n, 30n)).toBe(180n);
  });

  it('constantProductAmountOutWithFeeBps matches explicit bps (e.g. 100 = 1%)', () => {
    const dy = constantProductAmountOutNoFee(1000n, 2000n, 100n);
    expect(dy).toBe(181n);
    expect(constantProductAmountOutWithFeeBps(1000n, 2000n, 100n, 100n)).toBe((dy * 9900n) / 10000n);
  });

  it('encodeNativeAmmSetSwapFeeBpsCalldata is 64 bytes with selector 0x14', () => {
    const c = encodeNativeAmmSetSwapFeeBpsCalldata(100n);
    expect(c.length).toBe(64);
    expect(c[31]).toBe(SELECTOR_NATIVE_AMM_SET_SWAP_FEE_BPS);
  });

  it('encodeNativeAmmSetSwapFeeBpsCalldata rejects fee outside 1..10000', () => {
    expect(() => encodeNativeAmmSetSwapFeeBpsCalldata(0n)).toThrow(RangeError);
    expect(() => encodeNativeAmmSetSwapFeeBpsCalldata(10001n)).toThrow(RangeError);
  });

  it('native AMM Log2 topic0 hex (32-byte words)', () => {
    for (const h of [NATIVE_AMM_TOPIC_SWAP_HEX, NATIVE_AMM_TOPIC_ADD_LIQUIDITY_HEX, NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY_HEX]) {
      expect(h.length).toBe(66);
    }
    const swapT = nativeAmmLogTopic0Utf8('BOING_NATIVEAMM_SWAP_V1');
    expect(swapT.length).toBe(32);
    expect(swapT[23]).toBe(0);
    expect(swapT[31]).toBe(0);
  });

  it('encodeNativeAmmAddLiquidityCalldata length and selector', () => {
    const c = encodeNativeAmmAddLiquidityCalldata(10n, 20n, 0n);
    expect(c.length).toBe(128);
    expect(c[31]).toBe(SELECTOR_NATIVE_AMM_ADD_LIQUIDITY);
  });

  it('encodeNativeAmmRemoveLiquidityCalldata length and selector', () => {
    const c = encodeNativeAmmRemoveLiquidityCalldata(1n, 0n, 0n);
    expect(c.length).toBe(128);
    expect(c[31]).toBe(SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY);
  });

  it('encodeNativeAmmRemoveLiquidityCalldataHex matches bytes encoder', () => {
    const c = encodeNativeAmmRemoveLiquidityCalldata(5n, 1n, 2n);
    expect(encodeNativeAmmRemoveLiquidityCalldataHex(5n, 1n, 2n)).toBe(bytesToHex(c));
  });

  it('encodeNativeAmmRemoveLiquidityToCalldata is 192 bytes with selector 0x16', () => {
    const a = '0x' + '11'.repeat(32);
    const b = '0x' + '22'.repeat(32);
    const c = encodeNativeAmmRemoveLiquidityToCalldata(500n, 0n, 0n, a, b);
    expect(c.length).toBe(192);
    expect(c[31]).toBe(SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY_TO);
    expect(bytesToHex(c.slice(128, 160))).toBe(a);
    expect(bytesToHex(c.slice(160, 192))).toBe(b);
  });
});
