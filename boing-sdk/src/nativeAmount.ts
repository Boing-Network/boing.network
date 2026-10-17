/**
 * Native BOING amounts (18 decimals: 1 BOING = 10^18 base units). See `docs/NATIVE-DENOMINATION.md`.
 *
 * The node returns native amounts as decimal strings in **ledger units**: base units on chains without
 * history (and mainnet) and on testnet 6913 from height 1050; whole BOING on testnet before it. Use
 * {@link nativeDecimalsAt} with `boing_getNetworkInfo.native_currency` to pick the unit for a height.
 * All math is `bigint`; never route amounts through `number` (base-unit values exceed 2^53).
 */

export const BOING_DECIMALS = 18;
export const BASE_UNITS_PER_BOING = 10n ** 18n;
/** Fee per gas in base units from the 18-decimals switch (0.000001 BOING). */
export const GAS_PRICE_BASE_UNITS = 1_000_000_000_000n;

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

function toBigInt(v: bigint | string | number): bigint {
  if (typeof v === 'bigint') return v;
  if (typeof v === 'number') {
    if (!Number.isSafeInteger(v)) throw new RangeError(`unsafe integer amount: ${v}`);
    return BigInt(v);
  }
  const s = v.trim();
  if (!/^-?\d+$/.test(s)) throw new SyntaxError(`not an integer amount: ${JSON.stringify(v)}`);
  return BigInt(s);
}

/**
 * Format an integer amount with `decimals` fractional digits, trimming trailing zeros
 * (`formatUnits(1500000000000000000n, 18)` → `"1.5"`). `maxFractionDigits` truncates (never rounds up).
 */
export function formatUnits(
  amount: bigint | string | number,
  decimals: number,
  opts: { maxFractionDigits?: number } = {},
): string {
  const v = toBigInt(amount);
  const neg = v < 0n;
  const abs = neg ? -v : v;
  if (decimals <= 0) return (neg ? '-' : '') + abs.toString();
  const base = 10n ** BigInt(decimals);
  const whole = abs / base;
  let frac = (abs % base).toString().padStart(decimals, '0');
  if (opts.maxFractionDigits !== undefined) frac = frac.slice(0, Math.max(0, opts.maxFractionDigits));
  frac = frac.replace(/0+$/, '');
  return (neg ? '-' : '') + whole.toString() + (frac ? `.${frac}` : '');
}

/**
 * Parse a decimal string into integer units with `decimals` fractional digits (`parseUnits("0.021", 18)`).
 * Throws on more fractional digits than `decimals`, signs, exponents, or empty input.
 */
export function parseUnits(text: string, decimals: number): bigint {
  const s = text.trim().replace(/_/g, '');
  if (s === '') throw new SyntaxError('empty amount');
  const m = /^(\d*)(?:\.(\d*))?$/.exec(s);
  if (!m || (m[1] === '' && (m[2] === undefined || m[2] === ''))) {
    throw new SyntaxError(`invalid amount: ${JSON.stringify(text)}`);
  }
  const whole = m[1] || '0';
  const frac = m[2] ?? '';
  if (frac.length > decimals) {
    throw new RangeError(`too many decimal places (max ${decimals}): ${JSON.stringify(text)}`);
  }
  return BigInt(whole) * 10n ** BigInt(decimals) + BigInt(frac.padEnd(decimals, '0') || '0');
}

/** Base units → BOING string (`"20.021"`). */
export function formatBoing(baseUnits: bigint | string | number, opts: { maxFractionDigits?: number } = {}): string {
  return formatUnits(baseUnits, BOING_DECIMALS, opts);
}

/** BOING string → base units (`parseBoing("1.5")` → `1500000000000000000n`). */
export function parseBoing(text: string): bigint {
  return parseUnits(text, BOING_DECIMALS);
}

/**
 * Ledger decimals (0 or 18) for amounts committed at `height`, from `native_currency`. Nodes before
 * testnet-v0.1.17 (no `base_units_from_height` field) report `decimals` only.
 */
export function nativeDecimalsAt(info: NativeCurrencyInfo | null | undefined, height: number): number {
  if (!info) return 0;
  if (info.base_units_from_height === undefined) return info.decimals ?? 0;
  if (info.base_units_from_height === null) return info.base_unit_decimals ?? BOING_DECIMALS;
  return height >= info.base_units_from_height ? (info.base_unit_decimals ?? BOING_DECIMALS) : 0;
}

/** Format a ledger amount (decimal string) given its ledger decimals (0 = whole BOING). */
export function formatNativeAmount(
  raw: bigint | string | number,
  ledgerDecimals: number,
  opts: { maxFractionDigits?: number } = {},
): string {
  return formatUnits(raw, ledgerDecimals, opts);
}

/** User-entered BOING → ledger units for a tx (`ledgerDecimals` 18 → base units; 0 → whole BOING, integers only). */
export function boingToLedgerUnits(text: string, ledgerDecimals: number): bigint {
  return parseUnits(text, ledgerDecimals);
}

/** Network fee for `gasUsed` in base units (18-decimals rule; excludes governance extras). */
export function feeForGasBaseUnits(gasUsed: number | bigint): bigint {
  return BigInt(gasUsed) * GAS_PRICE_BASE_UNITS;
}
