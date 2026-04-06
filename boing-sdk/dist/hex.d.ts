/**
 * Hex encoding/decoding for 32-byte IDs, hashes, and RPC params.
 */
/** Ensure string has 0x prefix. */
export declare function ensureHex(s: string): string;
/** Encode bytes to hex with 0x prefix. */
export declare function bytesToHex(bytes: Uint8Array): string;
/** Decode hex string to bytes (with or without 0x). */
export declare function hexToBytes(hex: string): Uint8Array;
/**
 * Normalize hex and require exactly 32 bytes (64 hex chars). Use for account IDs and hashes.
 * @returns Hex string with 0x prefix
 * @throws if not valid hex or length is not 64
 */
export declare function validateHex32(hex: string): string;
/** True if **`hex`** is a valid **32-byte** Boing **`AccountId`** (`0x` + 64 hex). Use to branch wizards away from 20-byte EVM addresses. */
export declare function isBoingNativeAccountIdHex(hex: string): boolean;
/** Assert 32-byte buffer; return as hex. */
export declare function accountIdToHex(bytes: Uint8Array): string;
/** Parse 32-byte AccountId from hex. */
export declare function hexToAccountId(hex: string): Uint8Array;
//# sourceMappingURL=hex.d.ts.map