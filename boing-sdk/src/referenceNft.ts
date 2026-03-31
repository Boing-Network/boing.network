/**
 * Reference NFT calldata layout (Boing-defined). See `docs/BOING-REFERENCE-NFT.md`.
 */

import { hexToBytes, bytesToHex, ensureHex } from './hex.js';

export const SELECTOR_OWNER_OF = 0x03;
export const SELECTOR_TRANSFER_NFT = 0x04;
export const SELECTOR_SET_METADATA_HASH = 0x05;

function selectorWord(selector: number): Uint8Array {
  const w = new Uint8Array(32);
  w[31] = selector & 0xff;
  return w;
}

function word32(hexOrBytes: string | Uint8Array): Uint8Array {
  if (hexOrBytes instanceof Uint8Array) {
    if (hexOrBytes.length !== 32) throw new Error('word must be 32 bytes');
    return hexOrBytes;
  }
  const b = hexToBytes(ensureHex(hexOrBytes));
  if (b.length !== 32) throw new Error('word must be 32 bytes hex');
  return b;
}

/** 96-byte `owner_of(token_id)` reference calldata. */
export function encodeReferenceOwnerOfCalldata(tokenIdHex32: string): Uint8Array {
  const out = new Uint8Array(96);
  out.set(selectorWord(SELECTOR_OWNER_OF), 0);
  out.set(word32(tokenIdHex32), 32);
  return out;
}

/** 96-byte `transfer_nft(to, token_id)` reference calldata. */
export function encodeReferenceTransferNftCalldata(
  toHexAccount32: string,
  tokenIdHex32: string
): Uint8Array {
  const out = new Uint8Array(96);
  out.set(selectorWord(SELECTOR_TRANSFER_NFT), 0);
  out.set(word32(toHexAccount32), 32);
  out.set(word32(tokenIdHex32), 64);
  return out;
}

/** 96-byte `set_metadata_hash(token_id, hash)` reference calldata. */
export function encodeReferenceSetMetadataHashCalldata(
  tokenIdHex32: string,
  metadataHashHex32: string
): Uint8Array {
  const out = new Uint8Array(96);
  out.set(selectorWord(SELECTOR_SET_METADATA_HASH), 0);
  out.set(word32(tokenIdHex32), 32);
  out.set(word32(metadataHashHex32), 64);
  return out;
}

export function encodeReferenceOwnerOfCalldataHex(tokenIdHex32: string): string {
  return bytesToHex(encodeReferenceOwnerOfCalldata(tokenIdHex32));
}
