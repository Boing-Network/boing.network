import { describe, expect, it } from 'vitest';
import { isBoingNativeAccountIdHex, validateHex32 } from '../src/hex.js';

describe('hex', () => {
  const valid =
    '0xabababababababababababababababababababababababababababababababab';

  it('validateHex32 normalizes', () => {
    expect(validateHex32(valid)).toBe(valid);
    expect(validateHex32(valid.slice(2))).toBe(valid);
  });

  it('isBoingNativeAccountIdHex', () => {
    expect(isBoingNativeAccountIdHex(valid)).toBe(true);
    expect(isBoingNativeAccountIdHex('0x' + '00'.repeat(31))).toBe(false);
    expect(isBoingNativeAccountIdHex('0x' + 'ab'.repeat(20))).toBe(false);
  });
});
