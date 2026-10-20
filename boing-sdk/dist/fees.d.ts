/**
 * Native fee helpers for the low-base + optional priority-tip market.
 * Magnitudes match `boing_tokenomics` (`GAS_PRICE_BASE_UNITS`, `MICRO_FEE_EXECUTION_GAS_BPS`).
 */
/** Base units per gas at the full historical schedule (1e12 = 0.000001 BOING/gas). */
export declare const GAS_PRICE_BASE_UNITS = 1000000000000n;
/** Recommended micro share of the historical gas schedule (1%). */
export declare const MICRO_FEE_EXECUTION_GAS_BPS = 100;
/** Typical transfer gas (`GAS_PER_TRANSFER` / `GAS_UNITS_PER_BOING`). */
export declare const GAS_PER_TRANSFER = 21000n;
/**
 * Execution-gas fee in ledger base units:
 * `gasUsed * GAS_PRICE_BASE_UNITS * executionGasFeeBps / 10000`.
 */
export declare function estimateExecutionGasFee(gasUsed: bigint | number, executionGasFeeBps?: number): bigint;
/** Micro-fee for a simple transfer (~0.00021 BOING). */
export declare function estimateMicroTransferFee(): bigint;
/**
 * Suggested tip ladder in base units for wallets (untipped → light → medium → urgent).
 * Tips are optional; the base micro fee remains the spam deterrent.
 */
export declare function suggestPriorityTipLadder(): {
    label: string;
    tip: bigint;
}[];
/** Total cost = execution gas (at BPS) + optional tip (extras/blobs not included). */
export declare function estimateTotalFeeWithTip(gasUsed: bigint | number, priorityTip?: bigint, executionGasFeeBps?: number): bigint;
//# sourceMappingURL=fees.d.ts.map