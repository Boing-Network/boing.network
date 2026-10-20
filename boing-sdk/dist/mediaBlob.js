/**
 * L1 media blobs — content-addressed PutBlob / BlobAnnounce helpers (not the interface SSTORE store).
 * @see docs/BOING-L1-MEDIA-BLOBS.md
 */
import { blake3 } from '@noble/hashes/blake3';
import { bytesToHex, ensureHex, hexToBytes } from './hex.js';
/** Live consensus hard cap (1 MiB). Not video-scale. */
export const MAX_MEDIA_BLOB_BYTES = 1024 * 1024;
/** Phase 3 target raised path once DAS/chunking ships — not consensus-live. */
export const PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES = 16 * 1024 * 1024;
/** Suggested DAS chunk size (scaffold). */
export const MEDIA_BLOB_DAS_CHUNK_BYTES = 64 * 1024;
export const MAX_MEDIA_BLOB_INLINE_BYTES = MAX_MEDIA_BLOB_BYTES;
export const MAX_MEDIA_BLOB_CONTENT_TYPE_LEN = 64;
export const MAX_MEDIA_BLOB_TTL_BLOCKS = 1000000n;
export const MEDIA_BLOB_GAS_BASE = 21000n;
export const MEDIA_BLOB_GAS_PER_WORD = 16n;
export const MEDIA_BLOB_FOREVER_MULTIPLIER = 10n;
export const MEDIA_BLOB_URI_PREFIX = 'boing://blob/';
export const MEDIA_BLOBS_GOSSIP_TOPIC = 'boing/media-blobs';
/** Ceil chunk count for Phase 3 DAS scaffold. */
export function mediaBlobDasChunkCount(size) {
    if (size <= 0)
        return 0;
    return Math.ceil(size / MEDIA_BLOB_DAS_CHUNK_BYTES);
}
/** Scaffold Forever pin-rent units (KiB × blocks) — not charged on-chain yet. */
export function mediaBlobPinRentKibBlocks(size, ttlBlocks) {
    const kib = BigInt(Math.max(1, Math.ceil(size / 1024)));
    return kib * ttlBlocks;
}
/** `BlobId = BLAKE3(data)` (32 bytes). */
export function blobIdOf(data) {
    return blake3(data);
}
/** Hex without `0x` (64 chars). */
export function blobIdHex(id) {
    return bytesToHex(id).replace(/^0x/i, '');
}
export function mediaBlobUri(id) {
    return `${MEDIA_BLOB_URI_PREFIX}0x${blobIdHex(id)}`;
}
/** Parse `boing://blob/0x…`, `0x…`, or bare 64-hex. */
export function parseMediaBlobId(s) {
    const t = s.trim();
    let hex = t;
    if (t.startsWith(MEDIA_BLOB_URI_PREFIX)) {
        hex = t.slice(MEDIA_BLOB_URI_PREFIX.length);
    }
    try {
        const b = hexToBytes(ensureHex(hex));
        return b.length === 32 ? b : null;
    }
    catch {
        return null;
    }
}
export function mediaBlobGasRaw(size) {
    const words = BigInt(Math.max(1, Math.ceil(size / 32)));
    return MEDIA_BLOB_GAS_BASE + words * MEDIA_BLOB_GAS_PER_WORD;
}
export function mediaBlobGas(size, retention) {
    const mult = retention.kind === 'forever' ? MEDIA_BLOB_FOREVER_MULTIPLIER : 1n;
    return mediaBlobGasRaw(size) * mult;
}
export function validatePutBlob(retention, contentType, data) {
    if (data.length > MAX_MEDIA_BLOB_BYTES) {
        throw new Error(`media blob exceeds MAX_MEDIA_BLOB_BYTES (${MAX_MEDIA_BLOB_BYTES})`);
    }
    if (contentType.length > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN) {
        throw new Error(`contentType exceeds ${MAX_MEDIA_BLOB_CONTENT_TYPE_LEN} bytes`);
    }
    if (retention.kind === 'prunable') {
        if (retention.ttlBlocks < 1n)
            throw new Error('prunable ttlBlocks must be >= 1');
        if (retention.ttlBlocks > MAX_MEDIA_BLOB_TTL_BLOCKS) {
            throw new Error(`prunable ttlBlocks exceeds max ${MAX_MEDIA_BLOB_TTL_BLOCKS}`);
        }
    }
    return blobIdOf(data);
}
export function validateBlobAnnounce(blobId, size, retention, contentType) {
    if (blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (size <= 0)
        throw new Error('announce size must be > 0');
    if (size > MAX_MEDIA_BLOB_BYTES) {
        throw new Error(`announce size exceeds MAX_MEDIA_BLOB_BYTES (${MAX_MEDIA_BLOB_BYTES})`);
    }
    if (contentType.length > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN) {
        throw new Error(`contentType exceeds ${MAX_MEDIA_BLOB_CONTENT_TYPE_LEN} bytes`);
    }
    if (retention.kind === 'prunable') {
        if (retention.ttlBlocks < 1n)
            throw new Error('prunable ttlBlocks must be >= 1');
        if (retention.ttlBlocks > MAX_MEDIA_BLOB_TTL_BLOCKS) {
            throw new Error(`prunable ttlBlocks exceeds max ${MAX_MEDIA_BLOB_TTL_BLOCKS}`);
        }
    }
}
/** Build a PutBlob payload for `encodeTransaction` / signing. */
export function putBlobPayload(input) {
    const contentType = input.contentType ?? '';
    validatePutBlob(input.retention, contentType, input.data);
    return {
        kind: 'putBlob',
        retention: input.retention,
        contentType,
        data: input.data,
    };
}
/** Build a BlobAnnounce payload (commitment; bytes via P2P / sidecar). */
export function blobAnnouncePayload(input) {
    const contentType = input.contentType ?? '';
    validateBlobAnnounce(input.blobId, input.size, input.retention, contentType);
    return {
        kind: 'blobAnnounce',
        blobId: input.blobId,
        size: input.size,
        retention: input.retention,
        contentType,
    };
}
/**
 * NFT binding: store BlobId as `metadata_hash`, or put `boing://blob/0x{id}` in off-chain JSON
 * and hash that JSON. Prefer JSON + URI for marketplaces (name / attributes alongside media).
 */
export function metadataHashAsBlobId(blobId) {
    if (blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    return blobId;
}
/** Build token metadata JSON that points `image` at an on-chain blob. */
export function tokenMetadataJsonWithBlobImage(input) {
    const imageUri = mediaBlobUri(input.blobId);
    const doc = {
        name: input.name,
        description: input.description ?? '',
        image: imageUri,
    };
    if (input.animationBlobId) {
        doc.animation_url = mediaBlobUri(input.animationBlobId);
    }
    if (input.attributes?.length) {
        doc.attributes = input.attributes;
    }
    const json = JSON.stringify(doc);
    const metadataHash = blake3(new TextEncoder().encode(json));
    return { json, metadataHash, imageUri };
}
