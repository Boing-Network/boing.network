/**
 * Reference NFT calldata layout (Boing-defined). See `docs/BOING-REFERENCE-NFT.md`.
 */
export declare const SELECTOR_OWNER_OF = 3;
export declare const SELECTOR_TRANSFER_NFT = 4;
export declare const SELECTOR_SET_METADATA_HASH = 5;
/** 96-byte `owner_of(token_id)` reference calldata. */
export declare function encodeReferenceOwnerOfCalldata(tokenIdHex32: string): Uint8Array;
/** 96-byte `transfer_nft(to, token_id)` reference calldata. */
export declare function encodeReferenceTransferNftCalldata(toHexAccount32: string, tokenIdHex32: string): Uint8Array;
/** 96-byte `set_metadata_hash(token_id, hash)` reference calldata. */
export declare function encodeReferenceSetMetadataHashCalldata(tokenIdHex32: string, metadataHashHex32: string): Uint8Array;
export declare function encodeReferenceOwnerOfCalldataHex(tokenIdHex32: string): string;
//# sourceMappingURL=referenceNft.d.ts.map