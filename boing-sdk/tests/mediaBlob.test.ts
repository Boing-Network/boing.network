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
  quoteBlobSampleSettle,
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
  buildDasSuccinctProofStub,
  verifyDasSuccinctProofStub,
  PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
  cancelBlobSampleOfferPayload,
  blobSampleAskEscrowAccount,
  challengeBlobSampleServePayload,
  respondBlobSampleServePayload,
  phase10DasSuccinctProofPlan,
  phase11DasSuccinctProofPlan,
  phase12DasSuccinctProofPlan,
  phase13DasSuccinctProofPlan,
  PHASE11_SERVE_CHALLENGE_BOND,
  blobSampleServeChallengeBondEscrowAccount,
  phase29EncodeSignerBitset,
  phase29DecodeSignerBitset,
  PHASE29_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX,
  PHASE29_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED,
  PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED,
  phase29DasSuccinctProofPlan,
  phase29AdmitBlsAggregateResolveTx,
  phase30IdentitiesFromSelfContained,
  PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED,
  PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED,
  phase30DasSuccinctProofPlan,
  phase30AdmitBlsAggregateResolveTx,
  encodeResolveSlashAppealBlsAggregate,
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

  it('encodes Phase 9 CancelBlobSampleOffer (27) and escrow account', () => {
    const id = blobIdOf(new Uint8Array([4, 5, 6]));
    const cancel = encodeTransactionPayload(
      cancelBlobSampleOfferPayload({ blobId: id, chunkIndex: 0 }),
    );
    expect(cancel[0]).toBe(27);
    expect(blobSampleAskEscrowAccount().length).toBe(32);
  });

  it('encodes Phase 10 Challenge/Respond serve (28/29)', () => {
    const id = blobIdOf(new Uint8Array([7, 8, 9]));
    const provider = new Uint8Array(32);
    provider[0] = 2;
    const challenger = new Uint8Array(32);
    challenger[0] = 3;
    const ch = encodeTransactionPayload(
      challengeBlobSampleServePayload({
        blobId: id,
        chunkIndex: 0,
        chunkCount: 1,
        provider,
      }),
    );
    expect(ch[0]).toBe(28);
    const resp = encodeTransactionPayload(
      respondBlobSampleServePayload({
        blobId: id,
        chunkIndex: 0,
        chunkCount: 1,
        challenger,
        chunkHash: new Uint8Array(32),
        chunksRoot: new Uint8Array(32),
        siblings: [],
      }),
    );
    expect(resp[0]).toBe(29);
    expect(phase10DasSuccinctProofPlan().length).toBeGreaterThan(0);
    expect(phase11DasSuccinctProofPlan().length).toBeGreaterThan(
      phase10DasSuccinctProofPlan().length,
    );
    expect(phase12DasSuccinctProofPlan().length).toBeGreaterThan(
      phase11DasSuccinctProofPlan().length,
    );
    expect(phase13DasSuccinctProofPlan().length).toBeGreaterThan(
      phase12DasSuccinctProofPlan().length,
    );
    expect(PHASE11_SERVE_CHALLENGE_BOND).toBe(10n ** 18n);
    expect(blobSampleServeChallengeBondEscrowAccount().length).toBe(32);
  });

  it('Phase 8 settle quote and succinct stub envelope', () => {
    const id = blobIdOf(new Uint8Array([9, 8, 7]));
    const provider = new Uint8Array(32);
    provider[0] = 1;
    const q = quoteBlobSampleSettle({
      blobId: id,
      chunkIndex: 0,
      chunkCount: 1,
      provider,
      askFee: 50n,
      protocolSampleFee: 10n,
    });
    expect(q.total).toBe(60n);
    expect(PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES).toBe(256 * 1024 * 1024);
    const proof = buildDasSuccinctProofStub({
      blobId: id,
      chunksRoot: new Uint8Array(32),
      seedHeight: 7n,
      samples: [{ chunkIndex: 0, chunkHash: new Uint8Array(32) }],
    });
    expect(proof.kind).toBe(0);
    expect(verifyDasSuccinctProofStub(proof, proof)).toBe(true);
    const bad = { ...proof, commitment: new Uint8Array(32) };
    bad.commitment[0] = 1;
    expect(verifyDasSuccinctProofStub(bad, proof)).toBe(false);
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

  it('Phase 29 signer bitset round-trips (ascending pin index)', () => {
    const pinned = [new Uint8Array(32).fill(1), new Uint8Array(32).fill(2), new Uint8Array(32).fill(3)];
    const bits = phase29EncodeSignerBitset(pinned, [pinned[2]!, pinned[0]!]);
    const decoded = phase29DecodeSignerBitset(pinned, bits);
    expect(decoded).toHaveLength(2);
    expect(Array.from(decoded[0]!)).toEqual(Array.from(pinned[0]!));
    expect(Array.from(decoded[1]!)).toEqual(Array.from(pinned[2]!));
    expect(PHASE29_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX).toBe(33);
    expect(PHASE29_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED).toBe(true);
    expect(PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED).toBe(false);
    expect(phase29DasSuccinctProofPlan().some((s) => s.includes('Phase 29'))).toBe(true);
    expect(() => phase29AdmitBlsAggregateResolveTx()).toThrow(/CONSENSUS_WIRED=false/);
  });

  it('Phase 30 self-contained identities + encode tx 33', () => {
    const pinned = [new Uint8Array(32).fill(1), new Uint8Array(32).fill(2)];
    const bits = phase29EncodeSignerBitset(pinned, [pinned[0]!, pinned[1]!]);
    const pk1 = new Uint8Array(48).fill(0xaa);
    const pk2 = new Uint8Array(48).fill(0xbb);
    const ids = phase30IdentitiesFromSelfContained({
      pinned,
      bitset: bits,
      pubkeys: [pk1, pk2],
    });
    expect(ids).toHaveLength(2);
    expect(PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED).toBe(true);
    expect(PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED).toBe(false);
    expect(phase30DasSuccinctProofPlan().some((s) => s.includes('self-contained'))).toBe(true);
    expect(() => phase30AdmitBlsAggregateResolveTx()).toThrow(/CONSENSUS_WIRED=false/);
    const enc = encodeTransactionPayload(
      encodeResolveSlashAppealBlsAggregate({
        appealId: 1n,
        approved: true,
        signerBitset: bits,
        aggregateSignature: new Uint8Array(96).fill(7),
        signerBlsPubkeys: [pk1, pk2],
      }),
    );
    expect(enc[0]).toBe(33);
  });
});
