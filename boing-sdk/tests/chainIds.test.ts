import { describe, expect, it } from 'vitest';
import {
  BOING_TESTNET_CHAIN_ID_DECIMAL,
  BOING_TESTNET_CHAIN_ID_HEX,
  isBoingTestnetChainId,
  normalizeBoingChainIdHex,
} from '../src/chainIds.js';

describe('chainIds', () => {
  it('constants match docs', () => {
    expect(BOING_TESTNET_CHAIN_ID_DECIMAL).toBe(6913);
    expect(BOING_TESTNET_CHAIN_ID_HEX).toBe('0x1b01');
  });

  it('normalizeBoingChainIdHex accepts decimal and hex', () => {
    expect(normalizeBoingChainIdHex(6913)).toBe('0x1b01');
    expect(normalizeBoingChainIdHex('6913')).toBe('0x1b01');
    expect(normalizeBoingChainIdHex('0x1b01')).toBe('0x1b01');
    expect(normalizeBoingChainIdHex('0x01B01')).toBe('0x1b01');
    expect(normalizeBoingChainIdHex(6913n)).toBe('0x1b01');
  });

  it('isBoingTestnetChainId', () => {
    expect(isBoingTestnetChainId(6913)).toBe(true);
    expect(isBoingTestnetChainId('0x1b01')).toBe(true);
    expect(isBoingTestnetChainId(1)).toBe(false);
    expect(isBoingTestnetChainId('0x1')).toBe(false);
  });
});
