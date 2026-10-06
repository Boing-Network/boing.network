/**
 * Reference NFT calldata layout (Boing-defined). See `docs/BOING-REFERENCE-NFT.md`.
 */
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
import { calldataAccountIdWord, calldataFixedWord32, calldataSelectorLastByte, } from './calldata.js';
export const SELECTOR_OWNER_OF = 0x03;
export const SELECTOR_TRANSFER_NFT = 0x04;
export const SELECTOR_SET_METADATA_HASH = 0x05;
export const SELECTOR_MINT_BATCH = 0x06;
/** Bytecode cap; production `GAS_PER_CONTRACT_CALL` (40_000_000) is sized for this `n` with owner+metadata stores. */
export const MAX_REFERENCE_NFT_MINT_BATCH = 500;
/** XOR mask for owner slot — mirrors `REF_NFT_OWNER_STORAGE_XOR` in `reference_nft.rs`. */
export const REF_NFT_OWNER_STORAGE_XOR_HEX = validateHex32('0x424f494e475f5245464e46545f4f574e45523031000000000000000000000000');
/** XOR mask for metadata hash slot — mirrors `REF_NFT_METADATA_STORAGE_XOR` in `reference_nft.rs`. */
export const REF_NFT_METADATA_STORAGE_XOR_HEX = validateHex32('0x424f494e475f5245464e46545f4d455441303100000000000000000000000000');
function xorWords(a, b) {
    const out = new Uint8Array(32);
    for (let i = 0; i < 32; i++)
        out[i] = a[i] ^ b[i];
    return out;
}
function xorStorageKey(tokenIdHex32, xorHex32) {
    const tokenId = hexToBytes(validateHex32(tokenIdHex32));
    const mask = hexToBytes(validateHex32(xorHex32));
    return bytesToHex(xorWords(tokenId, mask));
}
/** `SLOAD` key for reference NFT owner: `token_id ^ REF_NFT_OWNER_STORAGE_XOR`. */
export function referenceNftOwnerStorageKey(tokenIdHex32) {
    return xorStorageKey(tokenIdHex32, REF_NFT_OWNER_STORAGE_XOR_HEX);
}
/** `SLOAD` key for reference NFT metadata hash: `token_id ^ REF_NFT_METADATA_STORAGE_XOR`. */
export function referenceNftMetadataStorageKey(tokenIdHex32) {
    return xorStorageKey(tokenIdHex32, REF_NFT_METADATA_STORAGE_XOR_HEX);
}
/**
 * Encode a sequential token id as a 32-byte word (big-endian u64 in the **low 8 bytes**).
 * Matches common reference collection usage; arbitrary ids may use {@link calldataFixedWord32} instead.
 */
