/**
 * Reference fungible calldata layout (Boing-defined). See `docs/BOING-REFERENCE-TOKEN.md`.
 */

import { hexToBytes, bytesToHex, ensureHex } from './hex.js';

/** Selector low byte for reference `transfer`. */
export const SELECTOR_TRANSFER = 0x01;
/** Selector low byte for reference first-mint style hook. */
export const SELECTOR_MINT_FIRST = 0x02;

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

function accountIdBytes(hexAccount32: string): Uint8Array {
  const b = hexToBytes(ensureHex(hexAccount32));
  if (b.length !== 32) {
    throw new Error('account id must be 32 bytes hex');
  }
  return b;
}

/** Build 96-byte reference `transfer(to, amount)` calldata. */
export function encodeReferenceTransferCalldata(toHexAccount32: string, amount: bigint): Uint8Array {
  const out = new Uint8Array(96);
  out.set(selectorWord(SELECTOR_TRANSFER), 0);
  out.set(accountIdBytes(toHexAccount32), 32);
  out.set(amountWord(amount), 64);
  return out;
}

/** Build 96-byte reference `mint_first` calldata. */
export function encodeReferenceMintFirstCalldata(toHexAccount32: string, amount: bigint): Uint8Array {
  const out = new Uint8Array(96);
  out.set(selectorWord(SELECTOR_MINT_FIRST), 0);
  out.set(accountIdBytes(toHexAccount32), 32);
  out.set(amountWord(amount), 64);
  return out;
}

/** Hex `0x` + 96-byte reference transfer calldata. */
export function encodeReferenceTransferCalldataHex(toHexAccount32: string, amount: bigint): string {
  return bytesToHex(encodeReferenceTransferCalldata(toHexAccount32, amount));
}
