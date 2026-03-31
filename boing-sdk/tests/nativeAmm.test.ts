import { describe, expect, it } from 'vitest';
import {
  SELECTOR_NATIVE_AMM_SWAP,
  encodeNativeAmmSwapCalldata,
  constantProductAmountOut,
} from '../src/nativeAmm.js';

describe('nativeAmm', () => {
  it('encodeNativeAmmSwapCalldata length and selector', () => {
    const c = encodeNativeAmmSwapCalldata(0n, 1_000_000n, 900_000n);
    expect(c.length).toBe(128);
    expect(c[31]).toBe(SELECTOR_NATIVE_AMM_SWAP);
  });

  it('constantProductAmountOut matches small pool', () => {
    expect(constantProductAmountOut(1000n, 2000n, 100n)).toBe(181n);
  });
});
