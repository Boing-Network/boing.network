/**
 * Reference NFT calldata layout (Boing-defined). See `docs/BOING-REFERENCE-NFT.md`.
 */
import { bytesToHex } from './hex.js';
import { calldataAccountIdWord, calldataFixedWord32, calldataSelectorLastByte, } from './calldata.js';
export const SELECTOR_OWNER_OF = 0x03;
export const SELECTOR_TRANSFER_NFT = 0x04;
export const SELECTOR_SET_METADATA_HASH = 0x05;
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