export function referenceNftTokenIdWordFromU64(id) {
    let n = BigInt(id);
    if (n < 0n)
        throw new RangeError('token id must be non-negative');
    const out = new Uint8Array(32);
    for (let i = 31; i >= 24; i--) {
        out[i] = Number(n & 0xffn);
        n >>= 8n;
    }
    return bytesToHex(out);
}
/** 96-byte `owner_of(token_id)` reference calldata. */
export function encodeReferenceOwnerOfCalldata(tokenIdHex32) {
    const out = new Uint8Array(96);
    out.set(calldataSelectorLastByte(SELECTOR_OWNER_OF), 0);
    out.set(calldataFixedWord32(tokenIdHex32), 32);
    return out;
}
/** 96-byte `transfer_nft(to, token_id)` reference calldata. */
export function encodeReferenceTransferNftCalldata(toHexAccount32, tokenIdHex32) {
    const out = new Uint8Array(96);
    out.set(calldataSelectorLastByte(SELECTOR_TRANSFER_NFT), 0);
    out.set(calldataAccountIdWord(toHexAccount32), 32);
    out.set(calldataFixedWord32(tokenIdHex32), 64);
    return out;
}
/** 96-byte `set_metadata_hash(token_id, hash)` reference calldata. */
export function encodeReferenceSetMetadataHashCalldata(tokenIdHex32, metadataHashHex32) {
    const out = new Uint8Array(96);
    out.set(calldataSelectorLastByte(SELECTOR_SET_METADATA_HASH), 0);
    out.set(calldataFixedWord32(tokenIdHex32), 32);
    out.set(calldataFixedWord32(metadataHashHex32), 64);
    return out;
}
export function encodeReferenceOwnerOfCalldataHex(tokenIdHex32) {
    return bytesToHex(encodeReferenceOwnerOfCalldata(tokenIdHex32));
}
export function encodeReferenceTransferNftCalldataHex(toHexAccount32, tokenIdHex32) {
    return bytesToHex(encodeReferenceTransferNftCalldata(toHexAccount32, tokenIdHex32));
}
export function encodeReferenceSetMetadataHashCalldataHex(tokenIdHex32, metadataHashHex32) {
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
export function encodeReferenceMintBatchCalldata(toHexAccount32, tokenIdsHex32, metadataHashesHex32) {
    if (tokenIdsHex32.length !== metadataHashesHex32.length) {
        throw new RangeError(`mint_batch length mismatch: ${tokenIdsHex32.length} token ids vs ${metadataHashesHex32.length} hashes`);
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
        out.set(calldataFixedWord32(tokenIdsHex32[i]), 96 + 32 * i);
        out.set(calldataFixedWord32(metadataHashesHex32[i]), 96 + 32 * n + 32 * i);
    }
    return out;
}
export function encodeReferenceMintBatchCalldataHex(toHexAccount32, tokenIdsHex32, metadataHashesHex32) {
    return bytesToHex(encodeReferenceMintBatchCalldata(toHexAccount32, tokenIdsHex32, metadataHashesHex32));
}
function normalizeCalldataBytes(calldata) {
    if (calldata instanceof Uint8Array)
        return calldata.length >= 32 ? calldata : null;
    const t = String(calldata).trim();
    if (!t)
        return null;
    try {
        const bytes = hexToBytes(t.startsWith('0x') || t.startsWith('0X') ? t : `0x${t}`);
        return bytes.length >= 32 ? bytes : null;
    }
    catch {
        return null;
    }
}
function wordHex(bytes, offset) {
    return bytesToHex(bytes.subarray(offset, offset + 32));
}
function readBeU64Low8(word) {
    for (let i = 0; i < 24; i++) {
        if (word[i] !== 0)
            return null;
    }
    let n = BigInt(0);
    for (let i = 24; i < 32; i++) {
        n = (n << BigInt(8)) | BigInt(word[i]);
    }
    if (n > BigInt(Number.MAX_SAFE_INTEGER))
        return null;
    return Number(n);
}
/** Selector byte is the last byte of the first 32-byte word. */
export function referenceNftCalldataSelector(calldata) {
    const bytes = normalizeCalldataBytes(calldata);
    if (!bytes)
        return null;
    return bytes[31] ?? null;
}
/**
 * If `tokenId` is a sequential reference id (high 24 bytes zero), return that u64.
 * FreshMint-style opaque hash ids return null.
 */
export function tryReferenceNftTokenIdU64(tokenIdHex32) {
    try {
        const bytes = hexToBytes(validateHex32(tokenIdHex32));
        return readBeU64Low8(bytes);
    }
    catch {
        return null;
    }
}
/** Best-effort decode of reference NFT collection calldata. Returns null when unrecognized. */
export function decodeReferenceNftCalldata(calldata) {
    const bytes = normalizeCalldataBytes(calldata);
    if (!bytes)
        return null;
    const selector = bytes[31];
    if (selector === SELECTOR_MINT_BATCH) {
        if (bytes.length < 96)
            return null;
        const n = readBeU64Low8(bytes.subarray(64, 96));
        if (n == null || n < 1 || n > MAX_REFERENCE_NFT_MINT_BATCH)
            return null;
        const expected = 96 + 64 * n;
        if (bytes.length < expected)
            return null;
        for (let i = expected; i < bytes.length; i++) {
            if (bytes[i] !== 0)
                return null;
        }
        const to = wordHex(bytes, 32);
        const tokenIds = [];
        const metadataHashes = [];
        for (let i = 0; i < n; i++) {
            tokenIds.push(wordHex(bytes, 96 + 32 * i));
            metadataHashes.push(wordHex(bytes, 96 + 32 * n + 32 * i));
        }
        return { selector: SELECTOR_MINT_BATCH, to, n, tokenIds, metadataHashes };
    }
    if (bytes.length < 96)
        return null;
    if (selector === SELECTOR_TRANSFER_NFT) {
        return {
            selector: SELECTOR_TRANSFER_NFT,
            to: wordHex(bytes, 32),
            tokenId: wordHex(bytes, 64),
        };
    }
    if (selector === SELECTOR_SET_METADATA_HASH) {
        return {
            selector: SELECTOR_SET_METADATA_HASH,
            tokenId: wordHex(bytes, 32),
            metadataHash: wordHex(bytes, 64),
        };
    }
    if (selector === SELECTOR_OWNER_OF) {
        return {
            selector: SELECTOR_OWNER_OF,
            tokenId: wordHex(bytes, 32),
        };
    }
    return null;
}
/** Collect opaque token-id words from a decoded reference NFT call. */
export function tokenIdsFromDecodedReferenceNftCall(decoded) {
    switch (decoded.selector) {
        case SELECTOR_MINT_BATCH:
            return [...decoded.tokenIds];
        case SELECTOR_TRANSFER_NFT:
        case SELECTOR_SET_METADATA_HASH:
        case SELECTOR_OWNER_OF:
            return [decoded.tokenId];
        default:
            return [];
    }
}
