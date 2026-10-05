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
export const SELECTOR_MINT_BATCH = 0x06;

/** Bytecode cap; production `GAS_PER_CONTRACT_CALL` (40_000_000) is sized for this `n` with owner+metadata stores. */
export const MAX_REFERENCE_NFT_MINT_BATCH = 500;

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

export function encodeReferenceTransferNftCalldataHex(
  toHexAccount32: string,
  tokenIdHex32: string
): string {
  return bytesToHex(encodeReferenceTransferNftCalldata(toHexAccount32, tokenIdHex32));
}

export function encodeReferenceSetMetadataHashCalldataHex(
  tokenIdHex32: string,
  metadataHashHex32: string
): string {
  return bytesToHex(encodeReferenceSetMetadataHashCalldata(tokenIdHex32, metadataHashHex32));
}

/**
 * Variable-length `mint_batch` calldata: `96 + 64n` bytes.
 *
 * | Offset | Content |
 * | 0..31 | selector word (low byte `0x06`) |
 * | 32..63 | `to` AccountId |
 * | 64..95 | `n` as big-endian u64 in the low 8 bytes |
 * | 96 .. 96+32n-1 | `tokenIds[i]` |
 * | 96+32n .. 96+64n-1 | `metadataHashes[i]` |
 *
 * All-zero metadata hashes skip `SSTORE` on-chain (option B). `n` must be in `1..=MAX_REFERENCE_NFT_MINT_BATCH`.
 */
export function encodeReferenceMintBatchCalldata(
  toHexAccount32: string,
  tokenIdsHex32: readonly string[],
  metadataHashesHex32: readonly string[]
): Uint8Array {
  if (tokenIdsHex32.length !== metadataHashesHex32.length) {
    throw new RangeError(
      `mint_batch length mismatch: ${tokenIdsHex32.length} token ids vs ${metadataHashesHex32.length} hashes`
    );
  }
  const n = tokenIdsHex32.length;
  if (n < 1 || n > MAX_REFERENCE_NFT_MINT_BATCH) {
    throw new RangeError(`mint_batch n must be 1..=${MAX_REFERENCE_NFT_MINT_BATCH}, got ${n}`);
  }
  const out = new Uint8Array(96 + 64 * n);
  out.set(calldataSelectorLastByte(SELECTOR_MINT_BATCH), 0);
  out.set(calldataAccountIdWord(toHexAccount32), 32);
  const nWord = new Uint8Array(32);
  let x = BigInt(n);
  for (let i = 31; i >= 24; i--) {
    nWord[i] = Number(x & 0xffn);
    x >>= 8n;
  }
  out.set(nWord, 64);
  for (let i = 0; i < n; i++) {
    out.set(calldataFixedWord32(tokenIdsHex32[i]!), 96 + 32 * i);
    out.set(calldataFixedWord32(metadataHashesHex32[i]!), 96 + 32 * n + 32 * i);
  }
  return out;
}

export function encodeReferenceMintBatchCalldataHex(
  toHexAccount32: string,
  tokenIdsHex32: readonly string[],
  metadataHashesHex32: readonly string[]
): string {
  return bytesToHex(encodeReferenceMintBatchCalldata(toHexAccount32, tokenIdsHex32, metadataHashesHex32));
}
