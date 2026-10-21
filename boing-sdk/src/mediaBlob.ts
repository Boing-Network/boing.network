/**
 * L1 media blobs — content-addressed PutBlob / BlobAnnounce helpers (not the interface SSTORE store).
 * @see docs/BOING-L1-MEDIA-BLOBS.md
 */

import { blake3 } from '@noble/hashes/blake3';
import type { TransactionPayloadInput } from './bincode.js';
import { bytesToHex, ensureHex, hexToBytes } from './hex.js';

/** Live consensus hard cap (16 MiB). Still not multi-minute video-scale. */
export const MAX_MEDIA_BLOB_BYTES = 16 * 1024 * 1024;
/** Alias: Phase 3 target is now the live cap. */
export const PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES = MAX_MEDIA_BLOB_BYTES;
/** Prefer BlobAnnounce + gossip above this size (PutBlob still allows up to max). */
export const MAX_MEDIA_BLOB_INLINE_PREFER_BYTES = 1024 * 1024;
export const MAX_MEDIA_BLOB_INLINE_BYTES = MAX_MEDIA_BLOB_INLINE_PREFER_BYTES;
/** DAS / erasure chunk size. */
export const MEDIA_BLOB_DAS_CHUNK_BYTES = 64 * 1024;
export const MAX_MEDIA_BLOB_CONTENT_TYPE_LEN = 64;
export const MAX_MEDIA_BLOB_TTL_BLOCKS = 1_000_000n;
export const MEDIA_BLOB_GAS_BASE = 21_000n;
export const MEDIA_BLOB_GAS_PER_WORD = 16n;
export const MEDIA_BLOB_FOREVER_MULTIPLIER = 10n;
export const MEDIA_BLOB_URI_PREFIX = 'boing://blob/';
export const MEDIA_BLOBS_GOSSIP_TOPIC = 'boing/media-blobs';

/** Ceil chunk count for DAS / XOR erasure stubs. */
export function mediaBlobDasChunkCount(size: number): number {
  if (size <= 0) return 0;
  return Math.ceil(size / MEDIA_BLOB_DAS_CHUNK_BYTES);
}

/** Forever pin-rent units (KiB × blocks) for off-chain markets; on-chain uses `blob_forever_pin_fee_per_kib`. */
export function mediaBlobPinRentKibBlocks(size: number, ttlBlocks: bigint): bigint {
  const kib = BigInt(Math.max(1, Math.ceil(size / 1024)));
  return kib * ttlBlocks;
}

function chunkHash(bytes: Uint8Array): Uint8Array {
  return blake3(bytes);
}

function hashPair(a: Uint8Array, b: Uint8Array): Uint8Array {
  const domain = new TextEncoder().encode('BOING_BLOB_CHUNK_MERKLE_V1');
  const buf = new Uint8Array(domain.length + 64);
  buf.set(domain, 0);
  buf.set(a, domain.length);
  buf.set(b, domain.length + 32);
  return blake3(buf);
}

/** Binary merkle root over 32-byte chunk hashes (matches Rust `media_blob_chunks_merkle_root`). */
export function mediaBlobChunksMerkleRoot(leaves: Uint8Array[]): Uint8Array {
  if (leaves.length === 0) return new Uint8Array(32);
  let layer: Uint8Array[] = leaves.map((l) => new Uint8Array(l));
  while (layer.length > 1) {
    const next: Uint8Array[] = [];
    for (let i = 0; i < layer.length; i += 2) {
      const left = layer[i]!;
      const right = i + 1 < layer.length ? layer[i + 1]! : left;
      next.push(new Uint8Array(hashPair(left, right)));
    }
    layer = next;
  }
  return layer[0]!;
}

export type MediaBlobErasureBundle = {
  blobId: Uint8Array;
  originalLen: number;
  dataChunks: Uint8Array[];
  parityChunk: Uint8Array;
  chunkHashes: Uint8Array[];
  chunksRoot: Uint8Array;
};

