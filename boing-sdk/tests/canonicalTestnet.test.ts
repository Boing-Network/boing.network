import { describe, expect, it } from 'vitest';
import { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from '../src/canonicalTestnet.js';
import { validateHex32 } from '../src/hex.js';

describe('canonicalTestnet', () => {
  it('exports normalized 32-byte pool id matching docs / boing.finance', () => {
    expect(CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX).toBe(
      '0x5b60d6f8514d1a8a40340fa354951d778b0821cd3ff7e534d0bea50f67d38976',
    );
    expect(validateHex32(CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX)).toBe(
      CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX,
    );
  });
});
