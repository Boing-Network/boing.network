import { describe, expect, it } from 'vitest';
import { encodeTransactionPayload } from '../src/bincode.js';
import {
  blobAnnouncePayload,
  blobIdOf,
  MAX_MEDIA_BLOB_BYTES,
  MEDIA_BLOB_DAS_CHUNK_BYTES,
  mediaBlobDasChunkCount,
  mediaBlobGas,
  mediaBlobPinRentKibBlocks,
  mediaBlobUri,
  parseMediaBlobId,
  PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES,
  putBlobPayload,
  tokenMetadataJsonWithBlobImage,
  validatePutBlob,
} from '../src/mediaBlob.js';
import {
  blobIdHexFromBoingUri,
  metadataHashWordToFetchUrls,
} from '../src/metadataMedia.js';

describe('mediaBlob', () => {
  it('content-addresses and URIs round-trip', () => {
    const data = new TextEncoder().encode('hello media');
    const id = blobIdOf(data);
    expect(id.length).toBe(32);
    const uri = mediaBlobUri(id);
    expect(uri.startsWith('boing://blob/0x')).toBe(true);
    expect(parseMediaBlobId(uri)).toEqual(id);
  });

  it('Phase 3 DAS / pin-rent scaffolds (live cap still 1 MiB)', () => {
    expect(PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES).toBe(16 * 1024 * 1024);
    expect(mediaBlobDasChunkCount(0)).toBe(0);
    expect(mediaBlobDasChunkCount(1)).toBe(1);
    expect(mediaBlobDasChunkCount(MEDIA_BLOB_DAS_CHUNK_BYTES + 1)).toBe(2);
    expect(mediaBlobPinRentKibBlocks(1024, 10n)).toBe(10n);
  });

  it('Phase 2 MiB cap and forever gas', () => {
    expect(MAX_MEDIA_BLOB_BYTES).toBe(1024 * 1024);
    const data = new Uint8Array(64);
    validatePutBlob({ kind: 'prunable', ttlBlocks: 100n }, 'image/png', data);
    expect(mediaBlobGas(64, { kind: 'forever' })).toBe(
      mediaBlobGas(64, { kind: 'prunable', ttlBlocks: 1n }) * 10n,
    );
    expect(() =>
      validatePutBlob({ kind: 'forever' }, '', new Uint8Array(MAX_MEDIA_BLOB_BYTES + 1)),
    ).toThrow(/MAX_MEDIA_BLOB_BYTES/);
  });

  it('encodes PutBlob (21) and BlobAnnounce (22)', () => {
    const data = new Uint8Array([1, 2, 3, 4]);
    const put = encodeTransactionPayload(
      putBlobPayload({ retention: { kind: 'forever' }, contentType: 'image/gif', data }),
    );
    expect(put[0]).toBe(21);
    const id = blobIdOf(data);
    const ann = encodeTransactionPayload(
      blobAnnouncePayload({
        blobId: id,
        size: data.length,
        retention: { kind: 'prunable', ttlBlocks: 10n },
        contentType: 'image/gif',
      }),
    );
    expect(ann[0]).toBe(22);
  });

  it('NFT binding helpers expose boing://blob URIs', () => {
    const id = blobIdOf(new Uint8Array([9, 9, 9]));
    const { imageUri, metadataHash } = tokenMetadataJsonWithBlobImage({
      name: 'Neon',
      blobId: id,
    });
    expect(imageUri).toBe(mediaBlobUri(id));
    expect(metadataHash.length).toBe(32);
    const hex = Array.from(id, (b) => b.toString(16).padStart(2, '0')).join('');
    const urls = metadataHashWordToFetchUrls(hex);
    expect(urls.some((u) => u.startsWith('boing://blob/'))).toBe(true);
    expect(blobIdHexFromBoingUri(imageUri)).toBe(`0x${hex}`);
  });
});