/** Split into DAS-sized chunks + XOR parity (recover one missing data chunk). */
export function chunkMediaBlob(data: Uint8Array): MediaBlobErasureBundle {
  if (data.length > MAX_MEDIA_BLOB_BYTES) {
    throw new Error(`media blob exceeds MAX_MEDIA_BLOB_BYTES (${MAX_MEDIA_BLOB_BYTES})`);
  }
  const blobId = blobIdOf(data);
  if (data.length === 0) {
    return {
      blobId,
      originalLen: 0,
      dataChunks: [],
      parityChunk: new Uint8Array(0),
      chunkHashes: [],
      chunksRoot: new Uint8Array(32),
    };
  }
  const dataChunks: Uint8Array[] = [];
  for (let off = 0; off < data.length; off += MEDIA_BLOB_DAS_CHUNK_BYTES) {
    const slice = data.subarray(off, Math.min(off + MEDIA_BLOB_DAS_CHUNK_BYTES, data.length));
    const padded = new Uint8Array(MEDIA_BLOB_DAS_CHUNK_BYTES);
    padded.set(slice);
    dataChunks.push(padded);
  }
  const parityChunk = new Uint8Array(MEDIA_BLOB_DAS_CHUNK_BYTES);
  for (const ch of dataChunks) {
    for (let i = 0; i < MEDIA_BLOB_DAS_CHUNK_BYTES; i++) parityChunk[i]! ^= ch[i]!;
  }
  const chunkHashes = dataChunks.map((c) => chunkHash(c));
  return {
    blobId,
    originalLen: data.length,
    dataChunks,
    parityChunk,
    chunkHashes,
    chunksRoot: mediaBlobChunksMerkleRoot(chunkHashes),
  };
}

/** Recover bytes when ≤1 data chunk is missing (XOR) or all present. */
export function recoverMediaBlobXor(
  blobId: Uint8Array,
  originalLen: number,
  dataChunks: Array<Uint8Array | null>,
  parity: Uint8Array,
): Uint8Array {
  if (originalLen === 0) {
    const empty = new Uint8Array(0);
    if (!bytesEqual(blobIdOf(empty), blobId)) throw new Error('recovered bytes do not match blob id');
    return empty;
  }
  const missing = dataChunks.map((c, i) => (c == null ? i : -1)).filter((i) => i >= 0);
  let chunks: Uint8Array[];
  if (missing.length === 0) {
    chunks = dataChunks.map((c) => c!);
  } else if (missing.length === 1) {
    if (parity.length !== MEDIA_BLOB_DAS_CHUNK_BYTES) throw new Error('cannot recover blob');
    const miss = missing[0]!;
    const recovered = Uint8Array.from(parity);
    for (let i = 0; i < dataChunks.length; i++) {
      if (i === miss) continue;
      const bytes = dataChunks[i];
      if (!bytes || bytes.length !== MEDIA_BLOB_DAS_CHUNK_BYTES) throw new Error('cannot recover blob');
      for (let j = 0; j < MEDIA_BLOB_DAS_CHUNK_BYTES; j++) recovered[j]! ^= bytes[j]!;
    }
    chunks = dataChunks.map((c, i) => (i === miss ? recovered : c!));
  } else {
    throw new Error('cannot recover blob');
  }
  const out = new Uint8Array(originalLen);
  let off = 0;
  for (const ch of chunks) {
    const n = Math.min(ch.length, originalLen - off);
    out.set(ch.subarray(0, n), off);
    off += n;
    if (off >= originalLen) break;
  }
  if (!bytesEqual(blobIdOf(out), blobId)) throw new Error('recovered bytes do not match blob id');
  return out;
}

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

export type BlobRetentionInput =
  | { kind: 'forever' }
  | { kind: 'prunable'; ttlBlocks: bigint };

/** `BlobId = BLAKE3(data)` (32 bytes). */
export function blobIdOf(data: Uint8Array): Uint8Array {
  return blake3(data);
}

/** Hex without `0x` (64 chars). */
export function blobIdHex(id: Uint8Array): string {
  return bytesToHex(id).replace(/^0x/i, '');
}

export function mediaBlobUri(id: Uint8Array): string {
  return `${MEDIA_BLOB_URI_PREFIX}0x${blobIdHex(id)}`;
}

/** Parse `boing://blob/0x…`, `0x…`, or bare 64-hex. */
export function parseMediaBlobId(s: string): Uint8Array | null {
  const t = s.trim();
  let hex = t;
  if (t.startsWith(MEDIA_BLOB_URI_PREFIX)) {
    hex = t.slice(MEDIA_BLOB_URI_PREFIX.length);
  }
  try {
    const b = hexToBytes(ensureHex(hex));
    return b.length === 32 ? b : null;
  } catch {
    return null;
  }
}

