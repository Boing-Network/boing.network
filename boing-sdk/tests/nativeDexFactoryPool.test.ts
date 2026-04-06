import { describe, expect, it } from 'vitest';
import {
  NATIVE_DEX_FACTORY_COUNT_KEY_HEX,
  decodeNativeDexFactoryGetPairAtReturnData,
  decodeNativeDexFactoryPairsCountReturnData,
} from '../src/nativeDexFactoryPool.js';

describe('nativeDexFactoryPool', () => {
  it('count storage key is 32-byte word hex', () => {
    expect(NATIVE_DEX_FACTORY_COUNT_KEY_HEX.length).toBe(66);
  });

  it('decodes pairs_count return word', () => {
    const word =
      '0x' +
      '00'.repeat(30) +
      '0007'; // u128 low area; count = 7 in low byte
    expect(decodeNativeDexFactoryPairsCountReturnData(word)).toBe(7n);
  });

  it('decodes get_pair_at return triple', () => {
    const a = '11'.repeat(32);
    const b = '22'.repeat(32);
    const p = '33'.repeat(32);
    const r = decodeNativeDexFactoryGetPairAtReturnData(`0x${a}${b}${p}`);
    expect(r.tokenAHex.toLowerCase()).toBe(`0x${a}`);
    expect(r.tokenBHex.toLowerCase()).toBe(`0x${b}`);
    expect(r.poolHex.toLowerCase()).toBe(`0x${p}`);
  });
});
