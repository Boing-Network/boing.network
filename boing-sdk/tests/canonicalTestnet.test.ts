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

describe('canonicalTestnetDex USDC/BOING pair', () => {
  it('exports normalized ids for the live USDC / BOING tokens and v9 pool', async () => {
    const m = await import('../src/index.js');
    for (const id of [m.CANONICAL_BOING_TESTNET_USDC_TOKEN_HEX, m.CANONICAL_BOING_TESTNET_BOING_TOKEN_HEX, m.CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX]) {
      expect(validateHex32(id)).toBe(id);
    }
    expect(validateHex32(m.CANONICAL_BOING_TESTNET_CUSTODY_ROUTER_HEX)).toBe(m.CANONICAL_BOING_TESTNET_CUSTODY_ROUTER_HEX);
    expect(validateHex32(m.CANONICAL_BOING_TESTNET_LP_VAULT_V2_HEX)).toBe(m.CANONICAL_BOING_TESTNET_LP_VAULT_V2_HEX);
    expect(m.CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX).toBe('0x2ad75c2e11b1e93761f4f8df39502deecbe7148324e5a80a951798142b2c5b61');
  });
});
