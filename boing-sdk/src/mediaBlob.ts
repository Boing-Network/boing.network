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

// --- Phase 6 (not consensus-activated; live Put/Announce cap remains 16 MiB) ---

/** Design target for a future multi-minute path (not live). Live cap remains 16 MiB. */
export const PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES = 256 * 1024 * 1024;
export const PHASE6_VIDEO_SEGMENT_TARGET_BYTES = MAX_MEDIA_BLOB_BYTES;
export const PHASE6_MAX_VIDEO_SEGMENTS = 256;
export const PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES = 32;
export const PHASE6_MAX_SAMPLE_OFFERS = 1024;

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

/** Split bytes into segments each ≤ live MAX_MEDIA_BLOB_BYTES; refuses above Phase 6 design target. */
export function splitIntoLiveCapSegments(data: Uint8Array): Uint8Array[] {
  if (data.length === 0) throw new Error('segment data empty');
  if (data.length > PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES) {
    throw new Error('total exceeds Phase 6 design target (not a live consensus rule)');
  }
  const out: Uint8Array[] = [];
  for (let i = 0; i < data.length; i += PHASE6_VIDEO_SEGMENT_TARGET_BYTES) {
    out.push(data.subarray(i, Math.min(i + PHASE6_VIDEO_SEGMENT_TARGET_BYTES, data.length)));
  }
  if (out.length > PHASE6_MAX_VIDEO_SEGMENTS) {
    throw new Error(`too many segments (max ${PHASE6_MAX_VIDEO_SEGMENTS})`);
  }
  return out;
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

/** Build manifest from segment bytes (each segment must be ≤ live 16 MiB). */
export function manifestFromSegmentBytes(
  contentType: string,
  segments: Uint8Array[],
): MediaBlobSegmentManifest {
  if (segments.length === 0) throw new Error('segment manifest empty');
  let total = 0n;
  const ids: Uint8Array[] = [];
  for (const seg of segments) {
    if (seg.length === 0) throw new Error('empty segment');
    if (seg.length > MAX_MEDIA_BLOB_BYTES) {
      throw new Error(`segment exceeds live MAX_MEDIA_BLOB_BYTES (${MAX_MEDIA_BLOB_BYTES})`);
    }
    total += BigInt(seg.length);
    ids.push(blobIdOf(seg));
  }
  const m = { contentType, totalBytes: total, segmentBlobIds: ids };
  validateSegmentManifest(m);
  return m;
}

/** Content-addressed id of an ordered segment manifest. */
export function segmentManifestId(m: MediaBlobSegmentManifest): Uint8Array {
  validateSegmentManifest(m);
  const domain = new TextEncoder().encode('BOING_BLOB_SEG_MANIFEST_V0');
  const total = new Uint8Array(8);
  new DataView(total.buffer).setBigUint64(0, m.totalBytes, true);
  const ct = new TextEncoder().encode(m.contentType);
  const count = new Uint8Array(4);
  new DataView(count.buffer).setUint32(0, m.segmentBlobIds.length, true);
  const parts = [domain, total, ct, count, ...m.segmentBlobIds];
  let len = 0;
  for (const p of parts) len += p.length;
  const buf = new Uint8Array(len);
  let o = 0;
  for (const p of parts) {
    buf.set(p, o);
    o += p.length;
  }
  return blake3(buf);
}

/** Match helper: paidFee must cover askFee (pairs with PayBlobSample on-chain). */
export function matchSampleOffer(
  offer: MediaBlobSampleOffer,
  paidFee: bigint,
  tipHeight: bigint = 0n,
): MediaBlobSampleOffer & { paidFee: bigint; matchedAtHeight: bigint } {
  if (offer.chunkCount <= 0 || offer.chunkIndex < 0 || offer.chunkIndex >= offer.chunkCount) {
    throw new Error('chunkIndex out of range');
  }
  if (offer.expiresAtHeight > 0n && tipHeight > offer.expiresAtHeight) {
    throw new Error('sample offer expired');
  }
  if (paidFee < offer.askFee) {
    throw new Error(`sample fee ${paidFee} below ask ${offer.askFee}`);
  }
  return { ...offer, paidFee, matchedAtHeight: tipHeight };
}

/** Height-bound deterministic seed (matches Rust `das_light_client_seed`). */
export function dasLightClientSeed(blobId: Uint8Array, tipHeight: bigint): bigint {
  if (blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  const domain = new TextEncoder().encode('BOING_BLOB_DAS_LC_V0');
  const h = new Uint8Array(8);
  new DataView(h.buffer).setBigUint64(0, tipHeight, true);
  const buf = new Uint8Array(domain.length + 32 + 8);
  buf.set(domain, 0);
  buf.set(blobId, domain.length);
  buf.set(h, domain.length + 32);
  const dig = blake3(buf);
  const view = new DataView(dig.buffer, dig.byteOffset, 8);
  return view.getBigUint64(0, true);
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
  let uniq = [...new Set(out)].sort((a, b) => a - b);
  let fill = seed + 1n;
  while (uniq.length < take) {
    fill = fill * 6364136223846793005n + 1n;
    const i = Number(fill % BigInt(n));
    if (!uniq.includes(i)) uniq.push(i);
    if (fill === seed) break;
  }
  return uniq.sort((a, b) => a - b);
}


/** Phase 7: register on-chain sample offer (`RegisterBlobSampleOffer`, variant 25). */
export function registerBlobSampleOfferPayload(input: {
  blobId: Uint8Array;
  chunkIndex: number;
  chunkCount: number;
  askFee: bigint;
  expiresAtHeight?: bigint;
}): TransactionPayloadInput {
  if (input.blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (input.chunkCount <= 0 || input.chunkIndex < 0 || input.chunkIndex >= input.chunkCount) {
    throw new Error('chunkIndex out of range');
  }
  if (input.askFee < 0n) throw new Error('askFee must be >= 0');
  return {
    kind: 'registerBlobSampleOffer',
    blobId: input.blobId,
    chunkIndex: input.chunkIndex,
    chunkCount: input.chunkCount,
    askFee: input.askFee,
    expiresAtHeight: input.expiresAtHeight ?? 0n,
  };
}

/** Phase 7: settle sample match (`SettleBlobSampleMatch`, variant 26). */
export function settleBlobSampleMatchPayload(input: {
  blobId: Uint8Array;
  chunkIndex: number;
  chunkCount: number;
  provider: Uint8Array;
}): TransactionPayloadInput {
  if (input.blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (input.provider.length !== 32) throw new Error('provider must be 32 bytes');
  if (input.chunkCount <= 0 || input.chunkIndex < 0 || input.chunkIndex >= input.chunkCount) {
    throw new Error('chunkIndex out of range');
  }
  return {
    kind: 'settleBlobSampleMatch',
    blobId: input.blobId,
    chunkIndex: input.chunkIndex,
    chunkCount: input.chunkCount,
    provider: input.provider,
  };
}

/** Phase 7 design / activated max (only live after cap activation height). */
export const PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES;

/** Effective max bytes: raised only when `phase7CapActive`. Default = live 16 MiB. */
export function mediaBlobMaxBytesAt(phase7CapActive: boolean): number {
  return phase7CapActive ? PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES : MAX_MEDIA_BLOB_BYTES;
}

/** VRF-bound DAS seed (matches Rust `das_light_client_seed_from_vrf`). Caller must verify VRF. */
export function dasLightClientSeedFromVrf(blobId: Uint8Array, vrfOutput: Uint8Array): bigint {
  if (blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (vrfOutput.length !== 32) throw new Error('vrfOutput must be 32 bytes');
  const domain = new TextEncoder().encode('BOING_BLOB_DAS_VRF_SEED_V0');
  const lc = new TextEncoder().encode('BOING_BLOB_DAS_LC_V0');
  const buf = new Uint8Array(domain.length + lc.length + 64);
  let o = 0;
  buf.set(domain, o); o += domain.length;
  buf.set(lc, o); o += lc.length;
  buf.set(blobId, o); o += 32;
  buf.set(vrfOutput, o);
  const dig = blake3(buf);
  return new DataView(dig.buffer, dig.byteOffset, 8).getBigUint64(0, true);
}

/** Honest succinct commitment stub (not a SNARK). */
export function dasSuccinctCommitmentStub(input: {
  blobId: Uint8Array;
  chunksRoot: Uint8Array;
  seedHeight: bigint;
  samples: Array<{ chunkIndex: number; chunkHash: Uint8Array }>;
}): Uint8Array {
  const domain = new TextEncoder().encode('BOING_BLOB_DAS_SUCCINCT_STUB_V0');
  const h = new Uint8Array(8);
  new DataView(h.buffer).setBigUint64(0, input.seedHeight, true);
  const count = new Uint8Array(4);
  new DataView(count.buffer).setUint32(0, input.samples.length, true);
  const parts: Uint8Array[] = [domain, input.blobId, input.chunksRoot, h, count];
  for (const s of input.samples) {
    const idx = new Uint8Array(4);
    new DataView(idx.buffer).setUint32(0, s.chunkIndex >>> 0, true);
    parts.push(idx, s.chunkHash);
  }
  let len = 0;
  for (const p of parts) len += p.length;
  const buf = new Uint8Array(len);
  let o = 0;
  for (const p of parts) {
    buf.set(p, o);
    o += p.length;
  }
  return blake3(buf);
}

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

export function quoteBlobSampleSettle(input: {
  blobId: Uint8Array;
  chunkIndex: number;
  chunkCount: number;
  provider: Uint8Array;
  askFee: bigint;
  protocolSampleFee: bigint;
}): BlobSampleSettleQuote {
  if (input.blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (input.provider.length !== 32) throw new Error('provider must be 32 bytes');
  if (input.askFee < 0n || input.protocolSampleFee < 0n) {
    throw new Error('fees must be >= 0');
  }
  return {
    protocolSampleFee: input.protocolSampleFee,
    askFee: input.askFee,
    total: input.protocolSampleFee + input.askFee,
    provider: input.provider,
    blobId: input.blobId,
    chunkIndex: input.chunkIndex,
    chunkCount: input.chunkCount,
  };
}

/** Phase 8 succinct proof kind: 0 = StubBlake3V0 (not a SNARK), 1 = reserved. */
export const DAS_SUCCINCT_PROOF_KIND_STUB_BLAKE3_V0 = 0;
export const DAS_SUCCINCT_PROOF_KIND_RESERVED_REAL_V1 = 1;

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
export function buildDasSuccinctProofStub(input: {
  blobId: Uint8Array;
  chunksRoot: Uint8Array;
  seedHeight: bigint;
  samples: Array<{ chunkIndex: number; chunkHash: Uint8Array }>;
}): DasSuccinctProof {
  const commitment = dasSuccinctCommitmentStub(input);
  return {
    kind: DAS_SUCCINCT_PROOF_KIND_STUB_BLAKE3_V0,
    commitment,
    blobId: input.blobId,
    chunksRoot: input.chunksRoot,
    seedHeight: input.seedHeight,
    sampleIndices: input.samples.map((s) => s.chunkIndex),
  };
}

/** VRF-bound stub envelope (matches Rust `build_das_succinct_proof_stub_vrf`). */
export function buildDasSuccinctProofStubVrf(
  input: {
    blobId: Uint8Array;
    chunksRoot: Uint8Array;
    samples: Array<{ chunkIndex: number; chunkHash: Uint8Array }>;
  },
  vrfOutput: Uint8Array,
): DasSuccinctProof {
  if (vrfOutput.length !== 32) throw new Error('vrfOutput must be 32 bytes');
  const base = buildDasSuccinctProofStub({
    ...input,
    seedHeight: 0n,
  });
  const domain = new TextEncoder().encode('BOING_BLOB_DAS_SUCCINCT_PROOF_V0');
  const buf = new Uint8Array(domain.length + 1 + 32 + 32);
  let o = 0;
  buf.set(domain, o);
  o += domain.length;
  buf[o++] = DAS_SUCCINCT_PROOF_KIND_STUB_BLAKE3_V0;
  buf.set(base.commitment, o);
  o += 32;
  buf.set(vrfOutput, o);
  return {
    ...base,
    seedHeight: 0n,
    vrfOutput,
    commitment: blake3(buf),
  };
}

/** Verify StubBlake3V0 against expected commitment; reserved kinds always fail. */
export function verifyDasSuccinctProofStub(
  proof: DasSuccinctProof,
  expected: DasSuccinctProof,
): boolean {
  if (proof.kind !== DAS_SUCCINCT_PROOF_KIND_STUB_BLAKE3_V0) return false;
  if (expected.kind !== DAS_SUCCINCT_PROOF_KIND_STUB_BLAKE3_V0) return false;
  if (proof.commitment.length !== 32 || expected.commitment.length !== 32) return false;
  if (proof.blobId.length !== 32 || expected.blobId.length !== 32) return false;
  if (
    proof.sampleIndices.length !== expected.sampleIndices.length ||
    proof.sampleIndices.some((v, i) => v !== expected.sampleIndices[i])
  ) {
    return false;
  }
  for (let i = 0; i < 32; i++) {
    if (proof.commitment[i] !== expected.commitment[i]) return false;
    if (proof.blobId[i] !== expected.blobId[i]) return false;
    if (proof.chunksRoot[i] !== expected.chunksRoot[i]) return false;
  }
  return proof.seedHeight === expected.seedHeight;
}

/** Alias: activated max after coordinated cap raise. */
export const PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/** Phase 9: cancel sample offer and refund SMT escrow (`CancelBlobSampleOffer`, variant 27). */
export function cancelBlobSampleOfferPayload(input: {
  blobId: Uint8Array;
  chunkIndex: number;
}): TransactionPayloadInput {
  if (input.blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (input.chunkIndex < 0) throw new Error('chunkIndex out of range');
  return {
    kind: 'cancelBlobSampleOffer',
    blobId: input.blobId,
    chunkIndex: input.chunkIndex,
  };
}

/** Phase 9 activated max alias. */
export const PHASE9_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/** Domain tag for ask-fee escrow account (BLAKE3 → AccountId). */
export const PHASE9_ASK_ESCROW_DOMAIN = 'BOING_BLOB_SAMPLE_ASK_ESCROW_V1';

/** System ask-escrow account id (matches Rust `blob_sample_ask_escrow_account`). */
export function blobSampleAskEscrowAccount(): Uint8Array {
  return blake3(new TextEncoder().encode(PHASE9_ASK_ESCROW_DOMAIN));
}

/** DAS LC seed from consensus leader VRF output (same mixer as `dasLightClientSeedFromVrf`). */
export function dasLightClientSeedFromLeaderVrf(
  blobId: Uint8Array,
  leaderVrfOutput: Uint8Array,
): bigint {
  return dasLightClientSeedFromVrf(blobId, leaderVrfOutput);
}

/** Phase 10: challenge sample serve (`ChallengeBlobSampleServe`, variant 28). */
export function challengeBlobSampleServePayload(input: {
  blobId: Uint8Array;
  chunkIndex: number;
  chunkCount: number;
  provider: Uint8Array;
}): TransactionPayloadInput {
  if (input.blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (input.provider.length !== 32) throw new Error('provider must be 32 bytes');
  if (input.chunkCount <= 0 || input.chunkIndex < 0 || input.chunkIndex >= input.chunkCount) {
    throw new Error('chunkIndex out of range');
  }
  return {
    kind: 'challengeBlobSampleServe',
    blobId: input.blobId,
    chunkIndex: input.chunkIndex,
    chunkCount: input.chunkCount,
    provider: input.provider,
  };
}

/** Phase 10: respond to serve challenge with DAS merkle sample (variant 29). */
export function respondBlobSampleServePayload(input: {
  blobId: Uint8Array;
  chunkIndex: number;
  chunkCount: number;
  challenger: Uint8Array;
  chunkHash: Uint8Array;
  chunksRoot: Uint8Array;
  siblings: Uint8Array[];
}): TransactionPayloadInput {
  if (input.blobId.length !== 32) throw new Error('blobId must be 32 bytes');
  if (input.challenger.length !== 32) throw new Error('challenger must be 32 bytes');
  if (input.chunkHash.length !== 32 || input.chunksRoot.length !== 32) {
    throw new Error('chunkHash/chunksRoot must be 32 bytes');
  }
  return {
    kind: 'respondBlobSampleServe',
    blobId: input.blobId,
    chunkIndex: input.chunkIndex,
    chunkCount: input.chunkCount,
    challenger: input.challenger,
    chunkHash: input.chunkHash,
    chunksRoot: input.chunksRoot,
    siblings: input.siblings,
  };
}

/** Phase 10 serve-challenge window (blocks). */
export const PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS = 256n;

/** Phase 10 activated max alias. */
export const PHASE10_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE9_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/** Honest plan strings for succinct DAS path (matches Rust Phase 10). */
export function phase10DasSuccinctProofPlan(): string[] {
  return [
    'StubBlake3V0: BLAKE3 commitment over merkle LC batch — implemented; not a SNARK.',
    'ReservedRealV1: opaque proof envelope accepted for encoding/RPC scaffolding; verify always rejects.',
    'Next: wire IPA/KZG/SNARK verifier behind ReservedRealV1 and flip is_implemented when production-ready.',
    'Leader-VRF LC: use boing_getLeaderVrfOutput / useLeaderVrf when consensus is in VRF mode.',
  ];
}

/** Phase 11 serve-challenge window (same as Phase 10). */
export const PHASE11_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS;

/** Phase 11 activated max alias. */
export const PHASE11_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE10_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/** Domain tag for serve-challenge bond escrow account (BLAKE3 → AccountId). */
export const PHASE11_SERVE_CHALLENGE_BOND_ESCROW_DOMAIN = 'BOING_BLOB_SERVE_CHALLENGE_BOND_V1';

/** Challenger bond + provider penalty amounts (1 BOING = 10^18 base units). */
export const PHASE11_SERVE_CHALLENGE_BOND = 10n ** 18n;
export const PHASE11_FAILED_SERVE_PENALTY = 10n ** 18n;

/** System serve-challenge bond escrow account id. */
export function blobSampleServeChallengeBondEscrowAccount(): Uint8Array {
  return blake3(new TextEncoder().encode(PHASE11_SERVE_CHALLENGE_BOND_ESCROW_DOMAIN));
}

/** Honest plan strings for succinct DAS path (Phase 11 progress; verify still rejects). */
export function phase11DasSuccinctProofPlan(): string[] {
  return [
    ...phase10DasSuccinctProofPlan(),
    'Phase 11: ReservedRealV1 envelope structure checks (kind/size/commitment rebind) — still not availability evidence.',
    'ReservedRealV1 verify remains reject-until-wired; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
  ];
}

/** Phase 12 aliases / constants. */
export const PHASE12_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE11_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE12_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE11_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE12_SERVE_CHALLENGE_BOND = PHASE11_SERVE_CHALLENGE_BOND;
export const PHASE12_FAILED_SERVE_PENALTY = PHASE11_FAILED_SERVE_PENALTY;
export const PHASE12_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = 1000n;
export const PHASE12_SETTLE_RECEIPT_DOMAIN = 'BOING_BLOB_SETTLE_RECEIPT_V1';

/** Honest plan strings for Phase 12 (settle-before-challenge + FailedServe audit). */
export function phase12DasSuccinctProofPlan(): string[] {
  return [
    ...phase11DasSuccinctProofPlan(),
    'Phase 12: FailedServe assigns consensus slash_id and mirrors SlashRegistry (BlobFailedServe); balance burn remains the economic slash.',
    'Phase 12: Challenge requires prior SettleBlobSampleMatch receipt (payer = challenger).',
  ];
}
