/**
 * Native constant-product pool calldata (Boing VM). See `docs/NATIVE-AMM-CALLDATA.md`.
 * Reserves and amounts should stay ≤ `Number.MAX_SAFE_INTEGER` / u64 range for VM `Mul` parity.
 */

import { bytesToHex } from './hex.js';

/** `swap` selector (low byte of word0). */
export const SELECTOR_NATIVE_AMM_SWAP = 0x10;
/** `add_liquidity` selector. */
export const SELECTOR_NATIVE_AMM_ADD_LIQUIDITY = 0x11;
/** `remove_liquidity` selector (MVP pool: no-op). */
export const SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY = 0x12;

function selectorWord(selector: number): Uint8Array {
  const w = new Uint8Array(32);
  w[31] = selector & 0xff;
  return w;
}

function amountWord(amount: bigint): Uint8Array {
  const w = new Uint8Array(32);
  if (amount < 0n || amount > (1n << 128n) - 1n) {
    throw new RangeError('amount must fit in u128');
  }
  const be = new Uint8Array(16);
  let x = amount;
  for (let i = 15; i >= 0; i--) {
    be[i] = Number(x & 0xffn);
    x >>= 8n;
  }
  w.set(be, 16);
  return w;
}

/** 128-byte `swap` calldata: direction 0 = A→B, 1 = B→A. */
export function encodeNativeAmmSwapCalldata(direction: bigint, amountIn: bigint, minOut: bigint): Uint8Array {
  const out = new Uint8Array(128);
  out.set(selectorWord(SELECTOR_NATIVE_AMM_SWAP), 0);
  out.set(amountWord(direction), 32);
  out.set(amountWord(amountIn), 64);
  out.set(amountWord(minOut), 96);
  return out;
}

/** 128-byte `add_liquidity` calldata. */
export function encodeNativeAmmAddLiquidityCalldata(
  amountA: bigint,
  amountB: bigint,
  minLiquidity: bigint
): Uint8Array {
  const out = new Uint8Array(128);
  out.set(selectorWord(SELECTOR_NATIVE_AMM_ADD_LIQUIDITY), 0);
  out.set(amountWord(amountA), 32);
  out.set(amountWord(amountB), 64);
  out.set(amountWord(minLiquidity), 96);
  return out;
}

/** 128-byte `remove_liquidity` calldata. */
export function encodeNativeAmmRemoveLiquidityCalldata(
  liquidityBurn: bigint,
  minA: bigint,
  minB: bigint
): Uint8Array {
  const out = new Uint8Array(128);
  out.set(selectorWord(SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY), 0);
  out.set(amountWord(liquidityBurn), 32);
  out.set(amountWord(minA), 64);
  out.set(amountWord(minB), 96);
  return out;
}

export function encodeNativeAmmSwapCalldataHex(direction: bigint, amountIn: bigint, minOut: bigint): string {
  return bytesToHex(encodeNativeAmmSwapCalldata(direction, amountIn, minOut));
}

export function encodeNativeAmmAddLiquidityCalldataHex(
  amountA: bigint,
  amountB: bigint,
  minLiquidity: bigint = 0n
): string {
  return bytesToHex(encodeNativeAmmAddLiquidityCalldata(amountA, amountB, minLiquidity));
}

/** No-fee constant product: Δout = ⌊ r_out · Δin / (r_in + Δin) ⌋ (u64). */
export function constantProductAmountOut(reserveIn: bigint, reserveOut: bigint, amountIn: bigint): bigint {
  if (reserveIn < 0n || reserveOut < 0n || amountIn < 0n) {
    throw new RangeError('reserves and amountIn must be non-negative');
  }
  const denom = reserveIn + amountIn;
  if (denom === 0n) return 0n;
  return (reserveOut * amountIn) / denom;
}
