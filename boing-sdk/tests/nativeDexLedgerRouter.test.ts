import { describe, expect, it } from 'vitest';
import { encodeNativeAmmSwapCalldata } from '../src/nativeAmm.js';
import { encodeNativeDexLedgerRouterForwardCalldata } from '../src/nativeDexLedgerRouter.js';

describe('nativeDexLedgerRouter', () => {
  const pool = '0x' + 'aa'.repeat(32);

  it('encodes 192-byte forward calldata', () => {
    const inner = encodeNativeAmmSwapCalldata(0n, 100n, 1n);
    expect(inner.length).toBe(128);
    const out = encodeNativeDexLedgerRouterForwardCalldata(pool, inner);
    expect(out.length).toBe(192);
    expect(out[31]).toBe(0xe0);
  });
});
