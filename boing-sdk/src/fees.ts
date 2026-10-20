/**
 * Native fee helpers for the low-base + optional priority-tip market.
 * Magnitudes match `boing_tokenomics` (`GAS_PRICE_BASE_UNITS`, `MICRO_FEE_EXECUTION_GAS_BPS`).
 */

/** Base units per gas at the full historical schedule (1e12 = 0.000001 BOING/gas). */
export const GAS_PRICE_BASE_UNITS = 1_000_000_000_000n;

/** Recommended micro share of the historical gas schedule (1%). */
export const MICRO_FEE_EXECUTION_GAS_BPS = 100;

/** Typical transfer gas (`GAS_PER_TRANSFER` / `GAS_UNITS_PER_BOING`). */
export const GAS_PER_TRANSFER = 21_000n;

/**
 * Execution-gas fee in ledger base units:
 * `gasUsed * GAS_PRICE_BASE_UNITS * executionGasFeeBps / 10000`.
 */
export function estimateExecutionGasFee(
  gasUsed: bigint | number,
  executionGasFeeBps: number = MICRO_FEE_EXECUTION_GAS_BPS,
): bigint {
  const g = BigInt(gasUsed);
  const bps = BigInt(Math.max(0, Math.min(10_000, executionGasFeeBps)));
  return (g * GAS_PRICE_BASE_UNITS * bps) / 10_000n;
}

/** Micro-fee for a simple transfer (~0.00021 BOING). */
export function estimateMicroTransferFee(): bigint {
  return estimateExecutionGasFee(GAS_PER_TRANSFER, MICRO_FEE_EXECUTION_GAS_BPS);
}

/**
 * Suggested tip ladder in base units for wallets (untipped → light → medium → urgent).
 * Tips are optional; the base micro fee remains the spam deterrent.
 */
export function suggestPriorityTipLadder(): { label: string; tip: bigint }[] {
  const micro = estimateMicroTransferFee();
  return [
    { label: 'normal', tip: 0n },
    { label: 'faster', tip: micro },
    { label: 'priority', tip: micro * 10n },
    { label: 'urgent', tip: micro * 100n },
  ];
}

/** Total cost = execution gas (at BPS) + optional tip (extras/blobs not included). */
export function estimateTotalFeeWithTip(
  gasUsed: bigint | number,
  priorityTip: bigint = 0n,
  executionGasFeeBps: number = MICRO_FEE_EXECUTION_GAS_BPS,
): bigint {
  return estimateExecutionGasFee(gasUsed, executionGasFeeBps) + priorityTip;
}
