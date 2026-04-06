/**
 * Generic Boing VM **calldata word** helpers (32-byte stack-word style).
 * Reference token/NFT layouts use selector in the **last** byte of the first word; custom contracts may
 * reuse these primitives for consistent SDK ergonomics. See `docs/BOING-REFERENCE-TOKEN.md`.
 */
/** Exactly 32 bytes — use {@link assertBoingCalldataWord} or `boingWord*` constructors. */
export type BoingCalldataWord = Uint8Array & {
    readonly __boingCalldataWord?: true;
};
/** Ensure `bytes` is exactly 32 bytes for use as a calldata argument word. */
export declare function assertBoingCalldataWord(bytes: Uint8Array): BoingCalldataWord;
/** Typed selector word (low byte only; rest zero), for {@link encodeBoingCall} argument lists. */
export declare function boingWordSelector(lowByte: number): BoingCalldataWord;
/** Typed u128-in-low-16-bytes word. */
export declare function boingWordU128(value: bigint): BoingCalldataWord;
/** Typed 32-byte account / blob word. */
export declare function boingWordAccount(hexAccount32: string): BoingCalldataWord;
/** Typed fixed 32-byte word from hex or bytes. */
export declare function boingWordFixed(hexOrBytes: string | Uint8Array): BoingCalldataWord;
/**
 * Build calldata: first word is `selectorLowByte` in the **last** byte (reference layout), then each `args` word (32 bytes).
 * For arbitrary trailing bytes (non–word-aligned layouts), use {@link concatCalldata} directly.
 */
export declare function encodeBoingCall(selectorLowByte: number, args: readonly BoingCalldataWord[]): Uint8Array;
/** 32-byte word with single-byte selector in the low byte (reference layout). */
export declare function calldataSelectorLastByte(selector: number): Uint8Array;
/** 32-byte word: unsigned 128-bit value in the low 16 bytes (big-endian); high 16 bytes zero. */
export declare function calldataU128BeWord(value: bigint): Uint8Array;
/** 32-byte word containing a 32-byte Boing `AccountId` (or any 32-byte blob), left-aligned. */
export declare function calldataAccountIdWord(hexAccount32: string): Uint8Array;
/** Normalize to a 32-byte calldata word (hex string or `Uint8Array`). */
export declare function calldataFixedWord32(hexOrBytes: string | Uint8Array): Uint8Array;
/** Concatenate calldata segments (often 32-byte words). */
export declare function concatCalldata(parts: Uint8Array[]): Uint8Array;
//# sourceMappingURL=calldata.d.ts.map