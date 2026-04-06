/**
 * Native DEX **pair directory** calldata (Boing VM). Matches `boing_execution::native_dex_factory`.
 * See `docs/NATIVE-DEX-FACTORY.md`.
 */
/** `register_pair(token_a, token_b, pool)` — **128** bytes. */
export declare const SELECTOR_NATIVE_DEX_REGISTER_PAIR = 208;
/** `pairs_count` — **32** bytes; return data is one word (count in low **8** bytes). */
export declare const SELECTOR_NATIVE_DEX_PAIRS_COUNT = 209;
/** `get_pair_at(index)` — **64** bytes; return data is **96** bytes (three account words). */
export declare const SELECTOR_NATIVE_DEX_GET_PAIR_AT = 210;
/** Max pairs the on-chain directory stores (`NATIVE_DEX_FACTORY_MAX_PAIRS`). */
export declare const NATIVE_DEX_FACTORY_MAX_PAIRS = 4096;
/** `Log3` topic0 hex after `register_pair` (matches Rust `NATIVE_DEX_FACTORY_TOPIC_REGISTER`). */
export declare const NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX: string;
/** CREATE2 salt v1 (hex `0x` + 64 hex chars). */
export declare const NATIVE_DEX_FACTORY_CREATE2_SALT_V1_HEX: string;
/** Build `register_pair` calldata bytes. */
export declare function encodeNativeDexRegisterPairCalldata(tokenAHex32: string, tokenBHex32: string, poolHex32: string): Uint8Array;
/** `0x`-prefixed hex calldata for RPC / Express. */
export declare function encodeNativeDexRegisterPairCalldataHex(tokenAHex32: string, tokenBHex32: string, poolHex32: string): string;
export declare function encodeNativeDexPairsCountCalldata(): Uint8Array;
export declare function encodeNativeDexPairsCountCalldataHex(): string;
export declare function encodeNativeDexGetPairAtCalldata(index: number): Uint8Array;
export declare function encodeNativeDexGetPairAtCalldataHex(index: number): string;
//# sourceMappingURL=nativeDexFactory.d.ts.map