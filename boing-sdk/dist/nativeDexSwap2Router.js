/**
 * Multihop swap router: **2–4** sequential pool `Call`s in one transaction.
 * Matches `boing_execution::native_dex_multihop_swap_router`. See `docs/NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md`.
 */
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
/** **352-byte** outer calldata; **128-byte** inners (v1/v3 `swap`). */
export const SELECTOR_NATIVE_DEX_SWAP2_ROUTER_128 = 0xe5;
/** **416-byte** outer calldata; **160-byte** inners (v5 `swap_to`). */
export const SELECTOR_NATIVE_DEX_SWAP2_ROUTER_160 = 0xe6;
/** **512-byte** outer; **128-byte** inners. */
export const SELECTOR_NATIVE_DEX_SWAP3_ROUTER_128 = 0xe7;
/** **608-byte** outer; **160-byte** inners. */
export const SELECTOR_NATIVE_DEX_SWAP3_ROUTER_160 = 0xe8;
/** **672-byte** outer; **128-byte** inners. */
export const SELECTOR_NATIVE_DEX_SWAP4_ROUTER_128 = 0xe9;
/** **800-byte** outer; **160-byte** inners. */
export const SELECTOR_NATIVE_DEX_SWAP4_ROUTER_160 = 0xea;
function selectorWord(selector) {
    const w = new Uint8Array(32);
    w[31] = selector & 0xff;
    return w;
}
export function encodeNativeDexSwap2RouterCalldata128(pool1Hex32, inner1_128, pool2Hex32, inner2_128) {
    if (inner1_128.length !== 128 || inner2_128.length !== 128) {
        throw new Error('inner calldata must be 128 bytes per hop');
    }
    const p1 = hexToBytes(validateHex32(pool1Hex32));
    const p2 = hexToBytes(validateHex32(pool2Hex32));
    const out = new Uint8Array(352);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_SWAP2_ROUTER_128));
    out.set(p1, 32);
    out.set(inner1_128, 64);
    out.set(p2, 192);
    out.set(inner2_128, 224);
    return out;
}
export function encodeNativeDexSwap2RouterCalldata128Hex(pool1Hex32, inner1_128, pool2Hex32, inner2_128) {
    return bytesToHex(encodeNativeDexSwap2RouterCalldata128(pool1Hex32, inner1_128, pool2Hex32, inner2_128));
}
export function encodeNativeDexSwap2RouterCalldata160(pool1Hex32, inner1_160, pool2Hex32, inner2_160) {
    if (inner1_160.length !== 160 || inner2_160.length !== 160) {
        throw new Error('inner calldata must be 160 bytes per hop');
    }
    const p1 = hexToBytes(validateHex32(pool1Hex32));
    const p2 = hexToBytes(validateHex32(pool2Hex32));
    const out = new Uint8Array(416);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_SWAP2_ROUTER_160));
    out.set(p1, 32);
    out.set(inner1_160, 64);
    out.set(p2, 224);
    out.set(inner2_160, 256);
    return out;
}
export function encodeNativeDexSwap2RouterCalldata160Hex(pool1Hex32, inner1_160, pool2Hex32, inner2_160) {
    return bytesToHex(encodeNativeDexSwap2RouterCalldata160(pool1Hex32, inner1_160, pool2Hex32, inner2_160));
}
export function encodeNativeDexSwap3RouterCalldata128(pool1Hex32, inner1_128, pool2Hex32, inner2_128, pool3Hex32, inner3_128) {
    for (const inner of [inner1_128, inner2_128, inner3_128]) {
        if (inner.length !== 128) {
            throw new Error('inner calldata must be 128 bytes per hop');
        }
    }
    const p1 = hexToBytes(validateHex32(pool1Hex32));
    const p2 = hexToBytes(validateHex32(pool2Hex32));
    const p3 = hexToBytes(validateHex32(pool3Hex32));
    const out = new Uint8Array(512);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_SWAP3_ROUTER_128));
    out.set(p1, 32);
    out.set(inner1_128, 64);
    out.set(p2, 192);
    out.set(inner2_128, 224);
    out.set(p3, 352);
    out.set(inner3_128, 384);
    return out;
}
export function encodeNativeDexSwap3RouterCalldata128Hex(pool1Hex32, inner1_128, pool2Hex32, inner2_128, pool3Hex32, inner3_128) {
    return bytesToHex(encodeNativeDexSwap3RouterCalldata128(pool1Hex32, inner1_128, pool2Hex32, inner2_128, pool3Hex32, inner3_128));
}
export function encodeNativeDexSwap3RouterCalldata160(pool1Hex32, inner1_160, pool2Hex32, inner2_160, pool3Hex32, inner3_160) {
    for (const inner of [inner1_160, inner2_160, inner3_160]) {
        if (inner.length !== 160) {
            throw new Error('inner calldata must be 160 bytes per hop');
        }
    }
    const p1 = hexToBytes(validateHex32(pool1Hex32));
    const p2 = hexToBytes(validateHex32(pool2Hex32));
    const p3 = hexToBytes(validateHex32(pool3Hex32));
    const out = new Uint8Array(608);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_SWAP3_ROUTER_160));
    out.set(p1, 32);
    out.set(inner1_160, 64);
    out.set(p2, 224);
    out.set(inner2_160, 256);
    out.set(p3, 416);
    out.set(inner3_160, 448);
    return out;
}
export function encodeNativeDexSwap3RouterCalldata160Hex(pool1Hex32, inner1_160, pool2Hex32, inner2_160, pool3Hex32, inner3_160) {
    return bytesToHex(encodeNativeDexSwap3RouterCalldata160(pool1Hex32, inner1_160, pool2Hex32, inner2_160, pool3Hex32, inner3_160));
}
export function encodeNativeDexSwap4RouterCalldata128(pool1Hex32, inner1_128, pool2Hex32, inner2_128, pool3Hex32, inner3_128, pool4Hex32, inner4_128) {
    for (const inner of [inner1_128, inner2_128, inner3_128, inner4_128]) {
        if (inner.length !== 128) {
            throw new Error('inner calldata must be 128 bytes per hop');
        }
    }
    const p1 = hexToBytes(validateHex32(pool1Hex32));
    const p2 = hexToBytes(validateHex32(pool2Hex32));
    const p3 = hexToBytes(validateHex32(pool3Hex32));
    const p4 = hexToBytes(validateHex32(pool4Hex32));
    const out = new Uint8Array(672);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_SWAP4_ROUTER_128));
    out.set(p1, 32);
    out.set(inner1_128, 64);
    out.set(p2, 192);
    out.set(inner2_128, 224);
    out.set(p3, 352);
    out.set(inner3_128, 384);
    out.set(p4, 512);
    out.set(inner4_128, 544);
    return out;
}
export function encodeNativeDexSwap4RouterCalldata128Hex(pool1Hex32, inner1_128, pool2Hex32, inner2_128, pool3Hex32, inner3_128, pool4Hex32, inner4_128) {
    return bytesToHex(encodeNativeDexSwap4RouterCalldata128(pool1Hex32, inner1_128, pool2Hex32, inner2_128, pool3Hex32, inner3_128, pool4Hex32, inner4_128));
}
export function encodeNativeDexSwap4RouterCalldata160(pool1Hex32, inner1_160, pool2Hex32, inner2_160, pool3Hex32, inner3_160, pool4Hex32, inner4_160) {
    for (const inner of [inner1_160, inner2_160, inner3_160, inner4_160]) {
        if (inner.length !== 160) {
            throw new Error('inner calldata must be 160 bytes per hop');
        }
    }
    const p1 = hexToBytes(validateHex32(pool1Hex32));
    const p2 = hexToBytes(validateHex32(pool2Hex32));
    const p3 = hexToBytes(validateHex32(pool3Hex32));
    const p4 = hexToBytes(validateHex32(pool4Hex32));
    const out = new Uint8Array(800);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_SWAP4_ROUTER_160));
    out.set(p1, 32);
    out.set(inner1_160, 64);
    out.set(p2, 224);
    out.set(inner2_160, 256);
    out.set(p3, 416);
    out.set(inner3_160, 448);
    out.set(p4, 608);
    out.set(inner4_160, 640);
    return out;
}
export function encodeNativeDexSwap4RouterCalldata160Hex(pool1Hex32, inner1_160, pool2Hex32, inner2_160, pool3Hex32, inner3_160, pool4Hex32, inner4_160) {
    return bytesToHex(encodeNativeDexSwap4RouterCalldata160(pool1Hex32, inner1_160, pool2Hex32, inner2_160, pool3Hex32, inner3_160, pool4Hex32, inner4_160));
}
