/**
 * Native BOING amounts (18 decimals: 1 BOING = 10^18 base units). See `docs/NATIVE-DENOMINATION.md`.
 *
 * The node returns native amounts as decimal strings in **ledger units**: base units on chains without
 * history (and mainnet) and on testnet 6913 from height 1050; whole BOING on testnet before it. Use
 * {@link nativeDecimalsAt} with `boing_getNetworkInfo.native_currency` to pick the unit for a height.
 * All math is `bigint`; never route amounts through `number` (base-unit values exceed 2^53).
 */
export declare const BOING_DECIMALS = 18;
export declare const BASE_UNITS_PER_BOING: bigint;
/** Fee per gas in base units from the 18-decimals switch (0.000001 BOING). */
export declare const GAS_PRICE_BASE_UNITS = 1000000000000n;
/** `boing_getNetworkInfo.native_currency` / `boing_health.native_currency` (newer fields optional). */
export interface NativeCurrencyInfo {
    symbol: string;
    /** Unit of the amounts the node returns now: 0 (whole BOING) or 18 (base units). */
    decimals: number;
    base_unit_decimals?: number;
    base_units_per_boing?: string;
    /** First height in base units; `null` = base units from genesis. Absent on nodes before testnet-v0.1.17. */
    base_units_from_height?: number | null;
    gas_price_base_units?: string;
}
/**
 * Format an integer amount with `decimals` fractional digits, trimming trailing zeros
 * (`formatUnits(1500000000000000000n, 18)` → `"1.5"`). `maxFractionDigits` truncates (never rounds up).
 */
export declare function formatUnits(amount: bigint | string | number, decimals: number, opts?: {
    maxFractionDigits?: number;
}): string;
/**
 * Parse a decimal string into integer units with `decimals` fractional digits (`parseUnits("0.021", 18)`).
 * Throws on more fractional digits than `decimals`, signs, exponents, or empty input.
 */
export declare function parseUnits(text: string, decimals: number): bigint;
/** Base units → BOING string (`"20.021"`). */
export declare function formatBoing(baseUnits: bigint | string | number, opts?: {
    maxFractionDigits?: number;
}): string;
/** BOING string → base units (`parseBoing("1.5")` → `1500000000000000000n`). */
export declare function parseBoing(text: string): bigint;
/**
 * Ledger decimals (0 or 18) for amounts committed at `height`, from `native_currency`. Nodes before
 * testnet-v0.1.17 (no `base_units_from_height` field) report `decimals` only.
 */
export declare function nativeDecimalsAt(info: NativeCurrencyInfo | null | undefined, height: number): number;
/** Format a ledger amount (decimal string) given its ledger decimals (0 = whole BOING). */
export declare function formatNativeAmount(raw: bigint | string | number, ledgerDecimals: number, opts?: {
    maxFractionDigits?: number;
}): string;
/** User-entered BOING → ledger units for a tx (`ledgerDecimals` 18 → base units; 0 → whole BOING, integers only). */
export declare function boingToLedgerUnits(text: string, ledgerDecimals: number): bigint;
/** Network fee for `gasUsed` in base units (18-decimals rule; excludes governance extras). */
export declare function feeForGasBaseUnits(gasUsed: number | bigint): bigint;
//# sourceMappingURL=nativeAmount.d.ts.map