export function mediaBlobGasRaw(size: number): bigint {
  const words = BigInt(Math.max(1, Math.ceil(size / 32)));
  return MEDIA_BLOB_GAS_BASE + words * MEDIA_BLOB_GAS_PER_WORD;
}

export function mediaBlobGas(size: number, retention: BlobRetentionInput): bigint {
  const mult = retention.kind === 'forever' ? MEDIA_BLOB_FOREVER_MULTIPLIER : 1n;
  return mediaBlobGasRaw(size) * mult;
}

export function validatePutBlob(
  retention: BlobRetentionInput,
  contentType: string,
  data: Uint8Array,
): Uint8Array {
  if (data.length > MAX_MEDIA_BLOB_BYTES) {
    throw new Error(`media blob exceeds MAX_MEDIA_BLOB_BYTES (${MAX_MEDIA_BLOB_BYTES})`);
  }
  if (contentType.length > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN) {
    throw new Error(`contentType exceeds ${MAX_MEDIA_BLOB_CONTENT_TYPE_LEN} bytes`);
  }
  if (retention.kind === 'prunable') {
    if (retention.ttlBlocks < 1n) throw new Error('prunable ttlBlocks must be >= 1');
    if (retention.ttlBlocks > MAX_MEDIA_BLOB_TTL_BLOCKS) {
      throw new Error(`prunable ttlBlocks exceeds max ${MAX_MEDIA_BLOB_TTL_BLOCKS}`);
    }
  }
  return blobIdOf(data);
}

