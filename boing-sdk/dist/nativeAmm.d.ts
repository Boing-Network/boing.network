/**
 * Native constant-product pool calldata (Boing VM). See `docs/NATIVE-AMM-CALLDATA.md`.
 * Reserves and amounts should stay ≤ `Number.MAX_SAFE_INTEGER` / u64 range for VM `Mul` parity.
 */
/** `swap` selector (low byte of word0). */
export declare const SELECTOR_NATIVE_AMM_SWAP = 16;
/** `add_liquidity` selector. */
export declare const SELECTOR_NATIVE_AMM_ADD_LIQUIDITY = 17;
/** `remove_liquidity` selector (LP burn + pro-rata withdrawal). */
export declare const SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY = 18;
/** **v2 pool:** one-time `set_tokens(token_a, token_b)`. */
export declare const SELECTOR_NATIVE_AMM_SET_TOKENS = 19;
/** **v3/v4 pool:** `set_swap_fee_bps(fee)` — **64-byte** calldata; only when **total LP == 0**; **`1 ≤ fee ≤ 10_000`**. */
export declare const SELECTOR_NATIVE_AMM_SET_SWAP_FEE_BPS = 20;
/** **v5 pool:** `swap_to` — like `swap` plus **word4** = output recipient (**160-byte** calldata). */
export declare const SELECTOR_NATIVE_AMM_SWAP_TO = 21;
/** **v5 pool:** `remove_liquidity_to` — like `remove_liquidity` plus **recipient_a** / **recipient_b** (**192-byte** calldata). */
export declare const SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY_TO = 22;
/** Swap fee in basis points on **output** (matches `native_amm::NATIVE_CP_SWAP_FEE_BPS`). */
export declare const NATIVE_CP_SWAP_FEE_BPS = 30;
/** Build 32-byte `Log2` **topic0** (UTF-8 ASCII + zero pad), matching `native_amm` constants. */
export declare function nativeAmmLogTopic0Utf8(ascii: string): Uint8Array;
/** `Log2` topic0 hex for a successful **`swap`** (see `NATIVE-AMM-CALLDATA.md` § Logs). */
export declare const NATIVE_AMM_TOPIC_SWAP_HEX: string;
/** `Log2` topic0 hex for **`add_liquidity`**. */
export declare const NATIVE_AMM_TOPIC_ADD_LIQUIDITY_HEX: string;
/** `Log2` topic0 hex for **`remove_liquidity`**. */
export declare const NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY_HEX: string;
/** 128-byte `swap` calldata: direction 0 = A→B, 1 = B→A. */
export declare function encodeNativeAmmSwapCalldata(direction: bigint, amountIn: bigint, minOut: bigint): Uint8Array;
/** **160-byte** `swap_to` calldata (v5 pool): explicit **output recipient** for token `transfer`. */
export declare function encodeNativeAmmSwapToCalldata(direction: bigint, amountIn: bigint, minOut: bigint, recipientHex32: string): Uint8Array;
/**
 * 128-byte `add_liquidity` calldata. On success the pool **returns** **32** bytes: LP minted in this
 * call as **u128** in the **low 16 bytes** of the word (big-endian word; value in bytes 16..32).
 */
export declare function encodeNativeAmmAddLiquidityCalldata(amountA: bigint, amountB: bigint, minLiquidity: bigint): Uint8Array;
/** 128-byte `remove_liquidity` calldata. */
export declare function encodeNativeAmmRemoveLiquidityCalldata(liquidityBurn: bigint, minA: bigint, minB: bigint): Uint8Array;
/** **192-byte** `remove_liquidity_to` (v5 pool): explicit **recipient_a** / **recipient_b** for reference-token `transfer`. */
export declare function encodeNativeAmmRemoveLiquidityToCalldata(liquidityBurn: bigint, minA: bigint, minB: bigint, recipientAHex32: string, recipientBHex32: string): Uint8Array;
/** **v2:** 96-byte `set_tokens` — each id is 32-byte account hex (`0x` + 64 hex). Use `0x` + 64 zeros for “no token” on that side. */
export declare function encodeNativeAmmSetTokensCalldata(tokenAHex32: string, tokenBHex32: string): Uint8Array;
/** **v3/v4:** 64-byte `set_swap_fee_bps` calldata (`native_amm::encode_set_swap_fee_bps_calldata`). */
export declare function encodeNativeAmmSetSwapFeeBpsCalldata(feeBps: bigint): Uint8Array;
export declare function encodeNativeAmmSwapCalldataHex(direction: bigint, amountIn: bigint, minOut: bigint): string;
export declare function encodeNativeAmmSwapToCalldataHex(direction: bigint, amountIn: bigint, minOut: bigint, recipientHex32: string): string;
export declare function encodeNativeAmmAddLiquidityCalldataHex(amountA: bigint, amountB: bigint, minLiquidity?: bigint): string;
export declare function encodeNativeAmmRemoveLiquidityCalldataHex(liquidityBurn: bigint, minA: bigint, minB: bigint): string;
export declare function encodeNativeAmmRemoveLiquidityToCalldataHex(liquidityBurn: bigint, minA: bigint, minB: bigint, recipientAHex32: string, recipientBHex32: string): string;
export declare function encodeNativeAmmSetTokensCalldataHex(tokenAHex32: string, tokenBHex32: string): string;
export declare function encodeNativeAmmSetSwapFeeBpsCalldataHex(feeBps: bigint): string;
/** Raw CP step (no swap fee): Δout = ⌊ r_out · Δin / (r_in + Δin) ⌋. */
export declare function constantProductAmountOutNoFee(reserveIn: bigint, reserveOut: bigint, amountIn: bigint): bigint;
/**
 * Amount out after an explicit **output-side** fee in basis points (`native_amm::constant_product_amount_out_after_fee_with_bps`).
 * **`feeBps`** must be **`0`…`10000`** (inclusive). For **v3/v4** pools, if storage at `swap_fee_bps_key` reads **`0`**, treat as **`NATIVE_CP_SWAP_FEE_BPS`** before quoting.
 */
export declare function constantProductAmountOutWithFeeBps(reserveIn: bigint, reserveOut: bigint, amountIn: bigint, feeBps: bigint): bigint;
/**
 * Amount out after pool swap fee (output-side): same as **`constantProductAmountOutWithFeeBps`** with **`NATIVE_CP_SWAP_FEE_BPS`**.
 */
export declare function constantProductAmountOut(reserveIn: bigint, reserveOut: bigint, amountIn: bigint): bigint;
//# sourceMappingURL=nativeAmm.d.ts.map