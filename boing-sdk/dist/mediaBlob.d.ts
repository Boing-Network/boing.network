/**
 * L1 media blobs — content-addressed PutBlob helpers (not the interface SSTORE blob store).
 * @see docs/BOING-L1-MEDIA-BLOBS.md
 */
import type { TransactionPayloadInput } from './bincode.js';
/** MVP max payload (matches `boing_primitives::MAX_MEDIA_BLOB_BYTES`). */
export declare const MAX_MEDIA_BLOB_BYTES: number;
export declare const MAX_MEDIA_BLOB_CONTENT_TYPE_LEN = 64;
export declare const MAX_MEDIA_BLOB_TTL_BLOCKS = 1000000n;
export declare const MEDIA_BLOB_GAS_BASE = 21000n;
export declare const MEDIA_BLOB_GAS_PER_WORD = 16n;
export declare const MEDIA_BLOB_FOREVER_MULTIPLIER = 10n;
export declare const MEDIA_BLOB_URI_PREFIX = "boing://blob/";
export type BlobRetentionInput = {
    kind: 'forever';
} | {
    kind: 'prunable';
    ttlBlocks: bigint;
};
/** `BlobId = BLAKE3(data)` (32 bytes). */
export declare function blobIdOf(data: Uint8Array): Uint8Array;
/** Hex without `0x` (64 chars). */
export declare function blobIdHex(id: Uint8Array): string;
export declare function mediaBlobUri(id: Uint8Array): string;
/** Parse `boing://blob/0x…`, `0x…`, or bare 64-hex. */
export declare function parseMediaBlobId(s: string): Uint8Array | null;
export declare function mediaBlobGasRaw(size: number): bigint;
export declare function mediaBlobGas(size: number, retention: BlobRetentionInput): bigint;
export declare function validatePutBlob(retention: BlobRetentionInput, contentType: string, data: Uint8Array): Uint8Array;
/** Build a PutBlob payload for `encodeTransaction` / signing. */
export declare function putBlobPayload(input: {
    retention: BlobRetentionInput;
    contentType?: string;
    data: Uint8Array;
}): TransactionPayloadInput;
/**
 * NFT binding convention: store BlobId as `metadata_hash`, or put
 * `boing://blob/0x{id}` in off-chain JSON and hash that JSON.
 */
export declare function metadataHashAsBlobId(blobId: Uint8Array): Uint8Array;
//# sourceMappingURL=mediaBlob.d.ts.map