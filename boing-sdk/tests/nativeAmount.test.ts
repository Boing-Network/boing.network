import { describe, expect, it } from 'vitest';
import {
  BASE_UNITS_PER_BOING,
  boingToLedgerUnits,
  feeForGasBaseUnits,
  formatBoing,
  formatUnits,
  nativeDecimalsAt,
  parseBoing,
  parseUnits,
} from '../src/nativeAmount.js';

describe('native BOING amounts (18 decimals)', () => {
  it('formats base units', () => {
    expect(formatBoing(0n)).toBe('0');
    expect(formatBoing(BASE_UNITS_PER_BOING)).toBe('1');
    expect(formatBoing('1500000000000000000')).toBe('1.5');
    expect(formatBoing(1n)).toBe('0.000000000000000001');
    expect(formatBoing(21_000n * 1_000_000_000_000n)).toBe('0.021');
    expect(formatBoing('20021000000000000000', { maxFractionDigits: 2 })).toBe('20.02');
    expect(formatBoing(10n ** 30n)).toBe('1000000000000');
    expect(formatUnits('20', 0)).toBe('20');
  });

  it('parses BOING strings exactly', () => {
    expect(parseBoing('1.5')).toBe(1_500_000_000_000_000_000n);
    expect(parseBoing('.5')).toBe(BASE_UNITS_PER_BOING / 2n);
    expect(parseBoing('2.')).toBe(2n * BASE_UNITS_PER_BOING);
    expect(parseBoing('0.000000000000000001')).toBe(1n);
    expect(parseBoing('1_000')).toBe(1000n * BASE_UNITS_PER_BOING);
    expect(() => parseBoing('1.0000000000000000001')).toThrow(RangeError);
    expect(() => parseBoing('-1')).toThrow(SyntaxError);
    expect(() => parseBoing('1e18')).toThrow(SyntaxError);
    expect(() => parseBoing('.')).toThrow(SyntaxError);
    expect(() => parseBoing('')).toThrow(SyntaxError);
    expect(() => parseUnits('1.5', 0)).toThrow(RangeError);
    for (const s of ['1', '0.021', '123456789.123456789123456789']) {
      expect(formatBoing(parseBoing(s))).toBe(s);
    }
  });

  it('picks the ledger unit for a height', () => {
    const testnet = { symbol: 'BOING', decimals: 18, base_unit_decimals: 18, base_units_from_height: 1050 };
    expect(nativeDecimalsAt(testnet, 1049)).toBe(0);
    expect(nativeDecimalsAt(testnet, 1050)).toBe(18);
    expect(nativeDecimalsAt({ symbol: 'BOING', decimals: 18, base_units_from_height: null }, 0)).toBe(18);
    expect(nativeDecimalsAt({ symbol: 'BOING', decimals: 0 }, 5000)).toBe(0); // old node
    expect(nativeDecimalsAt(null, 5)).toBe(0);
    expect(boingToLedgerUnits('20', 0)).toBe(20n);
    expect(boingToLedgerUnits('20', 18)).toBe(20n * BASE_UNITS_PER_BOING);
  });

  it('fees are gas x 10^12 base units', () => {
    expect(feeForGasBaseUnits(21_000)).toBe(21n * BASE_UNITS_PER_BOING / 1000n);
  });
});
