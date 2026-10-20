/**
 * L1 media blobs — content-addressed PutBlob / BlobAnnounce helpers (not the interface SSTORE store).
 * @see docs/BOING-L1-MEDIA-BLOBS.md
 */
import type { TransactionPayloadInput } from './bincode.js';
/** Live consensus hard cap (16 MiB). Still not multi-minute video-scale. */
export declare const MAX_MEDIA_BLOB_BYTES: number;
/** Alias: Phase 3 target is now the live cap. */
export declare const PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES: number;
/** Prefer BlobAnnounce + gossip above this size (PutBlob still allows up to max). */
export declare const MAX_MEDIA_BLOB_INLINE_PREFER_BYTES: number;
export declare const MAX_MEDIA_BLOB_INLINE_BYTES: number;
/** DAS / erasure chunk size. */
export declare const MEDIA_BLOB_DAS_CHUNK_BYTES: number;
export declare const MAX_MEDIA_BLOB_CONTENT_TYPE_LEN = 64;
export declare const MAX_MEDIA_BLOB_TTL_BLOCKS = 1000000n;
export declare const MEDIA_BLOB_GAS_BASE = 21000n;
export declare const MEDIA_BLOB_GAS_PER_WORD = 16n;
export declare const MEDIA_BLOB_FOREVER_MULTIPLIER = 10n;
export declare const MEDIA_BLOB_URI_PREFIX = "boing://blob/";
export declare const MEDIA_BLOBS_GOSSIP_TOPIC = "boing/media-blobs";
/** Ceil chunk count for DAS / XOR erasure stubs. */
export declare function mediaBlobDasChunkCount(size: number): number;
/** Forever pin-rent units (KiB × blocks) for off-chain markets; on-chain uses `blob_forever_pin_fee_per_kib`. */
export declare function mediaBlobPinRentKibBlocks(size: number, ttlBlocks: bigint): bigint;
/** Binary merkle root over 32-byte chunk hashes (matches Rust `media_blob_chunks_merkle_root`). */
export declare function mediaBlobChunksMerkleRoot(leaves: Uint8Array[]): Uint8Array;
export type MediaBlobErasureBundle = {
    blobId: Uint8Array;
    originalLen: number;
    dataChunks: Uint8Array[];
    parityChunk: Uint8Array;
    chunkHashes: Uint8Array[];
    chunksRoot: Uint8Array;
};
/** Split into DAS-sized chunks + XOR parity (recover one missing data chunk). */
export declare function chunkMediaBlob(data: Uint8Array): MediaBlobErasureBundle;
/** Recover bytes when ≤1 data chunk is missing (XOR) or all present. */
export declare function recoverMediaBlobXor(blobId: Uint8Array, originalLen: number, dataChunks: Array<Uint8Array | null>, parity: Uint8Array): Uint8Array;
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