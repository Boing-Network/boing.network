/**
 * Reference NFT calldata layout (Boing-defined). See `docs/BOING-REFERENCE-NFT.md`.
 */
export declare const SELECTOR_OWNER_OF = 3;
export declare const SELECTOR_TRANSFER_NFT = 4;
export declare const SELECTOR_SET_METADATA_HASH = 5;
export declare const SELECTOR_MINT_BATCH = 6;
/** Bytecode cap; production `GAS_PER_CONTRACT_CALL` (40_000_000) is sized for this `n` with owner+metadata stores. */
export declare const MAX_REFERENCE_NFT_MINT_BATCH = 500;
/** XOR mask for owner slot — mirrors `REF_NFT_OWNER_STORAGE_XOR` in `reference_nft.rs`. */
export declare const REF_NFT_OWNER_STORAGE_XOR_HEX: string;
/** XOR mask for metadata hash slot — mirrors `REF_NFT_METADATA_STORAGE_XOR` in `reference_nft.rs`. */
export declare const REF_NFT_METADATA_STORAGE_XOR_HEX: string;
/** `SLOAD` key for reference NFT owner: `token_id ^ REF_NFT_OWNER_STORAGE_XOR`. */
export declare function referenceNftOwnerStorageKey(tokenIdHex32: string): string;
/** `SLOAD` key for reference NFT metadata hash: `token_id ^ REF_NFT_METADATA_STORAGE_XOR`. */
export declare function referenceNftMetadataStorageKey(tokenIdHex32: string): string;
/**
 * Encode a sequential token id as a 32-byte word (big-endian u64 in the **low 8 bytes**).
 * Matches common reference collection usage; arbitrary ids may use {@link calldataFixedWord32} instead.
 */
export declare function referenceNftTokenIdWordFromU64(id: bigint | number): string;
/** 96-byte `owner_of(token_id)` reference calldata. */
export declare function encodeReferenceOwnerOfCalldata(tokenIdHex32: string): Uint8Array;
/** 96-byte `transfer_nft(to, token_id)` reference calldata. */
export declare function encodeReferenceTransferNftCalldata(toHexAccount32: string, tokenIdHex32: string): Uint8Array;
/** 96-byte `set_metadata_hash(token_id, hash)` reference calldata. */
export declare function encodeReferenceSetMetadataHashCalldata(tokenIdHex32: string, metadataHashHex32: string): Uint8Array;
export declare function encodeReferenceOwnerOfCalldataHex(tokenIdHex32: string): string;
export declare function encodeReferenceTransferNftCalldataHex(toHexAccount32: string, tokenIdHex32: string): string;
export declare function encodeReferenceSetMetadataHashCalldataHex(tokenIdHex32: string, metadataHashHex32: string): string;
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
export declare function encodeReferenceMintBatchCalldata(toHexAccount32: string, tokenIdsHex32: readonly string[], metadataHashesHex32: readonly string[]): Uint8Array;
export declare function encodeReferenceMintBatchCalldataHex(toHexAccount32: string, tokenIdsHex32: readonly string[], metadataHashesHex32: readonly string[]): string;
//# sourceMappingURL=referenceNft.d.ts.map