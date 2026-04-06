/**
 * Native DEX **pair directory** calldata (Boing VM). Matches `boing_execution::native_dex_factory`.
 * See `docs/NATIVE-DEX-FACTORY.md`.
 */

import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';

/** `register_pair(token_a, token_b, pool)` — **128** bytes. */
export const SELECTOR_NATIVE_DEX_REGISTER_PAIR = 0xd0;
/** `pairs_count` — **32** bytes; return data is one word (count in low **8** bytes). */
export const SELECTOR_NATIVE_DEX_PAIRS_COUNT = 0xd1;
/** `get_pair_at(index)` — **64** bytes; return data is **96** bytes (three account words). */
export const SELECTOR_NATIVE_DEX_GET_PAIR_AT = 0xd2;

/** Max pairs the on-chain directory stores (`NATIVE_DEX_FACTORY_MAX_PAIRS`). */
export const NATIVE_DEX_FACTORY_MAX_PAIRS = 4096;

/** `Log3` topic0 hex after `register_pair` (matches Rust `NATIVE_DEX_FACTORY_TOPIC_REGISTER`). */
export const NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX = (() => {
  const u8 = new Uint8Array(32);
  u8.set(new TextEncoder().encode('BOING_NATIVE_DEX_FACTORY_REG1'));
  return bytesToHex(u8);
})();

/** CREATE2 salt v1 (hex `0x` + 64 hex chars). */
export const NATIVE_DEX_FACTORY_CREATE2_SALT_V1_HEX = (() => {
  const u8 = new Uint8Array(32);
  const s = new TextEncoder().encode('BOING_NATIVEDEX_FACTORY_V1');
  u8.set(s);
  return bytesToHex(u8);
})();

function selectorWord(selector: number): Uint8Array {
  const w = new Uint8Array(32);
  w[31] = selector & 0xff;
  return w;
}

function u64Word(n: number): Uint8Array {
  if (!Number.isInteger(n) || n < 0 || n > Number.MAX_SAFE_INTEGER) {
    throw new RangeError('index must be a non-negative safe integer');
  }
  const w = new Uint8Array(32);
  new DataView(w.buffer).setBigUint64(24, BigInt(n), false);
  return w;
}

function concatWords(parts: Uint8Array[]): Uint8Array {
  const n = parts.reduce((a, p) => a + p.length, 0);
  const out = new Uint8Array(n);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

/** Build `register_pair` calldata bytes. */
export function encodeNativeDexRegisterPairCalldata(
  tokenAHex32: string,
  tokenBHex32: string,
  poolHex32: string
): Uint8Array {
  const a = hexToBytes(validateHex32(tokenAHex32));
  const b = hexToBytes(validateHex32(tokenBHex32));
  const p = hexToBytes(validateHex32(poolHex32));
  return concatWords([selectorWord(SELECTOR_NATIVE_DEX_REGISTER_PAIR), a, b, p]);
}

/** `0x`-prefixed hex calldata for RPC / Express. */
export function encodeNativeDexRegisterPairCalldataHex(
  tokenAHex32: string,
  tokenBHex32: string,
  poolHex32: string
): string {
  return bytesToHex(encodeNativeDexRegisterPairCalldata(tokenAHex32, tokenBHex32, poolHex32));
}

export function encodeNativeDexPairsCountCalldata(): Uint8Array {
  return selectorWord(SELECTOR_NATIVE_DEX_PAIRS_COUNT);
}

export function encodeNativeDexPairsCountCalldataHex(): string {
  return bytesToHex(encodeNativeDexPairsCountCalldata());
}

export function encodeNativeDexGetPairAtCalldata(index: number): Uint8Array {
  return concatWords([selectorWord(SELECTOR_NATIVE_DEX_GET_PAIR_AT), u64Word(index)]);
}

export function encodeNativeDexGetPairAtCalldataHex(index: number): string {
  return bytesToHex(encodeNativeDexGetPairAtCalldata(index));
}