export function validateBlobAnnounce(
  blobId: Uint8Array,
  size: number,
  retention: BlobRetentionInput,
  contentType: string,
): void {
  if (blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (size <= 0) throw new Error('announce size must be > 0');
  if (size > MAX_MEDIA_BLOB_BYTES) {
    throw new Error(`announce size exceeds MAX_MEDIA_BLOB_BYTES (${MAX_MEDIA_BLOB_BYTES})`);
  }
  if (contentType.length > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN) {
    throw new Error(`contentType exceeds ${MAX_MEDIA_BLOB_CONTENT_TYPE_LEN} bytes`);
  }
  if (retention.kind === 'prunable') {
    if (retention.ttlBlocks < 1n) throw new Error('prunable ttlBlocks must be >= 1');
    if (retention.ttlBlocks > MAX_MEDIA_BLOB_TTL_BLOCKS) {
      throw new Error(`prunable ttlBlocks exceeds max ${MAX_MEDIA_BLOB_TTL_BLOCKS}`);
    }
  }
}

/** Build a PutBlob payload for `encodeTransaction` / signing. */
export function putBlobPayload(input: {
  retention: BlobRetentionInput;
  contentType?: string;
  data: Uint8Array;
}): TransactionPayloadInput {
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
export function blobAnnouncePayload(input: {
  blobId: Uint8Array;
  size: number;
  retention: BlobRetentionInput;
  contentType?: string;
}): TransactionPayloadInput {
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

/** Max extend_blocks for RenewMediaBlobPin. */
export const MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS = 1_000_000n;
/** Suggested Forever pin horizon for large / video-ish media (docs hint). */
export const MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS = 150_000n;
/** Reed–Solomon parity shard count (matches Rust Phase 5). */
export const MEDIA_BLOB_RS_PARITY_SHARDS = 2;

/** Phase 5: extend pin horizon (`RenewMediaBlobPin`, variant 23). */
export function renewMediaBlobPinPayload(input: {
  blobId: Uint8Array;
  size: number;
  extendBlocks: bigint;
}): TransactionPayloadInput {
  if (input.blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (input.size <= 0 || input.size > MAX_MEDIA_BLOB_BYTES) {
    throw new Error(`renew size out of range (1..=${MAX_MEDIA_BLOB_BYTES})`);
  }
  if (input.extendBlocks < 1n) throw new Error('extendBlocks must be >= 1');
  if (input.extendBlocks > MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS) {
    throw new Error(`extendBlocks exceeds max ${MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS}`);
  }
  return {
    kind: 'renewMediaBlobPin',
    blobId: input.blobId,
    size: input.size,
    extendBlocks: input.extendBlocks,
  };
}

/** Phase 5: pay for one DAS sample (`PayBlobSample`, variant 24). */
export function payBlobSamplePayload(input: {
  blobId: Uint8Array;
  chunkIndex: number;
  chunkCount: number;
}): TransactionPayloadInput {
  if (input.blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (input.chunkCount <= 0 || input.chunkIndex < 0 || input.chunkIndex >= input.chunkCount) {
    throw new Error('chunkIndex out of range');
  }
  return {
    kind: 'payBlobSample',
    blobId: input.blobId,
    chunkIndex: input.chunkIndex,
    chunkCount: input.chunkCount,
  };
}

/**
 * NFT binding: store BlobId as `metadata_hash`, or put `boing://blob/0x{id}` in off-chain JSON
 * and hash that JSON. Prefer JSON + URI for marketplaces (name / attributes alongside media).
 */
export function metadataHashAsBlobId(blobId: Uint8Array): Uint8Array {
  if (blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  return blobId;
}

/** Build token metadata JSON that points `image` at an on-chain blob. */
export function tokenMetadataJsonWithBlobImage(input: {
  name: string;
  description?: string;
  blobId: Uint8Array;
  attributes?: Array<{ trait_type: string; value: string | number }>;
  animationBlobId?: Uint8Array;
}): { json: string; metadataHash: Uint8Array; imageUri: string } {
  const imageUri = mediaBlobUri(input.blobId);
  const doc: Record<string, unknown> = {
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

// --- Phase 6 scaffolds (not consensus-activated) ---

/** Design target for a future multi-minute path (not live). Live cap remains 16 MiB. */
export const PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES = 256 * 1024 * 1024;
export const PHASE6_VIDEO_SEGMENT_TARGET_BYTES = MAX_MEDIA_BLOB_BYTES;
export const PHASE6_MAX_VIDEO_SEGMENTS = 256;
export const PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES = 32;

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
export function suggestedVideoSegmentCount(totalBytes: bigint): number {
  if (totalBytes <= 0n) return 0;
  const seg = BigInt(PHASE6_VIDEO_SEGMENT_TARGET_BYTES);
  return Number((totalBytes + seg - 1n) / seg);
}

export function validateSegmentManifest(m: MediaBlobSegmentManifest): void {
  if (m.segmentBlobIds.length === 0) throw new Error('segment manifest empty');
  if (m.segmentBlobIds.length > PHASE6_MAX_VIDEO_SEGMENTS) {
    throw new Error(`too many segments (max ${PHASE6_MAX_VIDEO_SEGMENTS})`);
  }
  if (m.totalBytes <= 0n) throw new Error('totalBytes must be > 0');
  if (m.totalBytes > BigInt(PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES)) {
    throw new Error('totalBytes exceeds Phase 6 design target (not a live consensus rule)');
  }
  for (const id of m.segmentBlobIds) {
    if (id.length !== 32) throw new Error('segment blobId must be 32 bytes');
  }
}

/** Scaffold matcher: paidFee must cover askFee (pairs with PayBlobSample on-chain). */
export function matchSampleOffer(
  offer: MediaBlobSampleOffer,
  paidFee: bigint,
): MediaBlobSampleOffer & { paidFee: bigint } {
  if (offer.chunkCount <= 0 || offer.chunkIndex < 0 || offer.chunkIndex >= offer.chunkCount) {
    throw new Error('chunkIndex out of range');
  }
  if (paidFee < offer.askFee) {
    throw new Error(`sample fee ${paidFee} below ask ${offer.askFee}`);
  }
  return { ...offer, paidFee };
}

/** Deterministic light-client sample indices (scaffold; not VRF-bound). */
export function dasLightClientSampleIndices(
  blobSize: number,
  sampleCount: number,
  seed: bigint,
): number[] {
  const n = mediaBlobDasChunkCount(blobSize);
  if (n === 0 || sampleCount <= 0) return [];
  const take = Math.min(sampleCount, PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES, n);
  const out: number[] = [];
  let x = seed ^ (BigInt(n) * 0x9e3779b97f4a7c15n);
  for (let i = 0; i < take; i++) {
    x = x * 6364136223846793005n + 1n;
    out.push(Number(x % BigInt(n)));
  }
  return [...new Set(out)].sort((a, b) => a - b);
}
