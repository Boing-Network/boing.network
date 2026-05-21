/**
 * Reference NFT calldata layout (Boing-defined). See `docs/BOING-REFERENCE-NFT.md`.
 */
export declare const SELECTOR_OWNER_OF = 3;
export declare const SELECTOR_TRANSFER_NFT = 4;
export declare const SELECTOR_SET_METADATA_HASH = 5;
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
//# sourceMappingURL=referenceNft.d.ts.map