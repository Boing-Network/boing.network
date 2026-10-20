/**
 * L1 media blobs — content-addressed PutBlob / BlobAnnounce helpers (not the interface SSTORE store).
 * @see docs/BOING-L1-MEDIA-BLOBS.md
 */
import type { TransactionPayloadInput } from './bincode.js';
/** Live consensus hard cap (1 MiB). Not video-scale. */
export declare const MAX_MEDIA_BLOB_BYTES: number;
/** Phase 3 target raised path once DAS/chunking ships — not consensus-live. */
export declare const PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES: number;
/** Suggested DAS chunk size (scaffold). */
export declare const MEDIA_BLOB_DAS_CHUNK_BYTES: number;
export declare const MAX_MEDIA_BLOB_INLINE_BYTES: number;
export declare const MAX_MEDIA_BLOB_CONTENT_TYPE_LEN = 64;
export declare const MAX_MEDIA_BLOB_TTL_BLOCKS = 1000000n;
export declare const MEDIA_BLOB_GAS_BASE = 21000n;
export declare const MEDIA_BLOB_GAS_PER_WORD = 16n;
export declare const MEDIA_BLOB_FOREVER_MULTIPLIER = 10n;
export declare const MEDIA_BLOB_URI_PREFIX = "boing://blob/";
export declare const MEDIA_BLOBS_GOSSIP_TOPIC = "boing/media-blobs";
/** Ceil chunk count for Phase 3 DAS scaffold. */
export declare function mediaBlobDasChunkCount(size: number): number;
/** Scaffold Forever pin-rent units (KiB × blocks) — not charged on-chain yet. */
export declare function mediaBlobPinRentKibBlocks(size: number, ttlBlocks: bigint): bigint;
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
export declare function validateBlobAnnounce(blobId: Uint8Array, size: number, retention: BlobRetentionInput, contentType: string): void;
/** Build a PutBlob payload for `encodeTransaction` / signing. */
export declare function putBlobPayload(input: {
    retention: BlobRetentionInput;
    contentType?: string;
    data: Uint8Array;
}): TransactionPayloadInput;
/** Build a BlobAnnounce payload (commitment; bytes via P2P / sidecar). */
export declare function blobAnnouncePayload(input: {
    blobId: Uint8Array;
    size: number;
    retention: BlobRetentionInput;
    contentType?: string;
}): TransactionPayloadInput;
/**
 * NFT binding: store BlobId as `metadata_hash`, or put `boing://blob/0x{id}` in off-chain JSON
 * and hash that JSON. Prefer JSON + URI for marketplaces (name / attributes alongside media).
 */
export declare function metadataHashAsBlobId(blobId: Uint8Array): Uint8Array;
/** Build token metadata JSON that points `image` at an on-chain blob. */
export declare function tokenMetadataJsonWithBlobImage(input: {
    name: string;
    description?: string;
    blobId: Uint8Array;
    attributes?: Array<{
        trait_type: string;
        value: string | number;
    }>;
    animationBlobId?: Uint8Array;
}): {
    json: string;
    metadataHash: Uint8Array;
    imageUri: string;
};
//# sourceMappingURL=mediaBlob.d.ts.map