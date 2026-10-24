/**
 * L1 media blobs — content-addressed PutBlob / BlobAnnounce helpers (not the interface SSTORE store).
 * @see docs/BOING-L1-MEDIA-BLOBS.md
 */
import * as ed25519 from '@noble/ed25519';
import { blake3 } from '@noble/hashes/blake3';
import { bytesToHex, ensureHex, hexToBytes } from './hex.js';
function u64Le(n) {
    const b = new Uint8Array(8);
    new DataView(b.buffer).setBigUint64(0, BigInt(n), true);
    return b;
}
function concatBytes(...parts) {
    const len = parts.reduce((a, p) => a + p.length, 0);
    const out = new Uint8Array(len);
    let o = 0;
    for (const p of parts) {
        out.set(p, o);
        o += p.length;
    }
    return out;
}
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
export const MAX_MEDIA_BLOB_TTL_BLOCKS = 1000000n;
export const MEDIA_BLOB_GAS_BASE = 21000n;
export const MEDIA_BLOB_GAS_PER_WORD = 16n;
export const MEDIA_BLOB_FOREVER_MULTIPLIER = 10n;
export const MEDIA_BLOB_URI_PREFIX = 'boing://blob/';
export const MEDIA_BLOBS_GOSSIP_TOPIC = 'boing/media-blobs';
/** Ceil chunk count for DAS / XOR erasure stubs. */
export function mediaBlobDasChunkCount(size) {
    if (size <= 0)
        return 0;
    return Math.ceil(size / MEDIA_BLOB_DAS_CHUNK_BYTES);
}
/** Forever pin-rent units (KiB × blocks) for off-chain markets; on-chain uses `blob_forever_pin_fee_per_kib`. */
export function mediaBlobPinRentKibBlocks(size, ttlBlocks) {
    const kib = BigInt(Math.max(1, Math.ceil(size / 1024)));
    return kib * ttlBlocks;
}
function chunkHash(bytes) {
    return blake3(bytes);
}
function hashPair(a, b) {
    const domain = new TextEncoder().encode('BOING_BLOB_CHUNK_MERKLE_V1');
    const buf = new Uint8Array(domain.length + 64);
    buf.set(domain, 0);
    buf.set(a, domain.length);
    buf.set(b, domain.length + 32);
    return blake3(buf);
}
/** Binary merkle root over 32-byte chunk hashes (matches Rust `media_blob_chunks_merkle_root`). */
export function mediaBlobChunksMerkleRoot(leaves) {
    if (leaves.length === 0)
        return new Uint8Array(32);
    let layer = leaves.map((l) => new Uint8Array(l));
    while (layer.length > 1) {
        const next = [];
        for (let i = 0; i < layer.length; i += 2) {
            const left = layer[i];
            const right = i + 1 < layer.length ? layer[i + 1] : left;
            next.push(new Uint8Array(hashPair(left, right)));
        }
        layer = next;
    }
    return layer[0];
}
/** Split into DAS-sized chunks + XOR parity (recover one missing data chunk). */
export function chunkMediaBlob(data) {
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
    const dataChunks = [];
    for (let off = 0; off < data.length; off += MEDIA_BLOB_DAS_CHUNK_BYTES) {
        const slice = data.subarray(off, Math.min(off + MEDIA_BLOB_DAS_CHUNK_BYTES, data.length));
        const padded = new Uint8Array(MEDIA_BLOB_DAS_CHUNK_BYTES);
        padded.set(slice);
        dataChunks.push(padded);
    }
    const parityChunk = new Uint8Array(MEDIA_BLOB_DAS_CHUNK_BYTES);
    for (const ch of dataChunks) {
        for (let i = 0; i < MEDIA_BLOB_DAS_CHUNK_BYTES; i++)
            parityChunk[i] ^= ch[i];
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
export function recoverMediaBlobXor(blobId, originalLen, dataChunks, parity) {
    if (originalLen === 0) {
        const empty = new Uint8Array(0);
        if (!bytesEqual(blobIdOf(empty), blobId))
            throw new Error('recovered bytes do not match blob id');
        return empty;
    }
    const missing = dataChunks.map((c, i) => (c == null ? i : -1)).filter((i) => i >= 0);
    let chunks;
    if (missing.length === 0) {
        chunks = dataChunks.map((c) => c);
    }
    else if (missing.length === 1) {
        if (parity.length !== MEDIA_BLOB_DAS_CHUNK_BYTES)
            throw new Error('cannot recover blob');
        const miss = missing[0];
        const recovered = Uint8Array.from(parity);
        for (let i = 0; i < dataChunks.length; i++) {
            if (i === miss)
                continue;
            const bytes = dataChunks[i];
            if (!bytes || bytes.length !== MEDIA_BLOB_DAS_CHUNK_BYTES)
                throw new Error('cannot recover blob');
            for (let j = 0; j < MEDIA_BLOB_DAS_CHUNK_BYTES; j++)
                recovered[j] ^= bytes[j];
        }
        chunks = dataChunks.map((c, i) => (i === miss ? recovered : c));
    }
    else {
        throw new Error('cannot recover blob');
    }
    const out = new Uint8Array(originalLen);
    let off = 0;
    for (const ch of chunks) {
        const n = Math.min(ch.length, originalLen - off);
        out.set(ch.subarray(0, n), off);
        off += n;
        if (off >= originalLen)
            break;
    }
    if (!bytesEqual(blobIdOf(out), blobId))
        throw new Error('recovered bytes do not match blob id');
    return out;
}
function bytesEqual(a, b) {
    if (a.length !== b.length)
        return false;
    for (let i = 0; i < a.length; i++)
        if (a[i] !== b[i])
            return false;
    return true;
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
/** Max extend_blocks for RenewMediaBlobPin. */
export const MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS = 1000000n;
/** Suggested Forever pin horizon for large / video-ish media (docs hint). */
export const MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS = 150000n;
/** Reed–Solomon parity shard count (matches Rust Phase 5). */
export const MEDIA_BLOB_RS_PARITY_SHARDS = 2;
/** Phase 5: extend pin horizon (`RenewMediaBlobPin`, variant 23). */
export function renewMediaBlobPinPayload(input) {
    if (input.blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (input.size <= 0 || input.size > MAX_MEDIA_BLOB_BYTES) {
        throw new Error(`renew size out of range (1..=${MAX_MEDIA_BLOB_BYTES})`);
    }
    if (input.extendBlocks < 1n)
        throw new Error('extendBlocks must be >= 1');
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
export function payBlobSamplePayload(input) {
    if (input.blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
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
// --- Phase 6 (not consensus-activated; live Put/Announce cap remains 16 MiB) ---
/** Design target for a future multi-minute path (not live). Live cap remains 16 MiB. */
export const PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES = 256 * 1024 * 1024;
export const PHASE6_VIDEO_SEGMENT_TARGET_BYTES = MAX_MEDIA_BLOB_BYTES;
export const PHASE6_MAX_VIDEO_SEGMENTS = 256;
export const PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES = 32;
export const PHASE6_MAX_SAMPLE_OFFERS = 1024;
/** Suggested segment count if splitting totalBytes into live-cap chunks. */
export function suggestedVideoSegmentCount(totalBytes) {
    if (totalBytes <= 0n)
        return 0;
    const seg = BigInt(PHASE6_VIDEO_SEGMENT_TARGET_BYTES);
    return Number((totalBytes + seg - 1n) / seg);
}
/** Split bytes into segments each ≤ live MAX_MEDIA_BLOB_BYTES; refuses above Phase 6 design target. */
export function splitIntoLiveCapSegments(data) {
    if (data.length === 0)
        throw new Error('segment data empty');
    if (data.length > PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES) {
        throw new Error('total exceeds Phase 6 design target (not a live consensus rule)');
    }
    const out = [];
    for (let i = 0; i < data.length; i += PHASE6_VIDEO_SEGMENT_TARGET_BYTES) {
        out.push(data.subarray(i, Math.min(i + PHASE6_VIDEO_SEGMENT_TARGET_BYTES, data.length)));
    }
    if (out.length > PHASE6_MAX_VIDEO_SEGMENTS) {
        throw new Error(`too many segments (max ${PHASE6_MAX_VIDEO_SEGMENTS})`);
    }
    return out;
}
export function validateSegmentManifest(m) {
    if (m.segmentBlobIds.length === 0)
        throw new Error('segment manifest empty');
    if (m.segmentBlobIds.length > PHASE6_MAX_VIDEO_SEGMENTS) {
        throw new Error(`too many segments (max ${PHASE6_MAX_VIDEO_SEGMENTS})`);
    }
    if (m.totalBytes <= 0n)
        throw new Error('totalBytes must be > 0');
    if (m.totalBytes > BigInt(PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES)) {
        throw new Error('totalBytes exceeds Phase 6 design target (not a live consensus rule)');
    }
    for (const id of m.segmentBlobIds) {
        if (id.length !== 32)
            throw new Error('segment blobId must be 32 bytes');
    }
}
/** Build manifest from segment bytes (each segment must be ≤ live 16 MiB). */
export function manifestFromSegmentBytes(contentType, segments) {
    if (segments.length === 0)
        throw new Error('segment manifest empty');
    let total = 0n;
    const ids = [];
    for (const seg of segments) {
        if (seg.length === 0)
            throw new Error('empty segment');
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
export function segmentManifestId(m) {
    validateSegmentManifest(m);
    const domain = new TextEncoder().encode('BOING_BLOB_SEG_MANIFEST_V0');
    const total = new Uint8Array(8);
    new DataView(total.buffer).setBigUint64(0, m.totalBytes, true);
    const ct = new TextEncoder().encode(m.contentType);
    const count = new Uint8Array(4);
    new DataView(count.buffer).setUint32(0, m.segmentBlobIds.length, true);
    const parts = [domain, total, ct, count, ...m.segmentBlobIds];
    let len = 0;
    for (const p of parts)
        len += p.length;
    const buf = new Uint8Array(len);
    let o = 0;
    for (const p of parts) {
        buf.set(p, o);
        o += p.length;
    }
    return blake3(buf);
}
/** Match helper: paidFee must cover askFee (pairs with PayBlobSample on-chain). */
export function matchSampleOffer(offer, paidFee, tipHeight = 0n) {
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
export function dasLightClientSeed(blobId, tipHeight) {
    if (blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
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
export function dasLightClientSampleIndices(blobSize, sampleCount, seed) {
    const n = mediaBlobDasChunkCount(blobSize);
    if (n === 0 || sampleCount <= 0)
        return [];
    const take = Math.min(sampleCount, PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES, n);
    const out = [];
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
        if (!uniq.includes(i))
            uniq.push(i);
        if (fill === seed)
            break;
    }
    return uniq.sort((a, b) => a - b);
}
/** Phase 7: register on-chain sample offer (`RegisterBlobSampleOffer`, variant 25). */
export function registerBlobSampleOfferPayload(input) {
    if (input.blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (input.chunkCount <= 0 || input.chunkIndex < 0 || input.chunkIndex >= input.chunkCount) {
        throw new Error('chunkIndex out of range');
    }
    if (input.askFee < 0n)
        throw new Error('askFee must be >= 0');
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
export function settleBlobSampleMatchPayload(input) {
    if (input.blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (input.provider.length !== 32)
        throw new Error('provider must be 32 bytes');
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
export function mediaBlobMaxBytesAt(phase7CapActive) {
    return phase7CapActive ? PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES : MAX_MEDIA_BLOB_BYTES;
}
/** VRF-bound DAS seed (matches Rust `das_light_client_seed_from_vrf`). Caller must verify VRF. */
export function dasLightClientSeedFromVrf(blobId, vrfOutput) {
    if (blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (vrfOutput.length !== 32)
        throw new Error('vrfOutput must be 32 bytes');
    const domain = new TextEncoder().encode('BOING_BLOB_DAS_VRF_SEED_V0');
    const lc = new TextEncoder().encode('BOING_BLOB_DAS_LC_V0');
    const buf = new Uint8Array(domain.length + lc.length + 64);
    let o = 0;
    buf.set(domain, o);
    o += domain.length;
    buf.set(lc, o);
    o += lc.length;
    buf.set(blobId, o);
    o += 32;
    buf.set(vrfOutput, o);
    const dig = blake3(buf);
    return new DataView(dig.buffer, dig.byteOffset, 8).getBigUint64(0, true);
}
/** Honest succinct commitment stub (not a SNARK). */
export function dasSuccinctCommitmentStub(input) {
    const domain = new TextEncoder().encode('BOING_BLOB_DAS_SUCCINCT_STUB_V0');
    const h = new Uint8Array(8);
    new DataView(h.buffer).setBigUint64(0, input.seedHeight, true);
    const count = new Uint8Array(4);
    new DataView(count.buffer).setUint32(0, input.samples.length, true);
    const parts = [domain, input.blobId, input.chunksRoot, h, count];
    for (const s of input.samples) {
        const idx = new Uint8Array(4);
        new DataView(idx.buffer).setUint32(0, s.chunkIndex >>> 0, true);
        parts.push(idx, s.chunkHash);
    }
    let len = 0;
    for (const p of parts)
        len += p.length;
    const buf = new Uint8Array(len);
    let o = 0;
    for (const p of parts) {
        buf.set(p, o);
        o += p.length;
    }
    return blake3(buf);
}
export function quoteBlobSampleSettle(input) {
    if (input.blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (input.provider.length !== 32)
        throw new Error('provider must be 32 bytes');
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
/** Build StubBlake3V0 envelope (matches Rust `build_das_succinct_proof_stub`). */
export function buildDasSuccinctProofStub(input) {
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
export function buildDasSuccinctProofStubVrf(input, vrfOutput) {
    if (vrfOutput.length !== 32)
        throw new Error('vrfOutput must be 32 bytes');
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
export function verifyDasSuccinctProofStub(proof, expected) {
    if (proof.kind !== DAS_SUCCINCT_PROOF_KIND_STUB_BLAKE3_V0)
        return false;
    if (expected.kind !== DAS_SUCCINCT_PROOF_KIND_STUB_BLAKE3_V0)
        return false;
    if (proof.commitment.length !== 32 || expected.commitment.length !== 32)
        return false;
    if (proof.blobId.length !== 32 || expected.blobId.length !== 32)
        return false;
    if (proof.sampleIndices.length !== expected.sampleIndices.length ||
        proof.sampleIndices.some((v, i) => v !== expected.sampleIndices[i])) {
        return false;
    }
    for (let i = 0; i < 32; i++) {
        if (proof.commitment[i] !== expected.commitment[i])
            return false;
        if (proof.blobId[i] !== expected.blobId[i])
            return false;
        if (proof.chunksRoot[i] !== expected.chunksRoot[i])
            return false;
    }
    return proof.seedHeight === expected.seedHeight;
}
/** Alias: activated max after coordinated cap raise. */
export const PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
/** Phase 9: cancel sample offer and refund SMT escrow (`CancelBlobSampleOffer`, variant 27). */
export function cancelBlobSampleOfferPayload(input) {
    if (input.blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (input.chunkIndex < 0)
        throw new Error('chunkIndex out of range');
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
export function blobSampleAskEscrowAccount() {
    return blake3(new TextEncoder().encode(PHASE9_ASK_ESCROW_DOMAIN));
}
/** DAS LC seed from consensus leader VRF output (same mixer as `dasLightClientSeedFromVrf`). */
export function dasLightClientSeedFromLeaderVrf(blobId, leaderVrfOutput) {
    return dasLightClientSeedFromVrf(blobId, leaderVrfOutput);
}
/** Phase 10: challenge sample serve (`ChallengeBlobSampleServe`, variant 28). */
export function challengeBlobSampleServePayload(input) {
    if (input.blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (input.provider.length !== 32)
        throw new Error('provider must be 32 bytes');
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
export function respondBlobSampleServePayload(input) {
    if (input.blobId.length !== 32)
        throw new Error('blobId must be 32 bytes');
    if (input.challenger.length !== 32)
        throw new Error('challenger must be 32 bytes');
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
export function phase10DasSuccinctProofPlan() {
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
export function blobSampleServeChallengeBondEscrowAccount() {
    return blake3(new TextEncoder().encode(PHASE11_SERVE_CHALLENGE_BOND_ESCROW_DOMAIN));
}
/** Honest plan strings for succinct DAS path (Phase 11 progress; verify still rejects). */
export function phase11DasSuccinctProofPlan() {
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
export function phase12DasSuccinctProofPlan() {
    return [
        ...phase11DasSuccinctProofPlan(),
        'Phase 12: FailedServe assigns consensus slash_id and mirrors SlashRegistry (BlobFailedServe); balance burn remains the economic slash.',
        'Phase 12: Challenge requires prior SettleBlobSampleMatch receipt (payer = challenger).',
    ];
}
/** Phase 13 aliases / constants. */
export const PHASE13_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE12_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE13_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE12_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE13_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE12_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE13_SETTLE_RECEIPT_TTL_BLOCKS = 4096n;
/** Honest plan strings for Phase 13 (FailedServe balance unburn + receipt TTL). */
export function phase13DasSuccinctProofPlan() {
    return [
        ...phase12DasSuccinctProofPlan(),
        'Phase 13: approved BlobFailedServe appeal restores provider native balance from FEE_BURN_SINK (not stake).',
        'Phase 13: settle receipts TTL-prune and are consumed when a serve challenge closes (Respond / FailedServe).',
    ];
}
/** Phase 14 aliases / constants. */
export const PHASE14_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE13_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE14_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE13_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE13_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE14_SETTLE_RECEIPT_TTL_BLOCKS = PHASE13_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE14_MAX_APPEAL_EVIDENCE_BYTES = 4096;
export const PHASE14_SUBMIT_SLASH_APPEAL_TX_INDEX = 30;
export const PHASE14_RESOLVE_SLASH_APPEAL_TX_INDEX = 31;
/** Phase 14: submit consensus slash appeal (`SubmitSlashAppeal`, variant 30). */
export function encodeSubmitSlashAppeal(input) {
    const evidence = input.evidence ?? new Uint8Array();
    if (evidence.length > PHASE14_MAX_APPEAL_EVIDENCE_BYTES) {
        throw new Error(`evidence exceeds max ${PHASE14_MAX_APPEAL_EVIDENCE_BYTES} bytes`);
    }
    return {
        kind: 'submitSlashAppeal',
        slashId: input.slashId,
        evidence,
    };
}
/** Phase 14: resolve consensus slash appeal (`ResolveSlashAppeal`, variant 31). */
export function encodeResolveSlashAppeal(input) {
    return {
        kind: 'resolveSlashAppeal',
        appealId: input.appealId,
        approved: input.approved,
    };
}
/** Phase 18/22: attested resolve (`ResolveSlashAppealAttested`, variant 32). */
export function encodeResolveSlashAppealAttested(input) {
    if (input.ballotSignature.length !== 64) {
        throw new Error('ballotSignature must be 64 bytes');
    }
    return {
        kind: 'resolveSlashAppealAttested',
        appealId: input.appealId,
        approved: input.approved,
        ballotSignature: input.ballotSignature,
    };
}
/**
 * Phase 22: encode attested resolve by signing ballot domain **V2** (binds pin set).
 * `secretKey32` must match the voter AccountId / tx.sender.
 */
export async function encodeResolveSlashAppealAttestedV2(input) {
    const ballotSignature = await signSlashAppealResolveBallotV2({
        chainId: input.chainId,
        appealId: input.appealId,
        slashId: input.slashId,
        approved: input.approved,
        pinSetCommitment: input.pinSetCommitment,
        secretKey32: input.secretKey32,
    });
    return encodeResolveSlashAppealAttested({
        appealId: input.appealId,
        approved: input.approved,
        ballotSignature,
    });
}
/** Honest plan strings for Phase 14 (consensus slash-appeal txs; ReservedRealV1 still rejects). */
export function phase14DasSuccinctProofPlan() {
    return [
        ...phase13DasSuccinctProofPlan(),
        'Phase 14: SubmitSlashAppeal (30) / ResolveSlashAppeal (31) are consensus txs; FailedServe unburn replays with the block.',
        'Phase 14: ResolveSlashAppeal is ops/validator-approved — not SNARK/IPA/KZG verification of appeal evidence.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 15 aliases / constants. */
export const PHASE15_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE14_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE15_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE14_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE15_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE15_SETTLE_RECEIPT_TTL_BLOCKS = PHASE14_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE15_MAX_APPEAL_EVIDENCE_BYTES = PHASE14_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE15_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE14_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE15_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE14_RESOLVE_SLASH_APPEAL_TX_INDEX;
/** Honest plan strings for Phase 15 (consensus equivocation/liveness appeals; still 16 MiB). */
export function phase15DasSuccinctProofPlan() {
    return [
        ...phase14DasSuccinctProofPlan(),
        'Phase 15: equivocation/liveness slashes seed the consensus SlashAppealRegistry; Submit/Resolve (30/31) restore stake on multi-validator replay.',
        'Phase 15: slash detection/apply remains node-local (evidence gossip); appeal submit/resolve txs are consensus-replayable once a row exists.',
        'Phase 15: Submit/Resolve (30/31) are always admitted (not gated on media-blobs activation) so equivocation/liveness appeals work when media is inactive.',
        'Phase 15: ResolveSlashAppeal remains ops/validator-approved — not SNARK/IPA/KZG verification of appeal evidence.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 16 aliases / constants. */
export const PHASE16_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE15_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE16_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE15_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE16_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE15_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE16_SETTLE_RECEIPT_TTL_BLOCKS = PHASE15_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE16_MAX_APPEAL_EVIDENCE_BYTES = PHASE15_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE16_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE15_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE16_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE15_RESOLVE_SLASH_APPEAL_TX_INDEX;
/** Fixed resolve quorum numerator (⌈2M/3⌉ via integer `votes * DEN >= NUM * M`). */
export const PHASE16_RESOLVE_THRESHOLD_NUM = 2;
/** Fixed resolve quorum denominator. */
export const PHASE16_RESOLVE_THRESHOLD_DEN = 3;
/** Honest plan strings for Phase 16 (committee threshold ResolveSlashAppeal; still 16 MiB). */
export function phase16DasSuccinctProofPlan() {
    return [
        ...phase15DasSuccinctProofPlan(),
        'Phase 16: ResolveSlashAppeal (31) is an N-of-M committee vote over host-pinned slash_appeal_resolvers (⌈2M/3⌉ approve or reject).',
        'Phase 16: finalize (restore balance/stake or keep burn) only when the approve or reject threshold is met; one vote per voter; no flip.',
        'Phase 16: when fallback eligibility without a pinned resolver set, M=1 so one eligible vote finalizes (solo-dev UX).',
        'Phase 16: ResolveSlashAppeal remains ops/validator-approved voting — not SNARK/IPA/KZG verification of appeal evidence.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 17 aliases / constants. */
export const PHASE17_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE16_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE17_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE16_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE17_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE16_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE17_SETTLE_RECEIPT_TTL_BLOCKS = PHASE16_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE17_MAX_APPEAL_EVIDENCE_BYTES = PHASE16_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE17_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE16_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE17_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE16_RESOLVE_SLASH_APPEAL_TX_INDEX;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉ via integer `weight * DEN >= NUM * W`). */
export const PHASE17_RESOLVE_THRESHOLD_NUM = 2;
/** Fixed stake-weighted resolve quorum denominator. */
export const PHASE17_RESOLVE_THRESHOLD_DEN = 3;
/** Honest plan strings for Phase 17 (stake-weighted ResolveSlashAppeal + versioned registry; still 16 MiB). */
export function phase17DasSuccinctProofPlan() {
    return [
        ...phase16DasSuccinctProofPlan(),
        'Phase 17: ResolveSlashAppeal (31) is stake-weighted ⌈2W/3⌉ over host-pinned slash_appeal_resolvers (weight = bonded stake at cast).',
        'Phase 17: finalize (restore balance/stake or keep burn) only when approve or reject weight meets the threshold; one vote per voter; no flip.',
        'Phase 17: when fallback eligibility without a pinned resolver set, effective W=1 so one eligible vote finalizes (solo-dev UX).',
        'Phase 17: W=0 with a non-empty pinned set never finalizes — pin bonded resolvers or rebond.',
        'Phase 17: slash_appeals_registry.bin is versioned (magic+u32+bincode); legacy Phase 14–16 loads migrate without required clear.',
        'Phase 17: ResolveSlashAppeal remains ops/validator-approved voting — not SNARK/IPA/KZG verification of appeal evidence.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 18 aliases / constants. */
export const PHASE18_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE17_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE18_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE17_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE18_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE17_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE18_SETTLE_RECEIPT_TTL_BLOCKS = PHASE17_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE18_MAX_APPEAL_EVIDENCE_BYTES = PHASE17_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE18_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE17_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE18_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE17_RESOLVE_SLASH_APPEAL_TX_INDEX;
/** Attested resolve payload index (`ResolveSlashAppealAttested`). */
export const PHASE18_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = 32;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉). */
export const PHASE18_RESOLVE_THRESHOLD_NUM = PHASE17_RESOLVE_THRESHOLD_NUM;
/** Fixed stake-weighted resolve quorum denominator. */
export const PHASE18_RESOLVE_THRESHOLD_DEN = PHASE17_RESOLVE_THRESHOLD_DEN;
/** Honest plan strings for Phase 18 (Ed25519 attested resolve + registry V3; still 16 MiB). */
export function phase18DasSuccinctProofPlan() {
    return [
        ...phase17DasSuccinctProofPlan(),
        'Phase 18: ResolveSlashAppealAttested (32) carries a domain-separated Ed25519 ballot over appeal_id|slash_id|approved|voter|chain_id.',
        'Phase 18: ballots accumulate on the stake-weighted vote record and are verified individually (thin attest — not BLS aggregate).',
        'Phase 18: ResolveSlashAppeal (31) remains for historical replay and unattested solo casts (ballot_signature=None).',
        'Phase 18: prefer 32 for multi-validator committee nets; stake-weighted ⌈2W/3⌉ finalize math unchanged from Phase 17.',
        'Phase 18: slash_appeals_registry.bin is V3 (magic+u32+bincode); V2/legacy Phase 14–16 loads migrate without required clear.',
        'Phase 18: true BLS aggregate of resolve ballots is still later — do not invent BLS crates or fake-BLS naming.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 19 aliases / constants. */
export const PHASE19_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE18_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE19_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE18_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE19_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE18_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE19_SETTLE_RECEIPT_TTL_BLOCKS = PHASE18_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE19_MAX_APPEAL_EVIDENCE_BYTES = PHASE18_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE19_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE18_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE19_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE18_RESOLVE_SLASH_APPEAL_TX_INDEX;
/** Attested resolve payload index (`ResolveSlashAppealAttested`). */
export const PHASE19_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE18_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉). */
export const PHASE19_RESOLVE_THRESHOLD_NUM = PHASE18_RESOLVE_THRESHOLD_NUM;
/** Fixed stake-weighted resolve quorum denominator. */
export const PHASE19_RESOLVE_THRESHOLD_DEN = PHASE18_RESOLVE_THRESHOLD_DEN;
/**
 * True when a multi-pin committee must use attested resolve (32).
 * `pinnedLen` is `|slash_appeal_resolvers|` when the host pin set is present.
 */
export function phase19CommitteeRequiresAttested(pinnedLen) {
    return pinnedLen > 1;
}
/** Honest plan strings for Phase 19 (committee attested-required; still 16 MiB). */
export function phase19DasSuccinctProofPlan() {
    return [
        ...phase18DasSuccinctProofPlan(),
        'Phase 19: when |pinned slash_appeal_resolvers| > 1, ResolveSlashAppealAttested (32) is required; unattested ResolveSlashAppeal (31) is rejected.',
        'Phase 19: solo / unpinned / |pinned| ≤ 1 keep 31 for UX and historical replay; stake-weighted ⌈2W/3⌉ finalize math unchanged.',
        'Phase 19: mid-tally historical votes with ballot_signature=None keep their weight; new multi-pin casts must use 32.',
        'Phase 19: slash_appeals_registry.bin remains V3 (V2/legacy load); do not clear on upgrade — policy is execution-time only.',
        'Phase 19: true BLS aggregate of resolve ballots is still later — do not invent BLS crates or fake-BLS naming.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 20 aliases / constants. */
export const PHASE20_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE19_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE20_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE19_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE19_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE20_SETTLE_RECEIPT_TTL_BLOCKS = PHASE19_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE20_MAX_APPEAL_EVIDENCE_BYTES = PHASE19_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE20_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE19_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE20_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE19_RESOLVE_SLASH_APPEAL_TX_INDEX;
/** Attested resolve payload index (`ResolveSlashAppealAttested`). */
export const PHASE20_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE19_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉). */
export const PHASE20_RESOLVE_THRESHOLD_NUM = PHASE19_RESOLVE_THRESHOLD_NUM;
/** Fixed stake-weighted resolve quorum denominator. */
export const PHASE20_RESOLVE_THRESHOLD_DEN = PHASE19_RESOLVE_THRESHOLD_DEN;
/** Sentinel: pre-V4 Pending appeals use live-W fallback when snapshot is this value + zero pin commit. */
export const PHASE20_LEGACY_ELECTORATE_W_SENTINEL = 0n;
/**
 * True when a multi-pin committee must use attested resolve (32) (Phase 19 alias).
 */
export function phase20CommitteeRequiresAttested(pinnedLen) {
    return phase19CommitteeRequiresAttested(pinnedLen);
}
/** Honest plan strings for Phase 20 (electorate W snapshot at Submit; still 16 MiB). */
export function phase20DasSuccinctProofPlan() {
    return [
        ...phase19DasSuccinctProofPlan(),
        'Phase 20: SubmitSlashAppeal (30) snapshots electorate_w (Σ bonded stake over pinned slash_appeal_resolvers) and a pin-set commitment; Resolve (31/32) uses frozen W for ⌈2W/3⌉.',
        'Phase 20: voter weight remains bonded stake at cast; denominator is Submit-height W (not live restake mid-tally).',
        'Phase 20: unpinned / empty pin → snapshot W=1; W=0 with non-empty pin stays Pending (no rewrite); pin-set change vs snapshot commitment rejects Resolve.',
        'Phase 20: slash_appeals_registry.bin is V4; V3/V2/legacy load migrates without required clear — pre-V4 Pending uses live-W fallback (sentinel 0 + zero pin commit).',
        'Phase 20: appeal/resolve expiry, true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 21 aliases / constants. */
export const PHASE21_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE20_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE21_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE20_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE21_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
/** Resolve window after SubmitSlashAppeal (tip + this many blocks). */
export const PHASE21_RESOLVE_WINDOW_BLOCKS = PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE21_SETTLE_RECEIPT_TTL_BLOCKS = PHASE20_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE21_MAX_APPEAL_EVIDENCE_BYTES = PHASE20_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE21_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE20_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE21_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE20_RESOLVE_SLASH_APPEAL_TX_INDEX;
/** Attested resolve payload index (`ResolveSlashAppealAttested`). */
export const PHASE21_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE20_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
/** Fixed stake-weighted resolve quorum numerator (⌈2W/3⌉). */
export const PHASE21_RESOLVE_THRESHOLD_NUM = PHASE20_RESOLVE_THRESHOLD_NUM;
/** Fixed stake-weighted resolve quorum denominator. */
export const PHASE21_RESOLVE_THRESHOLD_DEN = PHASE20_RESOLVE_THRESHOLD_DEN;
/** Sentinel: pre-V4 Pending appeals use live-W fallback when snapshot is this value + zero pin commit. */
export const PHASE21_LEGACY_ELECTORATE_W_SENTINEL = PHASE20_LEGACY_ELECTORATE_W_SENTINEL;
/**
 * True when a multi-pin committee must use attested resolve (32) (Phase 19 alias).
 */
export function phase21CommitteeRequiresAttested(pinnedLen) {
    return phase20CommitteeRequiresAttested(pinnedLen);
}
/** `resolve_deadline` from Submit tip height. */
export function phase21ResolveDeadline(submitHeight) {
    return submitHeight + PHASE21_RESOLVE_WINDOW_BLOCKS;
}
/** True when tip is strictly past the resolve deadline. */
export function phase21ResolveExpired(tipHeight, resolveDeadline) {
    return tipHeight > resolveDeadline;
}
/** Honest plan strings for Phase 21 (resolve expiry + Pending cleanup; still 16 MiB). */
export function phase21DasSuccinctProofPlan() {
    return [
        ...phase20DasSuccinctProofPlan(),
        'Phase 21: SubmitSlashAppeal (30) stores resolve_deadline = tip + 1000; Resolve (31/32) after deadline expires Pending → Expired (no vote) and rejects ResolveWindowClosed.',
        'Phase 21: block housekeeping expire_pending_appeals cleans stuck Pending (W=0 / incomplete ⌈2W/3⌉); Expired never restores balance/stake (economically like Rejected).',
        'Phase 21: slash_appeals_registry.bin is V5; V4/V3/V2/legacy load migrates without required clear — pre-V5 Pending derives resolve_deadline from slash.appeal_deadline.',
        'Phase 21: Phase 19 multi-pin → 32 and Phase 20 frozen W unchanged; true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 22 ballot domain tags. */
export const SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V1 = 'BOING_SLASH_APPEAL_RESOLVE_BALLOT_V1';
export const SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V2 = 'BOING_SLASH_APPEAL_RESOLVE_BALLOT_V2';
/** Phase 22 aliases / constants. */
export const PHASE22_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE21_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE22_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE21_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE22_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE21_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE22_RESOLVE_WINDOW_BLOCKS = PHASE21_RESOLVE_WINDOW_BLOCKS;
export const PHASE22_SETTLE_RECEIPT_TTL_BLOCKS = PHASE21_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE22_MAX_APPEAL_EVIDENCE_BYTES = PHASE21_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE22_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE21_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE22_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE21_RESOLVE_SLASH_APPEAL_TX_INDEX;
export const PHASE22_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE21_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
export const PHASE22_RESOLVE_THRESHOLD_NUM = PHASE21_RESOLVE_THRESHOLD_NUM;
export const PHASE22_RESOLVE_THRESHOLD_DEN = PHASE21_RESOLVE_THRESHOLD_DEN;
export const PHASE22_LEGACY_ELECTORATE_W_SENTINEL = PHASE21_LEGACY_ELECTORATE_W_SENTINEL;
/** True when a multi-pin committee must use attested resolve (32) (Phase 19 alias). */
export function phase22CommitteeRequiresAttested(pinnedLen) {
    return phase21CommitteeRequiresAttested(pinnedLen);
}
export function phase22ResolveDeadline(submitHeight) {
    return phase21ResolveDeadline(submitHeight);
}
export function phase22ResolveExpired(tipHeight, resolveDeadline) {
    return phase21ResolveExpired(tipHeight, resolveDeadline);
}
/** BLAKE3 preimage for ballot domain V1 (historical). */
export function slashAppealResolveBallotPreimageV1(input) {
    if (input.voter.length !== 32)
        throw new Error('voter must be 32 bytes');
    const domain = new TextEncoder().encode(SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V1);
    return blake3(concatBytes(domain, u64Le(input.chainId), u64Le(input.appealId), u64Le(input.slashId), new Uint8Array([input.approved ? 1 : 0]), input.voter));
}
/** BLAKE3 preimage for ballot domain V2 (binds pin_set_commitment). */
export function slashAppealResolveBallotPreimageV2(input) {
    if (input.voter.length !== 32)
        throw new Error('voter must be 32 bytes');
    if (input.pinSetCommitment.length !== 32) {
        throw new Error('pinSetCommitment must be 32 bytes');
    }
    const domain = new TextEncoder().encode(SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V2);
    return blake3(concatBytes(domain, u64Le(input.chainId), u64Le(input.appealId), u64Le(input.slashId), new Uint8Array([input.approved ? 1 : 0]), input.voter, input.pinSetCommitment));
}
/** Sign a historical V1 resolve ballot. */
export async function signSlashAppealResolveBallotV1(input) {
    if (input.secretKey32.length !== 32)
        throw new Error('secret key must be 32 bytes');
    const voter = await ed25519.getPublicKeyAsync(input.secretKey32);
    const msg = slashAppealResolveBallotPreimageV1({
        chainId: input.chainId,
        appealId: input.appealId,
        slashId: input.slashId,
        approved: input.approved,
        voter,
    });
    return ed25519.signAsync(msg, input.secretKey32);
}
/** Sign a Phase 22 V2 resolve ballot (binds pin set). Prefer for new 32 casts. */
export async function signSlashAppealResolveBallotV2(input) {
    if (input.secretKey32.length !== 32)
        throw new Error('secret key must be 32 bytes');
    const voter = await ed25519.getPublicKeyAsync(input.secretKey32);
    const msg = slashAppealResolveBallotPreimageV2({
        chainId: input.chainId,
        appealId: input.appealId,
        slashId: input.slashId,
        approved: input.approved,
        voter,
        pinSetCommitment: input.pinSetCommitment,
    });
    return ed25519.signAsync(msg, input.secretKey32);
}
/**
 * Dual-verify a ballot: non-legacy pin tries V2 then V1; legacy all-zero pin is V1-only.
 * Returns which domain matched, or null if neither verifies.
 */
export async function verifySlashAppealResolveBallotDual(input) {
    if (input.voter.length !== 32 || input.ballotSignature.length !== 64)
        return null;
    if (input.pinSetCommitment.length !== 32)
        return null;
    const legacy = input.pinSetCommitment.every((b) => b === 0);
    const tryV1 = async () => {
        const msg = slashAppealResolveBallotPreimageV1(input);
        return ed25519.verifyAsync(input.ballotSignature, msg, input.voter);
    };
    const tryV2 = async () => {
        const msg = slashAppealResolveBallotPreimageV2(input);
        return ed25519.verifyAsync(input.ballotSignature, msg, input.voter);
    };
    if (legacy) {
        return (await tryV1()) ? 'v1' : null;
    }
    if (await tryV2())
        return 'v2';
    if (await tryV1())
        return 'v1';
    return null;
}
/** Honest plan strings for Phase 22 (batch re-verify + pin-bound V2 ballots; still 16 MiB). */
export function phase22DasSuccinctProofPlan() {
    return [
        ...phase21DasSuccinctProofPlan(),
        'Phase 22: boing_reverifySlashAppealBallots is a read-only audit RPC — batch-checks stored Ed25519 resolve ballots, pin-set match, and registry commitment; never mutates book state.',
        'Phase 22: ResolveSlashAppealAttested (32) new casts use ballot domain V2 binding pin_set_commitment; dual-verify accepts historical V1 (mid-upgrade / pre-Phase-22).',
        'Phase 22: registry stays V5 (no schema change, no clear); txs 30/31/32 unchanged; Phase 19 multi-pin→32, Phase 20 frozen W, Phase 21 resolve expiry unchanged.',
        'Phase 22: true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 23 retention for Expired slash appeals (blocks after resolve_deadline). */
export const PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS = 4096n;
/** Phase 23 aliases / constants. */
export const PHASE23_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE22_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE23_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE22_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE23_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE22_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE23_RESOLVE_WINDOW_BLOCKS = PHASE22_RESOLVE_WINDOW_BLOCKS;
export const PHASE23_SETTLE_RECEIPT_TTL_BLOCKS = PHASE22_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE23_MAX_APPEAL_EVIDENCE_BYTES = PHASE22_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE23_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE22_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE23_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE22_RESOLVE_SLASH_APPEAL_TX_INDEX;
export const PHASE23_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE22_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
export const PHASE23_RESOLVE_THRESHOLD_NUM = PHASE22_RESOLVE_THRESHOLD_NUM;
export const PHASE23_RESOLVE_THRESHOLD_DEN = PHASE22_RESOLVE_THRESHOLD_DEN;
export const PHASE23_LEGACY_ELECTORATE_W_SENTINEL = PHASE22_LEGACY_ELECTORATE_W_SENTINEL;
/** True when a multi-pin committee must use attested resolve (32) (Phase 19 alias). */
export function phase23CommitteeRequiresAttested(pinnedLen) {
    return phase22CommitteeRequiresAttested(pinnedLen);
}
export function phase23ResolveDeadline(submitHeight) {
    return phase22ResolveDeadline(submitHeight);
}
export function phase23ResolveExpired(tipHeight, resolveDeadline) {
    return phase22ResolveExpired(tipHeight, resolveDeadline);
}
/**
 * True when an Expired appeal is past retention and may be pruned:
 * `tipHeight > resolveDeadline + PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS`.
 */
export function phase23ExpiredAppealPrunable(tipHeight, resolveDeadline) {
    return tipHeight > resolveDeadline + PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS;
}
/** Honest plan strings for Phase 23 (Expired retention prune + list filters; still 16 MiB). */
export function phase23DasSuccinctProofPlan() {
    return [
        ...phase22DasSuccinctProofPlan(),
        'Phase 23: block housekeeping prune_expired_appeals drops Expired rows when tip > resolve_deadline + 4096 (mirrors settle-receipt TTL); slash records stay; no fund restore.',
        'Phase 23: do not prune Pending / Approved / Rejected — keep finalize + ballot audit trail for Phase 22 re-verify.',
        'Phase 23: boing_listSlashRecords gains appealStatus / includeExpired (default true for back-compat); boing_reverifySlashAppealBallots gains includeExpired (default false).',
        'Phase 23: registry stays V5 (row deletion only, no schema change, no clear); txs 30/31/32 unchanged; Phase 19–22 policies unchanged.',
        'Phase 23: true BLS aggregate, SNARK/IPA/KZG, serve-challenge TTL, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 24 retention for terminal serve challenges (blocks after deadline). */
export const PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = 4096n;
/** Phase 24 aliases / constants. */
export const PHASE24_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE23_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE24_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE23_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE24_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE23_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE24_RESOLVE_WINDOW_BLOCKS = PHASE23_RESOLVE_WINDOW_BLOCKS;
export const PHASE24_SETTLE_RECEIPT_TTL_BLOCKS = PHASE23_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE24_EXPIRED_APPEAL_RETENTION_BLOCKS = PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS;
export const PHASE24_MAX_APPEAL_EVIDENCE_BYTES = PHASE23_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE24_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE23_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE24_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE23_RESOLVE_SLASH_APPEAL_TX_INDEX;
export const PHASE24_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE23_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
export const PHASE24_RESOLVE_THRESHOLD_NUM = PHASE23_RESOLVE_THRESHOLD_NUM;
export const PHASE24_RESOLVE_THRESHOLD_DEN = PHASE23_RESOLVE_THRESHOLD_DEN;
export const PHASE24_LEGACY_ELECTORATE_W_SENTINEL = PHASE23_LEGACY_ELECTORATE_W_SENTINEL;
export function phase24CommitteeRequiresAttested(pinnedLen) {
    return phase23CommitteeRequiresAttested(pinnedLen);
}
export function phase24ResolveDeadline(submitHeight) {
    return phase23ResolveDeadline(submitHeight);
}
export function phase24ResolveExpired(tipHeight, resolveDeadline) {
    return phase23ResolveExpired(tipHeight, resolveDeadline);
}
export function phase24ExpiredAppealPrunable(tipHeight, resolveDeadline) {
    return phase23ExpiredAppealPrunable(tipHeight, resolveDeadline);
}
/**
 * True when a terminal serve challenge is past retention and may be pruned:
 * settled Responded/FailedServe with `tip > deadline + PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS`.
 */
export function phase24TerminalServeChallengePrunable(input) {
    if (!input.bondSettled || input.status === 'open')
        return false;
    return (input.tipHeight >
        input.deadlineHeight + PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS);
}
/** Honest plan strings for Phase 24 (serve-challenge TTL prune + list/get UX; still 16 MiB). */
export function phase24DasSuccinctProofPlan() {
    return [
        ...phase23DasSuccinctProofPlan(),
        'Phase 24: block housekeeping prune_terminal_serve_challenges drops Responded / settled FailedServe when tip > deadline + 4096 (frees soft-cap 256); Open and unsettled FailedServe stay.',
        'Phase 24: boing_listBlobSampleServeChallenges gains status / includeTerminal filters; boing_getBlobSampleServeChallenge looks up by blobId+chunkIndex+provider+challenger.',
        'Phase 24: slash_appeals_registry.bin untouched (stay V5); do not clear; txs 28/29/30/31/32 unchanged; Phase 19–23 policies unchanged.',
        'Phase 24: true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 25 retention for Approved/Rejected slash appeals (blocks after resolve_deadline). */
export const PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS = 4096n;
/** Phase 25 aliases / constants. */
export const PHASE25_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE24_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE25_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE24_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE25_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE24_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE25_RESOLVE_WINDOW_BLOCKS = PHASE24_RESOLVE_WINDOW_BLOCKS;
export const PHASE25_SETTLE_RECEIPT_TTL_BLOCKS = PHASE24_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE25_EXPIRED_APPEAL_RETENTION_BLOCKS = PHASE24_EXPIRED_APPEAL_RETENTION_BLOCKS;
export const PHASE25_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
export const PHASE25_MAX_APPEAL_EVIDENCE_BYTES = PHASE24_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE25_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE24_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE25_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE24_RESOLVE_SLASH_APPEAL_TX_INDEX;
export const PHASE25_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE24_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
export const PHASE25_RESOLVE_THRESHOLD_NUM = PHASE24_RESOLVE_THRESHOLD_NUM;
export const PHASE25_RESOLVE_THRESHOLD_DEN = PHASE24_RESOLVE_THRESHOLD_DEN;
export const PHASE25_LEGACY_ELECTORATE_W_SENTINEL = PHASE24_LEGACY_ELECTORATE_W_SENTINEL;
export function phase25CommitteeRequiresAttested(pinnedLen) {
    return phase24CommitteeRequiresAttested(pinnedLen);
}
export function phase25ResolveDeadline(submitHeight) {
    return phase24ResolveDeadline(submitHeight);
}
export function phase25ResolveExpired(tipHeight, resolveDeadline) {
    return phase24ResolveExpired(tipHeight, resolveDeadline);
}
export function phase25ExpiredAppealPrunable(tipHeight, resolveDeadline) {
    return phase24ExpiredAppealPrunable(tipHeight, resolveDeadline);
}
/**
 * True when an Approved/Rejected appeal is past retention and may be pruned:
 * `tipHeight > resolveDeadline + PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS`.
 */
export function phase25ResolvedAppealPrunable(tipHeight, resolveDeadline) {
    return tipHeight > resolveDeadline + PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS;
}
export function phase25TerminalServeChallengePrunable(input) {
    return phase24TerminalServeChallengePrunable(input);
}
/** Honest plan strings for Phase 25 (ancient Approved/Rejected prune + list filters; still 16 MiB). */
export function phase25DasSuccinctProofPlan() {
    return [
        ...phase24DasSuccinctProofPlan(),
        'Phase 25: block housekeeping prune_resolved_appeals drops Approved / Rejected rows when tip > resolve_deadline + 4096 (mirrors Expired TTL); slash records stay; Pending untouched.',
        'Phase 25: boing_listSlashRecords gains includeResolved (default true for back-compat); boing_reverifySlashAppealBallots gains includeResolved (default false).',
        'Phase 25: registry stays V5 (row deletion only, no schema change, no clear); txs 30/31/32 unchanged; Phase 19–24 policies unchanged.',
        'Phase 25: true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Compressed BLS12-381 G1 public key size (bytes). */
export const PHASE26_BLS_PUBLIC_KEY_G1_BYTES = 48;
/** Compressed BLS12-381 G2 signature / aggregate size (bytes). */
export const PHASE26_BLS_SIGNATURE_G2_BYTES = 96;
/**
 * Flip only when a real `blst` (or equivalent) verify path is wired.
 * Scaffold keeps this **false** — do not treat types as live crypto.
 */
export const PHASE26_BLS_AGGREGATE_WIRED = false;
/** Phase 26 aliases / constants. */
export const PHASE26_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE25_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE26_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE25_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE26_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE25_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE26_RESOLVE_WINDOW_BLOCKS = PHASE25_RESOLVE_WINDOW_BLOCKS;
export const PHASE26_SETTLE_RECEIPT_TTL_BLOCKS = PHASE25_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE26_EXPIRED_APPEAL_RETENTION_BLOCKS = PHASE25_EXPIRED_APPEAL_RETENTION_BLOCKS;
export const PHASE26_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = PHASE25_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
export const PHASE26_RESOLVED_APPEAL_RETENTION_BLOCKS = PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS;
export const PHASE26_MAX_APPEAL_EVIDENCE_BYTES = PHASE25_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE26_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE25_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE26_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE25_RESOLVE_SLASH_APPEAL_TX_INDEX;
export const PHASE26_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE25_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
export const PHASE26_RESOLVE_THRESHOLD_NUM = PHASE25_RESOLVE_THRESHOLD_NUM;
export const PHASE26_RESOLVE_THRESHOLD_DEN = PHASE25_RESOLVE_THRESHOLD_DEN;
export const PHASE26_LEGACY_ELECTORATE_W_SENTINEL = PHASE25_LEGACY_ELECTORATE_W_SENTINEL;
/** Length-check a dual-key set (unique accountIds). No curve validation until wired. */
export function phase26ValidateDualKeySet(identities) {
    if (identities.length === 0) {
        throw new Error('empty signer set');
    }
    const seen = new Set();
    for (const id of identities) {
        if (id.blsPubkey.length !== PHASE26_BLS_PUBLIC_KEY_G1_BYTES) {
            throw new Error(`invalid BLS public key length: expected ${PHASE26_BLS_PUBLIC_KEY_G1_BYTES}, got ${id.blsPubkey.length}`);
        }
        if (id.accountId.length !== 32) {
            throw new Error(`invalid AccountId length: expected 32, got ${id.accountId.length}`);
        }
        const key = bytesToHex(id.accountId);
        if (seen.has(key))
            throw new Error('duplicate AccountId in dual-key registration set');
        seen.add(key);
    }
}
/**
 * Aggregate / verify stubs — always throw until `PHASE26_BLS_AGGREGATE_WIRED`.
 * Live resolve path remains Ed25519 verify-each on tx 32.
 */
export function phase26AggregateBlsSignatures(_signatures) {
    throw new Error('BLS aggregate crypto not wired (PHASE26_BLS_AGGREGATE_WIRED=false); Ed25519 verify-each on tx 32 remains live');
}
export function phase26VerifyBlsAggregate(_input) {
    throw new Error('BLS aggregate crypto not wired (PHASE26_BLS_AGGREGATE_WIRED=false); Ed25519 verify-each on tx 32 remains live');
}
export function phase26CommitteeRequiresAttested(pinnedLen) {
    return phase25CommitteeRequiresAttested(pinnedLen);
}
export function phase26ResolveDeadline(submitHeight) {
    return phase25ResolveDeadline(submitHeight);
}
export function phase26ResolvedAppealPrunable(tipHeight, resolveDeadline) {
    return phase25ResolvedAppealPrunable(tipHeight, resolveDeadline);
}
/** Honest plan strings for Phase 26 (BLS dual-key scaffold; still 16 MiB). */
export function phase26DasSuccinctProofPlan() {
    return [
        ...phase25DasSuccinctProofPlan(),
        'Phase 26: scaffold dual-key BLS aggregate types (BLS12-381 G1 pubkey 48 B + G2 sig 96 B) and NotWired aggregate/verify APIs — do not fake crypto.',
        'Phase 26: live ResolveSlashAppealAttested (32) stays Ed25519 verify-each; PHASE26_BLS_AGGREGATE_WIRED=false until blst is wired.',
        'Phase 26: remaining — optional blst feature, dual-key persistence (likely V6), registration tx/host config, aggregate submit path, flip verify when wired.',
        'Phase 26: registry stays V5 (untouched); do not clear; txs 30/31/32 unchanged; Phase 19–25 policies unchanged.',
        'Phase 26: SNARK/IPA/KZG and live 256 MiB remain later — keep Fly-safe 16 MiB.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Compressed BLS12-381 sizes (same as Phase 26). */
export const PHASE27_BLS_PUBLIC_KEY_G1_BYTES = PHASE26_BLS_PUBLIC_KEY_G1_BYTES;
export const PHASE27_BLS_SIGNATURE_G2_BYTES = PHASE26_BLS_SIGNATURE_G2_BYTES;
/**
 * Node library + RPC aggregate verify is wired (`blst` min_pk).
 * Live tx **32** remains Ed25519 verify-each. JS SDK has no blst — use
 * `boing_verifyBlsAggregate` on the node for real verify.
 */
export const PHASE27_BLS_AGGREGATE_WIRED = true;
export const PHASE27_BLS_DUAL_KEY_REGISTRY_FILE = 'bls_dual_key_registry.bin';
/** Phase 27 aliases / constants. */
export const PHASE27_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE26_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE27_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE26_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE27_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE26_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE27_RESOLVE_WINDOW_BLOCKS = PHASE26_RESOLVE_WINDOW_BLOCKS;
export const PHASE27_SETTLE_RECEIPT_TTL_BLOCKS = PHASE26_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE27_EXPIRED_APPEAL_RETENTION_BLOCKS = PHASE26_EXPIRED_APPEAL_RETENTION_BLOCKS;
export const PHASE27_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = PHASE26_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
export const PHASE27_RESOLVED_APPEAL_RETENTION_BLOCKS = PHASE26_RESOLVED_APPEAL_RETENTION_BLOCKS;
export const PHASE27_MAX_APPEAL_EVIDENCE_BYTES = PHASE26_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE27_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE26_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE27_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE26_RESOLVE_SLASH_APPEAL_TX_INDEX;
export const PHASE27_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE26_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
export const PHASE27_RESOLVE_THRESHOLD_NUM = PHASE26_RESOLVE_THRESHOLD_NUM;
export const PHASE27_RESOLVE_THRESHOLD_DEN = PHASE26_RESOLVE_THRESHOLD_DEN;
export const PHASE27_LEGACY_ELECTORATE_W_SENTINEL = PHASE26_LEGACY_ELECTORATE_W_SENTINEL;
/** Length-check a dual-key set (unique accountIds). */
export function phase27ValidateDualKeySet(identities) {
    phase26ValidateDualKeySet(identities);
}
/**
 * JS has no `blst` — do not fake crypto. Use node RPC `boing_verifyBlsAggregate`
 * (or the Rust library). Live tx 32 remains Ed25519 verify-each.
 */
export function phase27AggregateBlsSignatures(_signatures) {
    throw new Error('JS SDK has no blst; use node library / boing_verifyBlsAggregate RPC (PHASE27_BLS_AGGREGATE_WIRED=true). Live tx 32 stays Ed25519 verify-each');
}
export function phase27VerifyBlsAggregate(_input) {
    throw new Error('JS SDK has no blst; use node library / boing_verifyBlsAggregate RPC (PHASE27_BLS_AGGREGATE_WIRED=true). Live tx 32 stays Ed25519 verify-each');
}
export function phase27CommitteeRequiresAttested(pinnedLen) {
    return phase26CommitteeRequiresAttested(pinnedLen);
}
export function phase27ResolveDeadline(submitHeight) {
    return phase26ResolveDeadline(submitHeight);
}
export function phase27ResolvedAppealPrunable(tipHeight, resolveDeadline) {
    return phase26ResolvedAppealPrunable(tipHeight, resolveDeadline);
}
/** Honest plan strings for Phase 27 (blst wire + host-local dual-keys; still 16 MiB). */
export function phase27DasSuccinctProofPlan() {
    return [
        ...phase26DasSuccinctProofPlan(),
        'Phase 27: wire blst (min_pk) — real sign / aggregate / fast_aggregate_verify; PHASE27_BLS_AGGREGATE_WIRED=true for library + RPC.',
        'Phase 27: host-local bls_dual_key_registry.bin (BDKR v1) — not slash V5, not state-root; list/register/verify RPC.',
        'Phase 27: live ResolveSlashAppealAttested (32) stays Ed25519 verify-each — do not flip committee cast until aggregate submit path lands.',
        'Phase 27: JS SDK has no blst — clients call boing_verifyBlsAggregate; do not fake browser verify.',
        'Phase 27: remaining — PoP for dual-keys, aggregate submit tx / signer bitset, flip verify-each → aggregate when operators opt in.',
        'Phase 27: slash_appeals_registry.bin stays V5 (untouched); do not clear; txs 30/31/32 unchanged; Phase 19–26 policies unchanged.',
        'Phase 27: SNARK/IPA/KZG and live 256 MiB remain later — keep Fly-safe 16 MiB.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
/** Phase 28 sizes / wired flags (library+RPC still from Phase 27). */
export const PHASE28_BLS_PUBLIC_KEY_G1_BYTES = PHASE27_BLS_PUBLIC_KEY_G1_BYTES;
export const PHASE28_BLS_SIGNATURE_G2_BYTES = PHASE27_BLS_SIGNATURE_G2_BYTES;
export const PHASE28_BLS_AGGREGATE_WIRED = PHASE27_BLS_AGGREGATE_WIRED;
export const PHASE28_BLS_DUAL_KEY_REGISTRY_FILE = PHASE27_BLS_DUAL_KEY_REGISTRY_FILE;
/** Registration requires a verified BLS proof-of-possession. */
export const PHASE28_BLS_POP_REQUIRED = true;
/** BLAKE3 domain for the PoP preimage (not the blst DST). */
export const PHASE28_BLS_POP_DOMAIN = new TextEncoder().encode('BOING_BLS_DUAL_KEY_POP_V1');
/** CipherSuite DST string for dual-key PoP (Rust/`blst` signers). */
export const PHASE28_BLS_POP_SIG_DST = 'BOING_BLS_POP_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_';
export const PHASE28_SERVE_CHALLENGE_WINDOW_BLOCKS = PHASE27_SERVE_CHALLENGE_WINDOW_BLOCKS;
export const PHASE28_ACTIVATED_MAX_MEDIA_BLOB_BYTES = PHASE27_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
export const PHASE28_FAILED_SERVE_APPEAL_WINDOW_BLOCKS = PHASE27_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
export const PHASE28_RESOLVE_WINDOW_BLOCKS = PHASE27_RESOLVE_WINDOW_BLOCKS;
export const PHASE28_SETTLE_RECEIPT_TTL_BLOCKS = PHASE27_SETTLE_RECEIPT_TTL_BLOCKS;
export const PHASE28_EXPIRED_APPEAL_RETENTION_BLOCKS = PHASE27_EXPIRED_APPEAL_RETENTION_BLOCKS;
export const PHASE28_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS = PHASE27_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
export const PHASE28_RESOLVED_APPEAL_RETENTION_BLOCKS = PHASE27_RESOLVED_APPEAL_RETENTION_BLOCKS;
export const PHASE28_MAX_APPEAL_EVIDENCE_BYTES = PHASE27_MAX_APPEAL_EVIDENCE_BYTES;
export const PHASE28_SUBMIT_SLASH_APPEAL_TX_INDEX = PHASE27_SUBMIT_SLASH_APPEAL_TX_INDEX;
export const PHASE28_RESOLVE_SLASH_APPEAL_TX_INDEX = PHASE27_RESOLVE_SLASH_APPEAL_TX_INDEX;
export const PHASE28_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX = PHASE27_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
export const PHASE28_RESOLVE_THRESHOLD_NUM = PHASE27_RESOLVE_THRESHOLD_NUM;
export const PHASE28_RESOLVE_THRESHOLD_DEN = PHASE27_RESOLVE_THRESHOLD_DEN;
export const PHASE28_LEGACY_ELECTORATE_W_SENTINEL = PHASE27_LEGACY_ELECTORATE_W_SENTINEL;
/**
 * BLAKE3 PoP preimage: domain || chainId_le8 || accountId || blsPubkey.
 * Sign this 32-byte message with blst under `PHASE28_BLS_POP_SIG_DST` (Rust tooling).
 */
export function phase28BlsPopPreimage(input) {
    if (input.accountId.length !== 32) {
        throw new Error(`invalid AccountId length: expected 32, got ${input.accountId.length}`);
    }
    if (input.blsPubkey.length !== PHASE28_BLS_PUBLIC_KEY_G1_BYTES) {
        throw new Error(`invalid BLS public key length: expected ${PHASE28_BLS_PUBLIC_KEY_G1_BYTES}, got ${input.blsPubkey.length}`);
    }
    return blake3(concatBytes(PHASE28_BLS_POP_DOMAIN, u64Le(input.chainId), input.accountId, input.blsPubkey));
}
/** Length-check a dual-key set (unique accountIds). */
export function phase28ValidateDualKeySet(identities) {
    phase27ValidateDualKeySet(identities);
}
/**
 * JS has no `blst` — do not fake PoP sign/verify. Use Rust/`blst` to sign
 * `phase28BlsPopPreimage`, then `boing_registerBlsDualKey` with `blsPop`.
 */
export function phase28SignBlsPop(_input) {
    throw new Error('JS SDK has no blst; sign phase28BlsPopPreimage with Rust/blst under PHASE28_BLS_POP_SIG_DST, then call boing_registerBlsDualKey');
}
export function phase28VerifyBlsPop(_input) {
    throw new Error('JS SDK has no blst; node verifies PoP on boing_registerBlsDualKey (PHASE28_BLS_POP_REQUIRED=true)');
}
export function phase28CommitteeRequiresAttested(pinnedLen) {
    return phase27CommitteeRequiresAttested(pinnedLen);
}
export function phase28ResolveDeadline(submitHeight) {
    return phase27ResolveDeadline(submitHeight);
}
export function phase28ResolvedAppealPrunable(tipHeight, resolveDeadline) {
    return phase27ResolvedAppealPrunable(tipHeight, resolveDeadline);
}
/** Honest plan strings for Phase 28 (BLS dual-key PoP; still 16 MiB). */
export function phase28DasSuccinctProofPlan() {
    return [
        ...phase27DasSuccinctProofPlan(),
        'Phase 28: BLS dual-key proof-of-possession — chain-bound PoP required on boing_registerBlsDualKey (PHASE28_BLS_POP_REQUIRED=true).',
        'Phase 28: PoP message = BLAKE3(BOING_BLS_DUAL_KEY_POP_V1 || chain_id || accountId || blsPubkey); signed under BOING_BLS_POP_… DST.',
        'Phase 28: live ResolveSlashAppealAttested (32) stays Ed25519 verify-each — PoP does not flip committee cast.',
        'Phase 28: JS SDK exposes phase28BlsPopPreimage only; sign with Rust/blst — do not fake browser PoP.',
        'Phase 28: remaining — aggregate submit tx / signer bitset, flip verify-each → aggregate, optional Ed25519 account-side binding, SNARK / 256 MiB.',
        'Phase 28: slash_appeals_registry.bin stays V5; bls_dual_key_registry.bin stays BDKR v1; do not clear.',
        'ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.',
    ];
}
