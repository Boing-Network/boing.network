/**
 * Generic Boing VM **calldata word** helpers (32-byte stack-word style).
 * Reference token/NFT layouts use selector in the **last** byte of the first word; custom contracts may
 * reuse these primitives for consistent SDK ergonomics. See `docs/BOING-REFERENCE-TOKEN.md`.
 */
import { ensureHex, hexToBytes } from './hex.js';
/** Ensure `bytes` is exactly 32 bytes for use as a calldata argument word. */
export function assertBoingCalldataWord(bytes) {
    if (bytes.length !== 32) {
        throw new Error(`BoingCalldataWord must be 32 bytes, got ${bytes.length}`);
    }
    return bytes;
}
/** Typed selector word (low byte only; rest zero), for {@link encodeBoingCall} argument lists. */
export function boingWordSelector(lowByte) {
    return assertBoingCalldataWord(calldataSelectorLastByte(lowByte));
}
/** Typed u128-in-low-16-bytes word. */
export function boingWordU128(value) {
    return assertBoingCalldataWord(calldataU128BeWord(value));
}
/** Typed 32-byte account / blob word. */
export function boingWordAccount(hexAccount32) {
    return assertBoingCalldataWord(calldataAccountIdWord(hexAccount32));
}
/** Typed fixed 32-byte word from hex or bytes. */
export function boingWordFixed(hexOrBytes) {
    return assertBoingCalldataWord(calldataFixedWord32(hexOrBytes));
}
/**
 * Build calldata: first word is `selectorLowByte` in the **last** byte (reference layout), then each `args` word (32 bytes).
 * For arbitrary trailing bytes (non–word-aligned layouts), use {@link concatCalldata} directly.
 */
export function encodeBoingCall(selectorLowByte, args) {
    const head = calldataSelectorLastByte(selectorLowByte);
    return concatCalldata([head, ...args]);
}
/** 32-byte word with single-byte selector in the low byte (reference layout). */
export function calldataSelectorLastByte(selector) {
    const w = new Uint8Array(32);
    w[31] = selector & 0xff;
    return w;
}
/** 32-byte word: unsigned 128-bit value in the low 16 bytes (big-endian); high 16 bytes zero. */
export function calldataU128BeWord(value) {
    const w = new Uint8Array(32);
    if (value < 0n || value > (1n << 128n) - 1n) {
        throw new RangeError('value must fit in u128');
    }
    const be = new Uint8Array(16);
    let x = value;
    for (let i = 15; i >= 0; i--) {
        be[i] = Number(x & 0xffn);
        x >>= 8n;
    }
    w.set(be, 16);
    return w;
}
/** 32-byte word containing a 32-byte Boing `AccountId` (or any 32-byte blob), left-aligned. */
export function calldataAccountIdWord(hexAccount32) {
    const b = hexToBytes(ensureHex(hexAccount32));
    if (b.length !== 32) {
        throw new Error('account id must be 32 bytes hex');
    }
    const w = new Uint8Array(32);
    w.set(b);
    return w;
}
/** Normalize to a 32-byte calldata word (hex string or `Uint8Array`). */
export function calldataFixedWord32(hexOrBytes) {
    if (hexOrBytes instanceof Uint8Array) {
        if (hexOrBytes.length !== 32)
            throw new Error('word must be 32 bytes');
        return hexOrBytes;
    }
    const b = hexToBytes(ensureHex(hexOrBytes));
    if (b.length !== 32)
        throw new Error('word must be 32 bytes hex');
    return b;
}
/** Concatenate calldata segments (often 32-byte words). */
export function concatCalldata(parts) {
    let n = 0;
    for (const p of parts)
        n += p.length;
    const out = new Uint8Array(n);
    let o = 0;
    for (const p of parts) {
        out.set(p, o);
        o += p.length;
    }
    return out;
}
