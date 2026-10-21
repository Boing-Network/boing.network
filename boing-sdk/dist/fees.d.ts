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
export type PriorityTipTier = {
    label: 'normal' | 'faster' | 'priority' | 'urgent';
    tip: bigint;
};
/**
 * Suggested tip ladder in base units for wallets (untipped → light → medium → urgent).
 * Tips are optional; the base micro fee remains the spam deterrent.
 */
export declare function suggestPriorityTipLadder(): PriorityTipTier[];
/** Total cost = execution gas (at BPS) + optional tip (extras/blobs not included). */
export declare function estimateTotalFeeWithTip(gasUsed: bigint | number, priorityTip?: bigint, executionGasFeeBps?: number): bigint;
/** Format ledger base units as a short BOING decimal string (18 decimals, trim trailing zeros). */
export declare function formatBoingFromBaseUnits(baseUnits: bigint, maxFrac?: number): string;
/**
 * Wallet-ready tip ladder: same tiers as [`suggestPriorityTipLadder`] plus display strings.
 * `baseFee` is the micro transfer fee; `totalWithTip` = baseFee + tip.
 */
export declare function formatTipLadderForUi(maxFrac?: number): {
    label: PriorityTipTier['label'];
    tip: bigint;
    tipBoing: string;
    baseFee: bigint;
    totalWithTip: bigint;
    totalBoing: string;
}[];
//# sourceMappingURL=fees.d.ts.map