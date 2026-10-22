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
/** Max extend_blocks for RenewMediaBlobPin. */
export declare const MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS = 1000000n;
/** Suggested Forever pin horizon for large / video-ish media (docs hint). */
export declare const MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS = 150000n;
/** Reed–Solomon parity shard count (matches Rust Phase 5). */
export declare const MEDIA_BLOB_RS_PARITY_SHARDS = 2;
/** Phase 5: extend pin horizon (`RenewMediaBlobPin`, variant 23). */
export declare function renewMediaBlobPinPayload(input: {
    blobId: Uint8Array;
    size: number;
    extendBlocks: bigint;
}): TransactionPayloadInput;
/** Phase 5: pay for one DAS sample (`PayBlobSample`, variant 24). */
export declare function payBlobSamplePayload(input: {
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
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
/** Design target for a future multi-minute path (not live). Live cap remains 16 MiB. */
export declare const PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE6_VIDEO_SEGMENT_TARGET_BYTES: number;
export declare const PHASE6_MAX_VIDEO_SEGMENTS = 256;
export declare const PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES = 32;
export declare const PHASE6_MAX_SAMPLE_OFFERS = 1024;
export type MediaBlobSampleOffer = {
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    providerHex: string;
    askFee: bigint;
    expiresAtHeight: bigint;
};
export type MediaBlobSegmentManifest = {
    contentType: string;
    totalBytes: bigint;
    segmentBlobIds: Uint8Array[];
};
/** Suggested segment count if splitting totalBytes into live-cap chunks. */
export declare function suggestedVideoSegmentCount(totalBytes: bigint): number;
/** Split bytes into segments each ≤ live MAX_MEDIA_BLOB_BYTES; refuses above Phase 6 design target. */
export declare function splitIntoLiveCapSegments(data: Uint8Array): Uint8Array[];
export declare function validateSegmentManifest(m: MediaBlobSegmentManifest): void;
/** Build manifest from segment bytes (each segment must be ≤ live 16 MiB). */
export declare function manifestFromSegmentBytes(contentType: string, segments: Uint8Array[]): MediaBlobSegmentManifest;
/** Content-addressed id of an ordered segment manifest. */
export declare function segmentManifestId(m: MediaBlobSegmentManifest): Uint8Array;
/** Match helper: paidFee must cover askFee (pairs with PayBlobSample on-chain). */
export declare function matchSampleOffer(offer: MediaBlobSampleOffer, paidFee: bigint, tipHeight?: bigint): MediaBlobSampleOffer & {
    paidFee: bigint;
    matchedAtHeight: bigint;
};
/** Height-bound deterministic seed (matches Rust `das_light_client_seed`). */
export declare function dasLightClientSeed(blobId: Uint8Array, tipHeight: bigint): bigint;
/** Deterministic light-client sample indices (scaffold; not VRF-bound). */
export declare function dasLightClientSampleIndices(blobSize: number, sampleCount: number, seed: bigint): number[];
/** Phase 7: register on-chain sample offer (`RegisterBlobSampleOffer`, variant 25). */
export declare function registerBlobSampleOfferPayload(input: {
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    askFee: bigint;
    expiresAtHeight?: bigint;
}): TransactionPayloadInput;
/** Phase 7: settle sample match (`SettleBlobSampleMatch`, variant 26). */
export declare function settleBlobSampleMatchPayload(input: {
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    provider: Uint8Array;
}): TransactionPayloadInput;
/** Phase 7 design / activated max (only live after cap activation height). */
export declare const PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
/** Effective max bytes: raised only when `phase7CapActive`. Default = live 16 MiB. */
export declare function mediaBlobMaxBytesAt(phase7CapActive: boolean): number;
/** VRF-bound DAS seed (matches Rust `das_light_client_seed_from_vrf`). Caller must verify VRF. */
export declare function dasLightClientSeedFromVrf(blobId: Uint8Array, vrfOutput: Uint8Array): bigint;
/** Honest succinct commitment stub (not a SNARK). */
export declare function dasSuccinctCommitmentStub(input: {
    blobId: Uint8Array;
    chunksRoot: Uint8Array;
    seedHeight: bigint;
    samples: Array<{
        chunkIndex: number;
        chunkHash: Uint8Array;
    }>;
}): Uint8Array;
/** Phase 8: settle quote = protocol sample fee + provider ask. */
export type BlobSampleSettleQuote = {
    protocolSampleFee: bigint;
    askFee: bigint;
    total: bigint;
    provider: Uint8Array;
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
};
export declare function quoteBlobSampleSettle(input: {
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    provider: Uint8Array;
    askFee: bigint;
    protocolSampleFee: bigint;
}): BlobSampleSettleQuote;
/** Phase 8 succinct proof kind: 0 = StubBlake3V0 (not a SNARK), 1 = reserved. */
export declare const DAS_SUCCINCT_PROOF_KIND_STUB_BLAKE3_V0 = 0;
export declare const DAS_SUCCINCT_PROOF_KIND_RESERVED_REAL_V1 = 1;
export type DasSuccinctProof = {
    kind: number;
    commitment: Uint8Array;
    blobId: Uint8Array;
    chunksRoot: Uint8Array;
    seedHeight: bigint;
    vrfOutput?: Uint8Array;
    sampleIndices: number[];
};
/** Build StubBlake3V0 envelope (matches Rust `build_das_succinct_proof_stub`). */
export declare function buildDasSuccinctProofStub(input: {
    blobId: Uint8Array;
    chunksRoot: Uint8Array;
    seedHeight: bigint;
    samples: Array<{
        chunkIndex: number;
        chunkHash: Uint8Array;
    }>;
}): DasSuccinctProof;
/** VRF-bound stub envelope (matches Rust `build_das_succinct_proof_stub_vrf`). */
export declare function buildDasSuccinctProofStubVrf(input: {
    blobId: Uint8Array;
    chunksRoot: Uint8Array;
    samples: Array<{
        chunkIndex: number;
        chunkHash: Uint8Array;
    }>;
}, vrfOutput: Uint8Array): DasSuccinctProof;
/** Verify StubBlake3V0 against expected commitment; reserved kinds always fail. */
export declare function verifyDasSuccinctProofStub(proof: DasSuccinctProof, expected: DasSuccinctProof): boolean;
/** Alias: activated max after coordinated cap raise. */
export declare const PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
/** Phase 9: cancel sample offer and refund SMT escrow (`CancelBlobSampleOffer`, variant 27). */
export declare function cancelBlobSampleOfferPayload(input: {
    blobId: Uint8Array;
    chunkIndex: number;
}): TransactionPayloadInput;
/** Phase 9 activated max alias. */
export declare const PHASE9_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
/** Domain tag for ask-fee escrow account (BLAKE3 → AccountId). */
export declare const PHASE9_ASK_ESCROW_DOMAIN = "BOING_BLOB_SAMPLE_ASK_ESCROW_V1";
/** System ask-escrow account id (matches Rust `blob_sample_ask_escrow_account`). */
export declare function blobSampleAskEscrowAccount(): Uint8Array;
/** DAS LC seed from consensus leader VRF output (same mixer as `dasLightClientSeedFromVrf`). */
export declare function dasLightClientSeedFromLeaderVrf(blobId: Uint8Array, leaderVrfOutput: Uint8Array): bigint;
/** Phase 10: challenge sample serve (`ChallengeBlobSampleServe`, variant 28). */
export declare function challengeBlobSampleServePayload(input: {
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    provider: Uint8Array;
}): TransactionPayloadInput;
/** Phase 10: respond to serve challenge with DAS merkle sample (variant 29). */
export declare function respondBlobSampleServePayload(input: {
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    challenger: Uint8Array;
    chunkHash: Uint8Array;
    chunksRoot: Uint8Array;
    siblings: Uint8Array[];
}): TransactionPayloadInput;
/** Phase 10 serve-challenge window (blocks). */
export declare const PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
/** Phase 10 activated max alias. */
export declare const PHASE10_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
/** Honest plan strings for succinct DAS path (matches Rust Phase 10). */
export declare function phase10DasSuccinctProofPlan(): string[];
/** Phase 11 serve-challenge window (same as Phase 10). */
export declare const PHASE11_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
/** Phase 11 activated max alias. */
export declare const PHASE11_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
/** Domain tag for serve-challenge bond escrow account (BLAKE3 → AccountId). */
export declare const PHASE11_SERVE_CHALLENGE_BOND_ESCROW_DOMAIN = "BOING_BLOB_SERVE_CHALLENGE_BOND_V1";
/** Challenger bond + provider penalty amounts (1 BOING = 10^18 base units). */
export declare const PHASE11_SERVE_CHALLENGE_BOND: bigint;
export declare const PHASE11_FAILED_SERVE_PENALTY: bigint;
/** System serve-challenge bond escrow account id. */
export declare function blobSampleServeChallengeBondEscrowAccount(): Uint8Array;
/** Honest plan strings for succinct DAS path (Phase 11 progress; verify still rejects). */
export declare function phase11DasSuccinctProofPlan(): string[];
/** Phase 12 aliases / constants. */
export declare const PHASE12_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE12_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE12_SERVE_CHALLENGE_BOND: bigint;
export declare const PHASE12_FAILED_SERVE_PENALTY: bigint;
export declare const PHASE12_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE12_SETTLE_RECEIPT_DOMAIN = "BOING_BLOB_SETTLE_RECEIPT_V1";
/** Honest plan strings for Phase 12 (settle-before-challenge + FailedServe audit). */
export declare function phase12DasSuccinctProofPlan(): string[];
/** Phase 13 aliases / constants. */
export declare const PHASE13_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE13_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE13_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE13_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
/** Honest plan strings for Phase 13 (FailedServe balance unburn + receipt TTL). */
export declare function phase13DasSuccinctProofPlan(): string[];
/** Phase 14 aliases / constants. */
export declare const PHASE14_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE14_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE14_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE14_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE14_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE14_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Phase 14: submit consensus slash appeal (`SubmitSlashAppeal`, variant 30). */
export declare function encodeSubmitSlashAppeal(input: {
    slashId: bigint;
    evidence?: Uint8Array;
}): TransactionPayloadInput;
/** Phase 14: resolve consensus slash appeal (`ResolveSlashAppeal`, variant 31). */
export declare function encodeResolveSlashAppeal(input: {
    appealId: bigint;
    approved: boolean;
}): TransactionPayloadInput;
/** Honest plan strings for Phase 14 (consensus slash-appeal txs; ReservedRealV1 still rejects). */
export declare function phase14DasSuccinctProofPlan(): string[];
/** Phase 15 aliases / constants. */
export declare const PHASE15_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE15_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE15_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE15_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE15_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE15_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE15_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Honest plan strings for Phase 15 (consensus equivocation/liveness appeals; still 16 MiB). */
export declare function phase15DasSuccinctProofPlan(): string[];
/** Phase 16 aliases / constants. */
export declare const PHASE16_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE16_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE16_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE16_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE16_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE16_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE16_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Fixed resolve quorum numerator (⌈2M/3⌉ via integer `votes * DEN >= NUM * M`). */
export declare const PHASE16_RESOLVE_THRESHOLD_NUM = 2;
/** Fixed resolve quorum denominator. */
export declare const PHASE16_RESOLVE_THRESHOLD_DEN = 3;
/** Honest plan strings for Phase 16 (committee threshold ResolveSlashAppeal; still 16 MiB). */
export declare function phase16DasSuccinctProofPlan(): string[];
/** Phase 17 aliases / constants. */
export declare const PHASE17_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE17_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE17_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE17_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE17_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE17_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE17_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉ via integer `weight * DEN >= NUM * W`). */
export declare const PHASE17_RESOLVE_THRESHOLD_NUM = 2;
/** Fixed stake-weighted resolve quorum denominator. */
export declare const PHASE17_RESOLVE_THRESHOLD_DEN = 3;
/** Honest plan strings for Phase 17 (stake-weighted ResolveSlashAppeal + versioned registry; still 16 MiB). */
export declare function phase17DasSuccinctProofPlan(): string[];
//# sourceMappingURL=mediaBlob.d.ts.map