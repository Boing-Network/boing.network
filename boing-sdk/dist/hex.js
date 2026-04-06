/**
 * Hex encoding/decoding for 32-byte IDs, hashes, and RPC params.
 */
/** Ensure string has 0x prefix. */
export function ensureHex(s) {
    const t = s.trim();
    return t.startsWith('0x') ? t : '0x' + t;
}
/** Encode bytes to hex with 0x prefix. */
export function bytesToHex(bytes) {
    return '0x' + Array.from(bytes)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');
}
const HEX_RE = /^[0-9a-fA-F]+$/;
/** Decode hex string to bytes (with or without 0x). */
export function hexToBytes(hex) {
    const raw = hex.trimStart().replace(/^0x/i, '');
    if (raw.length % 2 !== 0)
        throw new Error('Invalid hex: length must be even');
    if (!HEX_RE.test(raw))
        throw new Error('Invalid hex: expected 0-9, a-f, A-F');
    const bytes = new Uint8Array(raw.length / 2);
    for (let i = 0; i < bytes.length; i++) {
        bytes[i] = parseInt(raw.slice(i * 2, i * 2 + 2), 16);
    }
    return bytes;
}
/**
 * Normalize hex and require exactly 32 bytes (64 hex chars). Use for account IDs and hashes.
 * @returns Hex string with 0x prefix
 * @throws if not valid hex or length is not 64
 */
export function validateHex32(hex) {
    const normalized = ensureHex(hex);
    const raw = normalized.slice(2);
    if (raw.length !== 64)
        throw new Error(`Expected 32 bytes (64 hex chars), got ${raw.length} hex chars`);
    if (!HEX_RE.test(raw))
        throw new Error('Invalid hex: expected 0-9, a-f, A-F');
    return normalized;
}
/** True if **`hex`** is a valid **32-byte** Boing **`AccountId`** (`0x` + 64 hex). Use to branch wizards away from 20-byte EVM addresses. */
export function isBoingNativeAccountIdHex(hex) {
    try {
        validateHex32(hex);
        return true;
    }
    catch {
        return false;
    }
}
/** Assert 32-byte buffer; return as hex. */
export function accountIdToHex(bytes) {
    if (bytes.length !== 32)
        throw new Error('AccountId must be 32 bytes');
    return bytesToHex(bytes);
}
/** Parse 32-byte AccountId from hex. */
export function hexToAccountId(hex) {
    const bytes = hexToBytes(hex);
    if (bytes.length !== 32)
        throw new Error('AccountId must be 32 bytes hex');
    return bytes;
}
