/**
 * L1 media blobs — content-addressed PutBlob helpers (not the interface SSTORE blob store).
 * @see docs/BOING-L1-MEDIA-BLOBS.md
 */
import { blake3 } from '@noble/hashes/blake3';
import { bytesToHex, ensureHex, hexToBytes } from './hex.js';
/** MVP max payload (matches `boing_primitives::MAX_MEDIA_BLOB_BYTES`). */
export const MAX_MEDIA_BLOB_BYTES = 256 * 1024;
export const MAX_MEDIA_BLOB_CONTENT_TYPE_LEN = 64;
export const MAX_MEDIA_BLOB_TTL_BLOCKS = 1000000n;
export const MEDIA_BLOB_GAS_BASE = 21000n;
export const MEDIA_BLOB_GAS_PER_WORD = 16n;
export const MEDIA_BLOB_FOREVER_MULTIPLIER = 10n;
export const MEDIA_BLOB_URI_PREFIX = 'boing://blob/';
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
/**
 * NFT binding convention: store BlobId as `metadata_hash`, or put
 * `boing://blob/0x{id}` in off-chain JSON and hash that JSON.
 */
export function metadataHashAsBlobId(blobId) {
    if (blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    return blobId;
}
