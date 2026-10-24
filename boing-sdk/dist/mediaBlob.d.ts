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
/** Phase 18/22: attested resolve (`ResolveSlashAppealAttested`, variant 32). */
export declare function encodeResolveSlashAppealAttested(input: {
    appealId: bigint;
    approved: boolean;
    /**
     * 64-byte Ed25519 ballot. Prefer Phase 22 **V2** (`signSlashAppealResolveBallotV2`
     * over `pin_set_commitment`); historical V1 still dual-verifies on-node.
     */
    ballotSignature: Uint8Array;
}): TransactionPayloadInput;
/**
 * Phase 22: encode attested resolve by signing ballot domain **V2** (binds pin set).
 * `secretKey32` must match the voter AccountId / tx.sender.
 */
export declare function encodeResolveSlashAppealAttestedV2(input: {
    appealId: bigint;
    slashId: bigint;
    approved: boolean;
    chainId: bigint | number;
    pinSetCommitment: Uint8Array;
    secretKey32: Uint8Array;
}): Promise<TransactionPayloadInput>;
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
/** Phase 18 aliases / constants. */
export declare const PHASE18_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE18_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE18_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE18_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE18_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE18_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE18_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Attested resolve payload index (`ResolveSlashAppealAttested`). */
export declare const PHASE18_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉). */
export declare const PHASE18_RESOLVE_THRESHOLD_NUM = 2;
/** Fixed stake-weighted resolve quorum denominator. */
export declare const PHASE18_RESOLVE_THRESHOLD_DEN = 3;
/** Honest plan strings for Phase 18 (Ed25519 attested resolve + registry V3; still 16 MiB). */
export declare function phase18DasSuccinctProofPlan(): string[];
/** Phase 19 aliases / constants. */
export declare const PHASE19_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE19_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE19_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE19_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE19_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE19_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE19_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Attested resolve payload index (`ResolveSlashAppealAttested`). */
export declare const PHASE19_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉). */
export declare const PHASE19_RESOLVE_THRESHOLD_NUM = 2;
/** Fixed stake-weighted resolve quorum denominator. */
export declare const PHASE19_RESOLVE_THRESHOLD_DEN = 3;
/**
 * True when a multi-pin committee must use attested resolve (32).
 * `pinnedLen` is `|slash_appeal_resolvers|` when the host pin set is present.
 */
export declare function phase19CommitteeRequiresAttested(pinnedLen: number): boolean;
/** Honest plan strings for Phase 19 (committee attested-required; still 16 MiB). */
export declare function phase19DasSuccinctProofPlan(): string[];
/** Phase 20 aliases / constants. */
export declare const PHASE20_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE20_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE20_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE20_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE20_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE20_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Attested resolve payload index (`ResolveSlashAppealAttested`). */
export declare const PHASE20_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉). */
export declare const PHASE20_RESOLVE_THRESHOLD_NUM = 2;
/** Fixed stake-weighted resolve quorum denominator. */
export declare const PHASE20_RESOLVE_THRESHOLD_DEN = 3;
/** Sentinel: pre-V4 Pending appeals use live-W fallback when snapshot is this value + zero pin commit. */
export declare const PHASE20_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/**
 * True when a multi-pin committee must use attested resolve (32) (Phase 19 alias).
 */
export declare function phase20CommitteeRequiresAttested(pinnedLen: number): boolean;
/** Honest plan strings for Phase 20 (electorate W snapshot at Submit; still 16 MiB). */
export declare function phase20DasSuccinctProofPlan(): string[];
/** Phase 21 aliases / constants. */
export declare const PHASE21_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE21_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE21_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
/** Resolve window after SubmitSlashAppeal (tip + this many blocks). */
export declare const PHASE21_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE21_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE21_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE21_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE21_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Attested resolve payload index (`ResolveSlashAppealAttested`). */
export declare const PHASE21_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉). */
export declare const PHASE21_RESOLVE_THRESHOLD_NUM = 2;
/** Fixed stake-weighted resolve quorum denominator. */
export declare const PHASE21_RESOLVE_THRESHOLD_DEN = 3;
/** Sentinel: pre-V4 Pending appeals use live-W fallback when snapshot is this value + zero pin commit. */
export declare const PHASE21_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/**
 * True when a multi-pin committee must use attested resolve (32) (Phase 19 alias).
 */
