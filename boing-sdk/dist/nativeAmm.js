/**
 * Native constant-product pool calldata (Boing VM). See `docs/NATIVE-AMM-CALLDATA.md`.
 * Reserves and amounts should stay ≤ `Number.MAX_SAFE_INTEGER` / u64 range for VM `Mul` parity.
 */
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
/** `swap` selector (low byte of word0). */
export const SELECTOR_NATIVE_AMM_SWAP = 0x10;
/** `add_liquidity` selector. */
export const SELECTOR_NATIVE_AMM_ADD_LIQUIDITY = 0x11;
/** `remove_liquidity` selector (LP burn + pro-rata withdrawal). */
export const SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY = 0x12;
/** **v2 pool:** one-time `set_tokens(token_a, token_b)`. */
export const SELECTOR_NATIVE_AMM_SET_TOKENS = 0x13;
/** **v3/v4 pool:** `set_swap_fee_bps(fee)` — **64-byte** calldata; only when **total LP == 0**; **`1 ≤ fee ≤ 10_000`**. */
export const SELECTOR_NATIVE_AMM_SET_SWAP_FEE_BPS = 0x14;
/** **v5 pool:** `swap_to` — like `swap` plus **word4** = output recipient (**160-byte** calldata). */
export const SELECTOR_NATIVE_AMM_SWAP_TO = 0x15;
/** **v5 pool:** `remove_liquidity_to` — like `remove_liquidity` plus **recipient_a** / **recipient_b** (**192-byte** calldata). */
export const SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY_TO = 0x16;
/** Swap fee in basis points on **output** (matches `native_amm::NATIVE_CP_SWAP_FEE_BPS`). */
export const NATIVE_CP_SWAP_FEE_BPS = 30;
/** Build 32-byte `Log2` **topic0** (UTF-8 ASCII + zero pad), matching `native_amm` constants. */
export function nativeAmmLogTopic0Utf8(ascii) {
    const u8 = new Uint8Array(32);
    const enc = new TextEncoder().encode(ascii);
    if (enc.length > 32) {
        throw new RangeError('native AMM topic0 label too long');
    }
    u8.set(enc);
    return u8;
}
/** `Log2` topic0 hex for a successful **`swap`** (see `NATIVE-AMM-CALLDATA.md` § Logs). */
export const NATIVE_AMM_TOPIC_SWAP_HEX = bytesToHex(nativeAmmLogTopic0Utf8('BOING_NATIVEAMM_SWAP_V1'));
/** `Log2` topic0 hex for **`add_liquidity`**. */
export const NATIVE_AMM_TOPIC_ADD_LIQUIDITY_HEX = bytesToHex(nativeAmmLogTopic0Utf8('BOING_NATIVEAMM_ADDLP_V1'));
/** `Log2` topic0 hex for **`remove_liquidity`**. */
export const NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY_HEX = bytesToHex(nativeAmmLogTopic0Utf8('BOING_NATIVEAMM_RMLP_V1'));
function selectorWord(selector) {
    const w = new Uint8Array(32);
    w[31] = selector & 0xff;
    return w;
}
function amountWord(amount) {
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
export function encodeNativeAmmSwapCalldata(direction, amountIn, minOut) {
    const out = new Uint8Array(128);
    out.set(selectorWord(SELECTOR_NATIVE_AMM_SWAP), 0);
    out.set(amountWord(direction), 32);
    out.set(amountWord(amountIn), 64);
    out.set(amountWord(minOut), 96);
    return out;
}
/** **160-byte** `swap_to` calldata (v5 pool): explicit **output recipient** for token `transfer`. */
export function encodeNativeAmmSwapToCalldata(direction, amountIn, minOut, recipientHex32) {
    const out = new Uint8Array(160);
    out.set(selectorWord(SELECTOR_NATIVE_AMM_SWAP_TO), 0);
    out.set(amountWord(direction), 32);
    out.set(amountWord(amountIn), 64);
    out.set(amountWord(minOut), 96);
    out.set(hexToBytes(validateHex32(recipientHex32)), 128);
    return out;
}
/**
 * 128-byte `add_liquidity` calldata. On success the pool **returns** **32** bytes: LP minted in this
 * call as **u128** in the **low 16 bytes** of the word (big-endian word; value in bytes 16..32).
 */
export function encodeNativeAmmAddLiquidityCalldata(amountA, amountB, minLiquidity) {
    const out = new Uint8Array(128);
    out.set(selectorWord(SELECTOR_NATIVE_AMM_ADD_LIQUIDITY), 0);
    out.set(amountWord(amountA), 32);
    out.set(amountWord(amountB), 64);
    out.set(amountWord(minLiquidity), 96);
    return out;
}
/** 128-byte `remove_liquidity` calldata. */
export function encodeNativeAmmRemoveLiquidityCalldata(liquidityBurn, minA, minB) {
    const out = new Uint8Array(128);
    out.set(selectorWord(SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY), 0);
    out.set(amountWord(liquidityBurn), 32);
    out.set(amountWord(minA), 64);
    out.set(amountWord(minB), 96);
    return out;
}
/** **192-byte** `remove_liquidity_to` (v5 pool): explicit **recipient_a** / **recipient_b** for reference-token `transfer`. */
export function encodeNativeAmmRemoveLiquidityToCalldata(liquidityBurn, minA, minB, recipientAHex32, recipientBHex32) {
    const out = new Uint8Array(192);
    out.set(selectorWord(SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY_TO), 0);
    out.set(amountWord(liquidityBurn), 32);
    out.set(amountWord(minA), 64);
    out.set(amountWord(minB), 96);
    out.set(hexToBytes(validateHex32(recipientAHex32)), 128);
    out.set(hexToBytes(validateHex32(recipientBHex32)), 160);
    return out;
}
/** **v2:** 96-byte `set_tokens` — each id is 32-byte account hex (`0x` + 64 hex). Use `0x` + 64 zeros for “no token” on that side. */
export function encodeNativeAmmSetTokensCalldata(tokenAHex32, tokenBHex32) {
    const out = new Uint8Array(96);
    out.set(selectorWord(SELECTOR_NATIVE_AMM_SET_TOKENS), 0);
    out.set(hexToBytes(validateHex32(tokenAHex32)), 32);
    out.set(hexToBytes(validateHex32(tokenBHex32)), 64);
    return out;
}
/** **v3/v4:** 64-byte `set_swap_fee_bps` calldata (`native_amm::encode_set_swap_fee_bps_calldata`). */
export function encodeNativeAmmSetSwapFeeBpsCalldata(feeBps) {
    if (feeBps < 1n || feeBps > 10000n) {
        throw new RangeError('feeBps must satisfy 1 <= feeBps <= 10000');
    }
    const out = new Uint8Array(64);
    out.set(selectorWord(SELECTOR_NATIVE_AMM_SET_SWAP_FEE_BPS), 0);
    out.set(amountWord(feeBps), 32);
    return out;
}
export function encodeNativeAmmSwapCalldataHex(direction, amountIn, minOut) {
    return bytesToHex(encodeNativeAmmSwapCalldata(direction, amountIn, minOut));
}
export function encodeNativeAmmSwapToCalldataHex(direction, amountIn, minOut, recipientHex32) {
    return bytesToHex(encodeNativeAmmSwapToCalldata(direction, amountIn, minOut, recipientHex32));
}
export function encodeNativeAmmAddLiquidityCalldataHex(amountA, amountB, minLiquidity = 0n) {
    return bytesToHex(encodeNativeAmmAddLiquidityCalldata(amountA, amountB, minLiquidity));
}
export function encodeNativeAmmRemoveLiquidityCalldataHex(liquidityBurn, minA, minB) {
    return bytesToHex(encodeNativeAmmRemoveLiquidityCalldata(liquidityBurn, minA, minB));
}
export function encodeNativeAmmRemoveLiquidityToCalldataHex(liquidityBurn, minA, minB, recipientAHex32, recipientBHex32) {
    return bytesToHex(encodeNativeAmmRemoveLiquidityToCalldata(liquidityBurn, minA, minB, recipientAHex32, recipientBHex32));
}
export function encodeNativeAmmSetTokensCalldataHex(tokenAHex32, tokenBHex32) {
    return bytesToHex(encodeNativeAmmSetTokensCalldata(tokenAHex32, tokenBHex32));
}
export function encodeNativeAmmSetSwapFeeBpsCalldataHex(feeBps) {
    return bytesToHex(encodeNativeAmmSetSwapFeeBpsCalldata(feeBps));
}
/** Raw CP step (no swap fee): Δout = ⌊ r_out · Δin / (r_in + Δin) ⌋. */
export function constantProductAmountOutNoFee(reserveIn, reserveOut, amountIn) {
    if (reserveIn < 0n || reserveOut < 0n || amountIn < 0n) {
        throw new RangeError('reserves and amountIn must be non-negative');
    }
    const denom = reserveIn + amountIn;
    if (denom === 0n)
        return 0n;
    return (reserveOut * amountIn) / denom;
}
/**
 * Amount out after an explicit **output-side** fee in basis points (`native_amm::constant_product_amount_out_after_fee_with_bps`).
 * **`feeBps`** must be **`0`…`10000`** (inclusive). For **v3/v4** pools, if storage at `swap_fee_bps_key` reads **`0`**, treat as **`NATIVE_CP_SWAP_FEE_BPS`** before quoting.
 */
export function constantProductAmountOutWithFeeBps(reserveIn, reserveOut, amountIn, feeBps) {
    if (feeBps < 0n || feeBps > 10000n) {
        throw new RangeError('feeBps must satisfy 0 <= feeBps <= 10000');
    }
    const dy = constantProductAmountOutNoFee(reserveIn, reserveOut, amountIn);
    const keep = 10000n - feeBps;
    return (dy * keep) / 10000n;
}
/**
 * Amount out after pool swap fee (output-side): same as **`constantProductAmountOutWithFeeBps`** with **`NATIVE_CP_SWAP_FEE_BPS`**.
 */
export function constantProductAmountOut(reserveIn, reserveOut, amountIn) {
    return constantProductAmountOutWithFeeBps(reserveIn, reserveOut, amountIn, BigInt(NATIVE_CP_SWAP_FEE_BPS));
}
