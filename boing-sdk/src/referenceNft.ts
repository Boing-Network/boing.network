/**
 * Reference NFT calldata layout (Boing-defined). See `docs/BOING-REFERENCE-NFT.md`.
 */

import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
import {
  calldataAccountIdWord,
  calldataFixedWord32,
  calldataSelectorLastByte,
} from './calldata.js';

export const SELECTOR_OWNER_OF = 0x03;
export const SELECTOR_TRANSFER_NFT = 0x04;
export const SELECTOR_SET_METADATA_HASH = 0x05;

/** XOR mask for owner slot — mirrors `REF_NFT_OWNER_STORAGE_XOR` in `reference_nft.rs`. */
export const REF_NFT_OWNER_STORAGE_XOR_HEX = validateHex32(
  '0x424f494e475f5245464e46545f4f574e45523031000000000000000000000000',
);

/** XOR mask for metadata hash slot — mirrors `REF_NFT_METADATA_STORAGE_XOR` in `reference_nft.rs`. */
export const REF_NFT_METADATA_STORAGE_XOR_HEX = validateHex32(
  '0x424f494e475f5245464e46545f4d455441303100000000000000000000000000',
);

function xorWords(a: Uint8Array, b: Uint8Array): Uint8Array {
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) out[i] = a[i]! ^ b[i]!;
  return out;
}

function xorStorageKey(tokenIdHex32: string, xorHex32: string): string {
  const tokenId = hexToBytes(validateHex32(tokenIdHex32));
  const mask = hexToBytes(validateHex32(xorHex32));
  return bytesToHex(xorWords(tokenId, mask));
}

/** `SLOAD` key for reference NFT owner: `token_id ^ REF_NFT_OWNER_STORAGE_XOR`. */
export function referenceNftOwnerStorageKey(tokenIdHex32: string): string {
  return xorStorageKey(tokenIdHex32, REF_NFT_OWNER_STORAGE_XOR_HEX);
}

/** `SLOAD` key for reference NFT metadata hash: `token_id ^ REF_NFT_METADATA_STORAGE_XOR`. */
export function referenceNftMetadataStorageKey(tokenIdHex32: string): string {
  return xorStorageKey(tokenIdHex32, REF_NFT_METADATA_STORAGE_XOR_HEX);
}

/**
 * Encode a sequential token id as a 32-byte word (big-endian u64 in the **low 8 bytes**).
 * Matches common reference collection usage; arbitrary ids may use {@link calldataFixedWord32} instead.
 */
export function referenceNftTokenIdWordFromU64(id: bigint | number): string {
  let n = BigInt(id);
  if (n < 0n) throw new RangeError('token id must be non-negative');
  const out = new Uint8Array(32);
  for (let i = 31; i >= 24; i--) {
    out[i] = Number(n & 0xffn);
    n >>= 8n;
  }
  return bytesToHex(out);
}

/** 96-byte `owner_of(token_id)` reference calldata. */
export function encodeReferenceOwnerOfCalldata(tokenIdHex32: string): Uint8Array {
  const out = new Uint8Array(96);
  out.set(calldataSelectorLastByte(SELECTOR_OWNER_OF), 0);
  out.set(calldataFixedWord32(tokenIdHex32), 32);
  return out;
}

/** 96-byte `transfer_nft(to, token_id)` reference calldata. */
export function encodeReferenceTransferNftCalldata(
  toHexAccount32: string,
  tokenIdHex32: string
): Uint8Array {
  const out = new Uint8Array(96);
  out.set(calldataSelectorLastByte(SELECTOR_TRANSFER_NFT), 0);
  out.set(calldataAccountIdWord(toHexAccount32), 32);
  out.set(calldataFixedWord32(tokenIdHex32), 64);
  return out;
}

/** 96-byte `set_metadata_hash(token_id, hash)` reference calldata. */
export function encodeReferenceSetMetadataHashCalldata(
  tokenIdHex32: string,
  metadataHashHex32: string
): Uint8Array {
  const out = new Uint8Array(96);
  out.set(calldataSelectorLastByte(SELECTOR_SET_METADATA_HASH), 0);
  out.set(calldataFixedWord32(tokenIdHex32), 32);
  out.set(calldataFixedWord32(metadataHashHex32), 64);
  return out;
}

export function encodeReferenceOwnerOfCalldataHex(tokenIdHex32: string): string {
  return bytesToHex(encodeReferenceOwnerOfCalldata(tokenIdHex32));
}