export declare function phase21CommitteeRequiresAttested(pinnedLen: number): boolean;
/** `resolve_deadline` from Submit tip height. */
export declare function phase21ResolveDeadline(submitHeight: bigint): bigint;
/** True when tip is strictly past the resolve deadline. */
export declare function phase21ResolveExpired(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 21 (resolve expiry + Pending cleanup; still 16 MiB). */
export declare function phase21DasSuccinctProofPlan(): string[];
/** Phase 22 ballot domain tags. */
export declare const SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V1 = "BOING_SLASH_APPEAL_RESOLVE_BALLOT_V1";
export declare const SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V2 = "BOING_SLASH_APPEAL_RESOLVE_BALLOT_V2";
/** Phase 22 aliases / constants. */
export declare const PHASE22_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE22_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE22_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE22_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE22_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE22_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE22_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE22_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE22_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE22_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE22_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE22_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/** True when a multi-pin committee must use attested resolve (32) (Phase 19 alias). */
export declare function phase22CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase22ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase22ResolveExpired(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** BLAKE3 preimage for ballot domain V1 (historical). */
export declare function slashAppealResolveBallotPreimageV1(input: {
    chainId: bigint | number;
    appealId: bigint | number;
    slashId: bigint | number;
    approved: boolean;
    voter: Uint8Array;
}): Uint8Array;
/** BLAKE3 preimage for ballot domain V2 (binds pin_set_commitment). */
export declare function slashAppealResolveBallotPreimageV2(input: {
    chainId: bigint | number;
    appealId: bigint | number;
    slashId: bigint | number;
    approved: boolean;
    voter: Uint8Array;
    pinSetCommitment: Uint8Array;
}): Uint8Array;
/** Sign a historical V1 resolve ballot. */
export declare function signSlashAppealResolveBallotV1(input: {
    chainId: bigint | number;
    appealId: bigint | number;
    slashId: bigint | number;
    approved: boolean;
    secretKey32: Uint8Array;
}): Promise<Uint8Array>;
/** Sign a Phase 22 V2 resolve ballot (binds pin set). Prefer for new 32 casts. */
export declare function signSlashAppealResolveBallotV2(input: {
    chainId: bigint | number;
    appealId: bigint | number;
    slashId: bigint | number;
    approved: boolean;
    pinSetCommitment: Uint8Array;
    secretKey32: Uint8Array;
}): Promise<Uint8Array>;
export type SlashAppealBallotDomain = 'v1' | 'v2';
/**
 * Dual-verify a ballot: non-legacy pin tries V2 then V1; legacy all-zero pin is V1-only.
 * Returns which domain matched, or null if neither verifies.
 */
export declare function verifySlashAppealResolveBallotDual(input: {
    chainId: bigint | number;
    appealId: bigint | number;
    slashId: bigint | number;
    approved: boolean;
    voter: Uint8Array;
    pinSetCommitment: Uint8Array;
    ballotSignature: Uint8Array;
}): Promise<SlashAppealBallotDomain | null>;
/** Honest plan strings for Phase 22 (batch re-verify + pin-bound V2 ballots; still 16 MiB). */
export declare function phase22DasSuccinctProofPlan(): string[];
/** Phase 23 retention for Expired slash appeals (blocks after resolve_deadline). */
export declare const PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
/** Phase 23 aliases / constants. */
export declare const PHASE23_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE23_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE23_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE23_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE23_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE23_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE23_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE23_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE23_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE23_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE23_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE23_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/** True when a multi-pin committee must use attested resolve (32) (Phase 19 alias). */
export declare function phase23CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase23ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase23ResolveExpired(tipHeight: bigint, resolveDeadline: bigint): boolean;
/**
 * True when an Expired appeal is past retention and may be pruned:
 * `tipHeight > resolveDeadline + PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS`.
 */
export declare function phase23ExpiredAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 23 (Expired retention prune + list filters; still 16 MiB). */
export declare function phase23DasSuccinctProofPlan(): string[];
/** Phase 24 retention for terminal serve challenges (blocks after deadline). */
export declare const PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
/** Phase 24 aliases / constants. */
export declare const PHASE24_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE24_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE24_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE24_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE24_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE24_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE24_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE24_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE24_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE24_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE24_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE24_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE24_LEGACY_ELECTORATE_W_SENTINEL = 0n;
export declare function phase24CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase24ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase24ResolveExpired(tipHeight: bigint, resolveDeadline: bigint): boolean;
export declare function phase24ExpiredAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
export type BlobSampleServeChallengeStatusName = 'open' | 'responded' | 'failedServe';
/**
 * True when a terminal serve challenge is past retention and may be pruned:
 * settled Responded/FailedServe with `tip > deadline + PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS`.
 */
export declare function phase24TerminalServeChallengePrunable(input: {
    tipHeight: bigint;
    deadlineHeight: bigint;
    status: BlobSampleServeChallengeStatusName;
    bondSettled: boolean;
}): boolean;
/** Honest plan strings for Phase 24 (serve-challenge TTL prune + list/get UX; still 16 MiB). */
export declare function phase24DasSuccinctProofPlan(): string[];
/** Phase 25 retention for Approved/Rejected slash appeals (blocks after resolve_deadline). */
export declare const PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
/** Phase 25 aliases / constants. */
export declare const PHASE25_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE25_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE25_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE25_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE25_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE25_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE25_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
export declare const PHASE25_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE25_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE25_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE25_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE25_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE25_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE25_LEGACY_ELECTORATE_W_SENTINEL = 0n;
export declare function phase25CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase25ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase25ResolveExpired(tipHeight: bigint, resolveDeadline: bigint): boolean;
export declare function phase25ExpiredAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/**
 * True when an Approved/Rejected appeal is past retention and may be pruned:
 * `tipHeight > resolveDeadline + PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS`.
 */
export declare function phase25ResolvedAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
export declare function phase25TerminalServeChallengePrunable(input: {
    tipHeight: bigint;
    deadlineHeight: bigint;
    status: BlobSampleServeChallengeStatusName;
    bondSettled: boolean;
}): boolean;
/** Honest plan strings for Phase 25 (ancient Approved/Rejected prune + list filters; still 16 MiB). */
export declare function phase25DasSuccinctProofPlan(): string[];
/** Compressed BLS12-381 G1 public key size (bytes). */
export declare const PHASE26_BLS_PUBLIC_KEY_G1_BYTES = 48;
/** Compressed BLS12-381 G2 signature / aggregate size (bytes). */
export declare const PHASE26_BLS_SIGNATURE_G2_BYTES = 96;
/**
 * Flip only when a real `blst` (or equivalent) verify path is wired.
 * Scaffold keeps this **false** — do not treat types as live crypto.
 */
export declare const PHASE26_BLS_AGGREGATE_WIRED = false;
/** Phase 26 aliases / constants. */
export declare const PHASE26_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE26_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE26_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE26_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE26_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE26_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE26_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
export declare const PHASE26_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE26_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE26_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE26_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE26_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE26_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE26_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE26_LEGACY_ELECTORATE_W_SENTINEL = 0n;
export type DualKeyIdentity = {
    accountId: Uint8Array;
    blsPubkey: Uint8Array;
};
/** Length-check a dual-key set (unique accountIds). No curve validation until wired. */
export declare function phase26ValidateDualKeySet(identities: DualKeyIdentity[]): void;
/**
 * Aggregate / verify stubs — always throw until `PHASE26_BLS_AGGREGATE_WIRED`.
 * Live resolve path remains Ed25519 verify-each on tx 32.
 */
export declare function phase26AggregateBlsSignatures(_signatures: Uint8Array[]): never;
export declare function phase26VerifyBlsAggregate(_input: {
    message: Uint8Array;
    aggregate: Uint8Array;
    identities: DualKeyIdentity[];
}): never;
export declare function phase26CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase26ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase26ResolvedAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 26 (BLS dual-key scaffold; still 16 MiB). */
export declare function phase26DasSuccinctProofPlan(): string[];
/** Compressed BLS12-381 sizes (same as Phase 26). */
export declare const PHASE27_BLS_PUBLIC_KEY_G1_BYTES = 48;
export declare const PHASE27_BLS_SIGNATURE_G2_BYTES = 96;
/**
 * Node library + RPC aggregate verify is wired (`blst` min_pk).
 * Live tx **32** remains Ed25519 verify-each. JS SDK has no blst — use
 * `boing_verifyBlsAggregate` on the node for real verify.
 */
export declare const PHASE27_BLS_AGGREGATE_WIRED = true;
export declare const PHASE27_BLS_DUAL_KEY_REGISTRY_FILE = "bls_dual_key_registry.bin";
/** Phase 27 aliases / constants. */
export declare const PHASE27_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE27_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE27_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE27_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE27_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE27_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE27_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
export declare const PHASE27_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE27_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE27_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE27_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE27_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE27_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE27_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE27_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/** Length-check a dual-key set (unique accountIds). */
export declare function phase27ValidateDualKeySet(identities: DualKeyIdentity[]): void;
/**
 * JS has no `blst` — do not fake crypto. Use node RPC `boing_verifyBlsAggregate`
 * (or the Rust library). Live tx 32 remains Ed25519 verify-each.
 */
export declare function phase27AggregateBlsSignatures(_signatures: Uint8Array[]): never;
export declare function phase27VerifyBlsAggregate(_input: {
    message: Uint8Array;
    aggregate: Uint8Array;
    identities: DualKeyIdentity[];
}): never;
export declare function phase27CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase27ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase27ResolvedAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 27 (blst wire + host-local dual-keys; still 16 MiB). */
export declare function phase27DasSuccinctProofPlan(): string[];
/** Phase 28 sizes / wired flags (library+RPC still from Phase 27). */
export declare const PHASE28_BLS_PUBLIC_KEY_G1_BYTES = 48;
export declare const PHASE28_BLS_SIGNATURE_G2_BYTES = 96;
export declare const PHASE28_BLS_AGGREGATE_WIRED = true;
export declare const PHASE28_BLS_DUAL_KEY_REGISTRY_FILE = "bls_dual_key_registry.bin";
/** Registration requires a verified BLS proof-of-possession. */
export declare const PHASE28_BLS_POP_REQUIRED = true;
/** BLAKE3 domain for the PoP preimage (not the blst DST). */
export declare const PHASE28_BLS_POP_DOMAIN: Uint8Array<ArrayBuffer>;
/** CipherSuite DST string for dual-key PoP (Rust/`blst` signers). */
export declare const PHASE28_BLS_POP_SIG_DST = "BOING_BLS_POP_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";
export declare const PHASE28_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE28_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE28_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE28_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE28_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE28_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE28_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
export declare const PHASE28_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE28_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE28_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE28_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE28_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE28_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE28_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE28_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/**
 * BLAKE3 PoP preimage: domain || chainId_le8 || accountId || blsPubkey.
 * Sign this 32-byte message with blst under `PHASE28_BLS_POP_SIG_DST` (Rust tooling).
 */
export declare function phase28BlsPopPreimage(input: {
    chainId: bigint | number;
    accountId: Uint8Array;
    blsPubkey: Uint8Array;
}): Uint8Array;
/** Length-check a dual-key set (unique accountIds). */
export declare function phase28ValidateDualKeySet(identities: DualKeyIdentity[]): void;
/**
 * JS has no `blst` — do not fake PoP sign/verify. Use Rust/`blst` to sign
 * `phase28BlsPopPreimage`, then `boing_registerBlsDualKey` with `blsPop`.
 */
export declare function phase28SignBlsPop(_input: {
    ikm: Uint8Array;
    chainId: bigint | number;
    accountId: Uint8Array;
    blsPubkey: Uint8Array;
}): never;
export declare function phase28VerifyBlsPop(_input: {
    chainId: bigint | number;
    accountId: Uint8Array;
    blsPubkey: Uint8Array;
    pop: Uint8Array;
}): never;
export declare function phase28CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase28ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase28ResolvedAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 28 (BLS dual-key PoP; still 16 MiB). */
export declare function phase28DasSuccinctProofPlan(): string[];
/** Phase 29 sizes / wired flags (library + dry-run RPC; consensus admit still off). */
export declare const PHASE29_BLS_PUBLIC_KEY_G1_BYTES = 48;
export declare const PHASE29_BLS_SIGNATURE_G2_BYTES = 96;
export declare const PHASE29_BLS_AGGREGATE_WIRED = true;
export declare const PHASE29_BLS_DUAL_KEY_REGISTRY_FILE = "bls_dual_key_registry.bin";
export declare const PHASE29_BLS_POP_REQUIRED = true;
/** Reserved payload index for future ResolveSlashAppealBlsAggregate (not consensus-admitted). */
export declare const PHASE29_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX = 33;
/** Soft cap on pin-set length for bitset helpers. */
export declare const PHASE29_MAX_PIN_SET_LEN = 64;
/** Library + RPC dry-run helpers are available. */
export declare const PHASE29_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED = true;
/** Consensus admit of tx 33 stays false (dual-keys still host-local). */
export declare const PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED = false;
export declare const PHASE29_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE29_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE29_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE29_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE29_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE29_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE29_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
export declare const PHASE29_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE29_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE29_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE29_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE29_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE29_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE29_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE29_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/** Aggregate resolve ballot payload (library / future tx 33 body). */
export type BlsAggregateResolvePayload = {
    appealId: bigint;
    approved: boolean;
    /** Bitset over Submit-frozen pin-set order (LSB of byte 0 = pinned[0]). */
    signerBitset: Uint8Array;
    aggregateSignature: Uint8Array;
};
/** Encode a pin-set-relative signer bitset (LSB = pinned[0]). */
export declare function phase29EncodeSignerBitset(pinned: Uint8Array[], signers: Uint8Array[]): Uint8Array;
/** Decode bitset → pin-set AccountIds (ascending pin index). */
export declare function phase29DecodeSignerBitset(pinned: Uint8Array[], bitset: Uint8Array): Uint8Array[];
/**
 * JS has no `blst` — do not fake aggregate-resolve verify.
 * Use node RPC `boing_verifyBlsAggregateResolve` (library wired; consensus admit false).
 */
export declare function phase29VerifyBlsAggregateResolve(_input: {
    chainId: bigint | number;
    slashId: bigint | number;
    pinSetCommitment: Uint8Array;
    pinned: Uint8Array[];
    payload: BlsAggregateResolvePayload;
    identities: DualKeyIdentity[];
}): never;
/** Consensus admit helper — always throws until dual-keys are consensus-visible. */
export declare function phase29AdmitBlsAggregateResolveTx(): never;
export declare function phase29CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase29ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase29ResolvedAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 29 (aggregate resolve payload + bitset; still 16 MiB). */
export declare function phase29DasSuccinctProofPlan(): string[];
/** Phase 30 sizes / wired flags (self-contained pubkeys; consensus admit still off). */
export declare const PHASE30_BLS_PUBLIC_KEY_G1_BYTES = 48;
export declare const PHASE30_BLS_SIGNATURE_G2_BYTES = 96;
export declare const PHASE30_BLS_AGGREGATE_WIRED = true;
export declare const PHASE30_BLS_DUAL_KEY_REGISTRY_FILE = "bls_dual_key_registry.bin";
export declare const PHASE30_BLS_POP_REQUIRED = true;
export declare const PHASE30_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX = 33;
export declare const PHASE30_MAX_PIN_SET_LEN = 64;
export declare const PHASE30_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED = true;
/** Self-contained G1 pubkey list verify path (no host registry). */
export declare const PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED = true;
/** Consensus admit of tx 33 stays false (mempool/VM reject). */
export declare const PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED = false;
export declare const PHASE30_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE30_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE30_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE30_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE30_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE30_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE30_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
export declare const PHASE30_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE30_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE30_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE30_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE30_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE30_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE30_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE30_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/** Aggregate resolve with self-contained G1 pubkeys (library / future tx 33 body). */
export type BlsAggregateResolveSelfContainedPayload = BlsAggregateResolvePayload & {
    /** Ordered with decoded bitset (ascending pin index); each 48 bytes. */
    signerBlsPubkeys: Uint8Array[];
};
export declare function phase30EncodeSignerBitset(pinned: Uint8Array[], signers: Uint8Array[]): Uint8Array;
export declare function phase30DecodeSignerBitset(pinned: Uint8Array[], bitset: Uint8Array): Uint8Array[];
/** Build dual-key identities from pin-set + bitset + self-contained pubkeys. */
export declare function phase30IdentitiesFromSelfContained(input: {
    pinned: Uint8Array[];
    bitset: Uint8Array;
    pubkeys: Uint8Array[];
}): DualKeyIdentity[];
/**
 * JS has no `blst` — do not fake self-contained aggregate-resolve verify.
 * Use node RPC `boing_verifyBlsAggregateResolve` with `signerPubkeys`.
 */
export declare function phase30VerifyBlsAggregateResolveSelfContained(_input: {
    chainId: bigint | number;
    slashId: bigint | number;
    pinSetCommitment: Uint8Array;
    pinned: Uint8Array[];
    payload: BlsAggregateResolveSelfContainedPayload;
}): never;
/** Consensus admit helper — always throws until operators opt in. */
export declare function phase30AdmitBlsAggregateResolveTx(): never;
/** Encode ResolveSlashAppealBlsAggregate (33) — consensus admit still rejects on node. */
export declare function encodeResolveSlashAppealBlsAggregate(input: {
    appealId: bigint;
    approved: boolean;
    signerBitset: Uint8Array;
    aggregateSignature: Uint8Array;
    signerBlsPubkeys: Uint8Array[];
}): TransactionPayloadInput;
export declare function phase30CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase30ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase30ResolvedAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 30 (self-contained pubkeys; still 16 MiB). */
export declare function phase30DasSuccinctProofPlan(): string[];
/** Phase 31: consensus admit/execute tx 33; multi-pin default still Ed25519 32. */
export declare const PHASE31_BLS_PUBLIC_KEY_G1_BYTES = 48;
export declare const PHASE31_BLS_SIGNATURE_G2_BYTES = 96;
export declare const PHASE31_BLS_AGGREGATE_WIRED = true;
export declare const PHASE31_BLS_DUAL_KEY_REGISTRY_FILE = "bls_dual_key_registry.bin";
export declare const PHASE31_BLS_POP_REQUIRED = true;
export declare const PHASE31_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX = 33;
export declare const PHASE31_MAX_PIN_SET_LEN = 64;
export declare const PHASE31_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED = true;
export declare const PHASE31_BLS_AGGREGATE_SELF_CONTAINED_WIRED = true;
/** Consensus admit/execute of tx 33 is on (optional path). */
export declare const PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED = true;
/** Multi-pin committee default remains Ed25519 tx 32. */
export declare const PHASE31_BLS_LIVE_TX32_STILL_ED25519 = true;
export declare const PHASE31_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE31_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE31_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE31_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE31_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE31_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE31_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
export declare const PHASE31_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE31_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE31_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE31_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE31_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE31_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE31_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE31_LEGACY_ELECTORATE_W_SENTINEL = 0n;
export declare function phase31EncodeSignerBitset(pinned: Uint8Array[], signers: Uint8Array[]): Uint8Array;
export declare function phase31DecodeSignerBitset(pinned: Uint8Array[], bitset: Uint8Array): Uint8Array[];
export declare function phase31IdentitiesFromSelfContained(input: {
    pinned: Uint8Array[];
    bitset: Uint8Array;
    pubkeys: Uint8Array[];
}): DualKeyIdentity[];
/** Admit helper — succeeds in Phase 31 (consensus wired). */
export declare function phase31AdmitBlsAggregateResolveTx(): void;
/**
 * JS has no `blst` — do not fake verify. Encode with `encodeResolveSlashAppealBlsAggregate`
 * and submit; node verifies on execute. Dry-run via `boing_verifyBlsAggregateResolve`.
 */
export declare function phase31VerifyBlsAggregateResolveSelfContained(_input: {
    chainId: bigint | number;
    slashId: bigint | number;
    pinSetCommitment: Uint8Array;
    pinned: Uint8Array[];
    payload: BlsAggregateResolveSelfContainedPayload;
}): never;
export declare function phase31CommitteeRequiresAttested(pinnedLen: number): boolean;
export declare function phase31ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase31ResolvedAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 31 (consensus tx 33; default cast still 32). */
export declare function phase31DasSuccinctProofPlan(): string[];
/** Phase 32: opt-in flip of multi-pin committee verify from Ed25519 32 → BLS aggregate 33. */
export declare const PHASE32_BLS_COMMITTEE_FLIP_ENV = "BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT";
export declare const PHASE32_BLS_PUBLIC_KEY_G1_BYTES = 48;
export declare const PHASE32_BLS_SIGNATURE_G2_BYTES = 96;
export declare const PHASE32_BLS_AGGREGATE_WIRED = true;
export declare const PHASE32_BLS_DUAL_KEY_REGISTRY_FILE = "bls_dual_key_registry.bin";
export declare const PHASE32_BLS_POP_REQUIRED = true;
export declare const PHASE32_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX = 33;
export declare const PHASE32_MAX_PIN_SET_LEN = 64;
export declare const PHASE32_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED = true;
export declare const PHASE32_BLS_AGGREGATE_SELF_CONTAINED_WIRED = true;
/** Consensus admit/execute of tx 33 remains on. */
export declare const PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED = true;
/**
 * Compile-time default: until operators set the flip height, live multi-pin cast stays Ed25519 tx 32.
 * Prefer dynamic `!phase32BlsCommitteeFlipActive(height, flipFrom)` for live tip checks.
 */
export declare const PHASE32_BLS_LIVE_TX32_STILL_ED25519 = true;
export declare const PHASE32_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;
export declare const PHASE32_ACTIVATED_MAX_MEDIA_BLOB_BYTES: number;
export declare const PHASE32_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export declare const PHASE32_RESOLVE_WINDOW_BLOCKS = 1000n;
export declare const PHASE32_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
export declare const PHASE32_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE32_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
export declare const PHASE32_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
export declare const PHASE32_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export declare const PHASE32_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export declare const PHASE32_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
export declare const PHASE32_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
export declare const PHASE32_RESOLVE_THRESHOLD_NUM = 2;
export declare const PHASE32_RESOLVE_THRESHOLD_DEN = 3;
export declare const PHASE32_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/** True when flip is configured and `height >= flipFrom`. */
export declare function phase32BlsCommitteeFlipActive(height: bigint | number, flipFrom: bigint | number | null | undefined): boolean;
/**
 * When flip not active: same as Phase 31 (`pinnedLen > 1` ⇒ require Ed25519 32).
 * When flip active: return false (multi-pin no longer requires Ed25519 32).
 */
export declare function phase32CommitteeRequiresAttested(pinnedLen: number, height: bigint | number, flipFrom: bigint | number | null | undefined): boolean;
/** True when flip is active and `pinnedLen > 1` (multi-pin must use tx 33). */
export declare function phase32CommitteeRequiresBlsAggregate(pinnedLen: number, height: bigint | number, flipFrom: bigint | number | null | undefined): boolean;
export declare function phase32EncodeSignerBitset(pinned: Uint8Array[], signers: Uint8Array[]): Uint8Array;
export declare function phase32DecodeSignerBitset(pinned: Uint8Array[], bitset: Uint8Array): Uint8Array[];
export declare function phase32IdentitiesFromSelfContained(input: {
    pinned: Uint8Array[];
    bitset: Uint8Array;
    pubkeys: Uint8Array[];
}): DualKeyIdentity[];
/** Admit helper — succeeds in Phase 32 (consensus wired). */
export declare function phase32AdmitBlsAggregateResolveTx(): void;
/**
 * JS has no `blst` — do not fake verify. Encode with `encodeResolveSlashAppealBlsAggregate`
 * and submit; node verifies on execute. Dry-run via `boing_verifyBlsAggregateResolve`.
 */
export declare function phase32VerifyBlsAggregateResolveSelfContained(_input: {
    chainId: bigint | number;
    slashId: bigint | number;
    pinSetCommitment: Uint8Array;
    pinned: Uint8Array[];
    payload: BlsAggregateResolveSelfContainedPayload;
}): never;
export declare function phase32ResolveDeadline(submitHeight: bigint): bigint;
export declare function phase32ResolvedAppealPrunable(tipHeight: bigint, resolveDeadline: bigint): boolean;
/** Honest plan strings for Phase 32 (opt-in BLS multi-pin committee flip). */
export declare function phase32DasSuccinctProofPlan(): string[];
//# sourceMappingURL=mediaBlob.d.ts.map