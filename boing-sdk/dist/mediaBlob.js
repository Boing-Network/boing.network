/**
 * L1 media blobs — content-addressed PutBlob / BlobAnnounce helpers (not the interface SSTORE store).
 * @see docs/BOING-L1-MEDIA-BLOBS.md
 */
import { blake3 } from '@noble/hashes/blake3';
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
