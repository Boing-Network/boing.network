import { describe, expect, it } from 'vitest';
import {
  MICRO_FEE_EXECUTION_GAS_BPS,
  estimateExecutionGasFee,
  estimateMicroTransferFee,
  estimateTotalFeeWithTip,
  formatBoingFromBaseUnits,
  formatTipLadderForUi,
  suggestPriorityTipLadder,
} from '../src/fees.js';

describe('micro fee helpers', () => {
  it('estimates ~0.00021 BOING for a transfer at micro BPS', () => {
    const fee = estimateMicroTransferFee();
    expect(fee).toBe(estimateExecutionGasFee(21_000n, MICRO_FEE_EXECUTION_GAS_BPS));
    expect(formatBoingFromBaseUnits(fee)).toBe('0.00021');
  });

  it('builds a tip ladder with optional tips on top of the micro base', () => {
    const ladder = suggestPriorityTipLadder();
    expect(ladder.map((t) => t.label)).toEqual(['normal', 'faster', 'priority', 'urgent']);
    expect(ladder[0]!.tip).toBe(0n);
    const micro = estimateMicroTransferFee();
    expect(ladder[1]!.tip).toBe(micro);
    expect(ladder[2]!.tip).toBe(micro * 10n);
    expect(ladder[3]!.tip).toBe(micro * 100n);
    expect(estimateTotalFeeWithTip(21_000n, ladder[2]!.tip)).toBe(micro + ladder[2]!.tip);
  });

  it('formats UI ladder totals', () => {
    const ui = formatTipLadderForUi();
    expect(ui[0]!.tipBoing).toBe('0');
    expect(ui[0]!.totalBoing).toBe('0.00021');
    expect(ui[1]!.totalBoing).toBe('0.00042');
  });
});
