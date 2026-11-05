import { describe, expect, it } from 'vitest';
import {
  SELECTOR_NATIVE_DEX_CREATE_AND_SEED,
  encodeNativeDexCreateAndSeedCalldata,
  predictCreateAndSeedPoolAddress,
  buildCreateAndSeedBatchCalls,
  NATIVE_DEX_CREATE_AND_SEED_BYTECODE_HEX,
  predictNativeDexCreateAndSeedFactoryCreate2Address,
} from '../src/nativeDexCreateAndSeed.js';
import { boingPoolBytecodeHex } from '../src/boingPool.js';
import { hexToBytes } from '../src/hex.js';

const A = '0x' + 'a1'.repeat(32);
const B = '0x' + 'b1'.repeat(32);
const F = '0x' + 'f1'.repeat(32);
const D = '0x' + 'd1'.repeat(32);
const S = '0x' + '07'.repeat(32);
const SENDER = '0x' + '11'.repeat(32);

describe('nativeDexCreateAndSeed', () => {
  it('encodes header + bytecode', () => {
    const pool = hexToBytes(boingPoolBytecodeHex(8));
    const cd = encodeNativeDexCreateAndSeedCalldata({
      saltHex32: S,
      tokenAHex32: A,
      tokenBHex32: B,
      directoryHex32: D,
      poolBytecode: pool,
    });
    expect(cd[31]).toBe(SELECTOR_NATIVE_DEX_CREATE_AND_SEED);
    expect(cd.length).toBe(192 + pool.length);
  });

  it('builds one-shot batch with predicted pool', () => {
    const pool = hexToBytes(boingPoolBytecodeHex(8));
    const { poolHex, calls } = buildCreateAndSeedBatchCalls({
      senderHex: SENDER,
      factoryHex: F,
      saltHex32: S,
      tokenAHex: A,
      tokenBHex: B,
      amountA: 1000n,
      amountB: 2000n,
      directoryHex: D,
      poolBytecode: pool,
    });
    expect(poolHex).toBe(predictCreateAndSeedPoolAddress(F, S, pool).toLowerCase());
    expect(calls).toHaveLength(4);
    expect(NATIVE_DEX_CREATE_AND_SEED_BYTECODE_HEX.startsWith('0x')).toBe(true);
    expect(predictNativeDexCreateAndSeedFactoryCreate2Address(SENDER)).toMatch(/^0x[0-9a-f]{64}$/);
  });
});
