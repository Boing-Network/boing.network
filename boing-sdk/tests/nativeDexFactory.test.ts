import { describe, expect, it } from 'vitest';
import {
  NATIVE_DEX_FACTORY_CREATE2_SALT_V1_HEX,
  NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX,
  encodeNativeDexGetPairAtCalldata,
  encodeNativeDexPairsCountCalldata,
  encodeNativeDexRegisterPairCalldata,
} from '../src/nativeDexFactory.js';
import { tryParseNativeDexFactoryRegisterLog3 } from '../src/nativeDexFactoryLogs.js';

describe('nativeDexFactory', () => {
  const z = '0x' + '00'.repeat(32);

  it('encodes calldata lengths', () => {
    expect(encodeNativeDexRegisterPairCalldata(z, z, z).length).toBe(128);
    expect(encodeNativeDexPairsCountCalldata().length).toBe(32);
    expect(encodeNativeDexGetPairAtCalldata(0).length).toBe(64);
  });

  it('topic0 and salt hex are 0x + 64 hex chars', () => {
    for (const h of [NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX, NATIVE_DEX_FACTORY_CREATE2_SALT_V1_HEX]) {
      expect(h.length).toBe(66);
      expect(h.startsWith('0x')).toBe(true);
    }
  });

  it('parses register Log3', () => {
    const ta = '0x' + '01'.repeat(32);
    const tb = '0x' + '02'.repeat(32);
    const pool = '0x' + '03'.repeat(32);
    const p = tryParseNativeDexFactoryRegisterLog3({
      topics: [NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX, ta, tb],
      data: pool,
    });
    expect(p).toEqual({
      tokenAHex: ta.toLowerCase(),
      tokenBHex: tb.toLowerCase(),
      poolHex: pool.toLowerCase(),
    });
  });
});
