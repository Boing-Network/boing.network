import { describe, expect, it } from 'vitest';
import { encodeTransactionPayload } from '../src/bincode.js';
import {
  blobAnnouncePayload,
  blobIdOf,
  chunkMediaBlob,
  dasLightClientSampleIndices,
  dasLightClientSeed,
  matchSampleOffer,
  manifestFromSegmentBytes,
  MAX_MEDIA_BLOB_BYTES,
  MAX_MEDIA_BLOB_INLINE_PREFER_BYTES,
  MEDIA_BLOB_DAS_CHUNK_BYTES,
  mediaBlobDasChunkCount,
  mediaBlobGas,
  mediaBlobPinRentKibBlocks,
  mediaBlobUri,
  parseMediaBlobId,
  PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES,
  PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES,
  mediaBlobMaxBytesAt,
  payBlobSamplePayload,
  putBlobPayload,
  recoverMediaBlobXor,
  registerBlobSampleOfferPayload,
  renewMediaBlobPinPayload,
  segmentManifestId,
  settleBlobSampleMatchPayload,
  splitIntoLiveCapSegments,
  suggestedVideoSegmentCount,
  tokenMetadataJsonWithBlobImage,
  validatePutBlob,
  validateSegmentManifest,
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

  it('Phase 4 16 MiB cap + XOR chunk recover', () => {
    expect(MAX_MEDIA_BLOB_BYTES).toBe(16 * 1024 * 1024);
    expect(PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES).toBe(MAX_MEDIA_BLOB_BYTES);
    expect(MAX_MEDIA_BLOB_INLINE_PREFER_BYTES).toBe(1024 * 1024);
    expect(mediaBlobDasChunkCount(0)).toBe(0);
    expect(mediaBlobDasChunkCount(MEDIA_BLOB_DAS_CHUNK_BYTES + 1)).toBe(2);
    expect(mediaBlobPinRentKibBlocks(1024, 10n)).toBe(10n);
    const data = new Uint8Array(MEDIA_BLOB_DAS_CHUNK_BYTES + 50);
    data[0] = 7;
    data[MEDIA_BLOB_DAS_CHUNK_BYTES] = 9;
    const bundle = chunkMediaBlob(data);
    const parts: Array<Uint8Array | null> = [...bundle.dataChunks];
    parts[1] = null;
    const recovered = recoverMediaBlobXor(
      bundle.blobId,
      bundle.originalLen,
      parts,
      bundle.parityChunk,
    );
    expect(recovered).toEqual(data);
  });

  it('forever gas multiplier', () => {
    expect(MAX_MEDIA_BLOB_BYTES).toBe(16 * 1024 * 1024);
    const data = new Uint8Array(64);
    validatePutBlob({ kind: 'prunable', ttlBlocks: 100n }, 'image/png', data);
    expect(mediaBlobGas(64, { kind: 'forever' })).toBe(
      mediaBlobGas(64, { kind: 'prunable', ttlBlocks: 1n }) * 10n,
    );
    expect(() =>
      validatePutBlob({ kind: 'forever' }, '', new Uint8Array(MAX_MEDIA_BLOB_BYTES + 1)),
    ).toThrow(/MAX_MEDIA_BLOB_BYTES/);
  });

  it('Phase 6: segments (live-cap), sample match expiry, LC seed', () => {
    expect(suggestedVideoSegmentCount(BigInt(16 * 1024 * 1024 * 3 + 1))).toBe(4);
    expect(PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES).toBe(256 * 1024 * 1024);
    const data = new Uint8Array(MAX_MEDIA_BLOB_BYTES + 100);
    data.fill(9);
    const segs = splitIntoLiveCapSegments(data);
    expect(segs.length).toBe(2);
    expect(segs.every((s) => s.length <= MAX_MEDIA_BLOB_BYTES)).toBe(true);
    const manifest = manifestFromSegmentBytes('video/mp4', segs);
    expect(manifest.totalBytes).toBe(BigInt(data.length));
    expect(segmentManifestId(manifest)).toHaveLength(32);
    const matched = matchSampleOffer(
      {
        blobId: new Uint8Array(32),
        chunkIndex: 0,
        chunkCount: 4,
        providerHex: '0x' + '11'.repeat(32),
        askFee: 5n,
        expiresAtHeight: 100n,
      },
      5n,
      50n,
    );
    expect(matched.paidFee).toBe(5n);
    expect(() =>
      matchSampleOffer(
        {
          blobId: new Uint8Array(32),
          chunkIndex: 0,
          chunkCount: 4,
          providerHex: '0x',
          askFee: 5n,
          expiresAtHeight: 10n,
        },
        5n,
        11n,
      ),
    ).toThrow(/expired/);
    validateSegmentManifest({
      contentType: 'video/mp4',
      totalBytes: 32n * 1024n * 1024n,
      segmentBlobIds: [new Uint8Array(32), new Uint8Array(32)],
    });
    const seed = dasLightClientSeed(new Uint8Array(32), 42n);
    const idxs = dasLightClientSampleIndices(130_000, 3, seed);
    expect(idxs.length).toBeGreaterThan(0);
  });

  it('encodes RenewMediaBlobPin (23) and PayBlobSample (24)', () => {
    const id = blobIdOf(new Uint8Array([1, 2, 3]));
    const renew = encodeTransactionPayload(
      renewMediaBlobPinPayload({ blobId: id, size: 3, extendBlocks: 100n }),
    );
    expect(renew[0]).toBe(23);
    const sample = encodeTransactionPayload(
      payBlobSamplePayload({ blobId: id, chunkIndex: 0, chunkCount: 1 }),
    );
    expect(sample[0]).toBe(24);
  });

  it('encodes Phase 7 RegisterBlobSampleOffer (25) and SettleBlobSampleMatch (26)', () => {
    const id = blobIdOf(new Uint8Array([1, 2, 3]));
    const provider = new Uint8Array(32);
    provider[0] = 9;
    const reg = encodeTransactionPayload(
      registerBlobSampleOfferPayload({
        blobId: id,
        chunkIndex: 0,
        chunkCount: 1,
        askFee: 7n,
      }),
    );
    expect(reg[0]).toBe(25);
    const settle = encodeTransactionPayload(
      settleBlobSampleMatchPayload({
        blobId: id,
        chunkIndex: 0,
        chunkCount: 1,
        provider,
      }),
    );
    expect(settle[0]).toBe(26);
    expect(mediaBlobMaxBytesAt(false)).toBe(MAX_MEDIA_BLOB_BYTES);
    expect(mediaBlobMaxBytesAt(true)).toBe(256 * 1024 * 1024);
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
