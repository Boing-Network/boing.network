//! L1 media blobs: content-addressed ids, retention policy, caps, and soft commitments.
//!
//! Bytes live in a node sidecar (not account SMT / SSTORE). Consensus records metadata via
//! [`crate::TransactionPayload::PutBlob`] (inline) or [`crate::TransactionPayload::BlobAnnounce`]
//! (commitment; bytes arrive via P2P / RPC). See `docs/BOING-L1-MEDIA-BLOBS.md`.
//!
//! Phase 4: live cap **16 MiB**, XOR chunking, Forever pin fee, DAS sample hooks.
//! Phase 5: Reed–Solomon path ([`crate::blobs_erasure`]), renew / sample txs, sampling fee hooks.
//! Phase 6: [`crate::blobs_phase6`] (sampling market, DAS LC batch, segment manifests).
//! Phase 7: [`crate::blobs_phase7`] — on-chain offer/settle txs, VRF DAS seed, gated cap raise.
//! Phase 8: [`crate::blobs_phase8`] — consensus-replay market, succinct proof envelope, activation notes.
//! Phase 9: [`crate::blobs_phase9`] — SMT escrow helpers, leader-VRF DAS LC path, extended ops notes.
//! Phase 10: [`crate::blobs_phase10`] — serve challenges, ReservedRealV1 opaque DAS path.
//! Phase 11: [`crate::blobs_phase11`] — serve-challenge bond / FailedServe slash, ReservedRealV1 envelope checks.
//! Phase 12: [`crate::blobs_phase12`] — settle-before-challenge, FailedServe SlashRegistry audit ids.
//! Phase 13: [`crate::blobs_phase13`] — FailedServe appeal balance unburn, settle-receipt TTL.
//! Phase 14: [`crate::blobs_phase14`] — consensus Submit/Resolve slash-appeal txs (30/31).
//! Phase 15: [`crate::blobs_phase15`] — seed equivocation/liveness into consensus appeal book.
//! Phase 16: [`crate::blobs_phase16`] — committee threshold ResolveSlashAppeal (⌈2M/3⌉).

use serde::{Deserialize, Serialize};

use crate::hasher;
use crate::{Hash, Transaction, TransactionPayload};

/// Domain / URI scheme for client resolvers (`boing://blob/0x…`).
pub const MEDIA_BLOB_URI_PREFIX: &str = "boing://blob/";

/// Gossipsub topic for out-of-band media blob bytes (`boing/media-blobs`).
pub const MEDIA_BLOBS_GOSSIP_TOPIC: &str = "boing/media-blobs";

/// Live consensus hard cap: **16 MiB**. Honest limit — still not multi-minute mp4 / planet-scale DA.
/// Prefer [`BlobAnnounce`](crate::TransactionPayload::BlobAnnounce) + gossip above
/// [`MAX_MEDIA_BLOB_INLINE_PREFER_BYTES`].
pub const MAX_MEDIA_BLOB_BYTES: usize = 16 * 1024 * 1024;

/// Historical Phase 3 target — now equal to the live cap.
pub const PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES: usize = MAX_MEDIA_BLOB_BYTES;

/// Soft preference: above this size, clients should prefer BlobAnnounce + P2P over inline PutBlob.
/// PutBlob still admits up to [`MAX_MEDIA_BLOB_BYTES`].
pub const MAX_MEDIA_BLOB_INLINE_PREFER_BYTES: usize = 1024 * 1024;

/// Preferred inline PutBlob ceiling used by RPC / SDK hints (same as soft preference).
pub const MAX_MEDIA_BLOB_INLINE_BYTES: usize = MAX_MEDIA_BLOB_INLINE_PREFER_BYTES;

/// Erasure / DAS chunk size for chunking + sampling hooks.
pub const MEDIA_BLOB_DAS_CHUNK_BYTES: usize = 64 * 1024;

/// Max UTF-8 length of the optional MIME / content-type hint.
pub const MAX_MEDIA_BLOB_CONTENT_TYPE_LEN: usize = 64;

/// Soft cap on prunable TTL (blocks).
pub const MAX_MEDIA_BLOB_TTL_BLOCKS: u64 = 1_000_000;

/// Max blocks a single [`crate::TransactionPayload::RenewMediaBlobPin`] may extend.
pub const MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS: u64 = 1_000_000;

/// Suggested pin horizon for multi-minute / large Forever media economics docs (not a hard cap).
pub const MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS: u64 = 150_000;

/// Base gas for PutBlob / BlobAnnounce before size and retention scaling.
pub const MEDIA_BLOB_GAS_BASE: u64 = 21_000;

/// Gas per 32-byte word of payload (sidecar data — far cheaper than SSTORE).
pub const MEDIA_BLOB_GAS_PER_WORD: u64 = 16;

/// Forever retention multiplies blob gas by this factor (prepaid best-effort).
pub const MEDIA_BLOB_FOREVER_MULTIPLIER: u64 = 10;

/// Content-addressed blob id: `BLAKE3(payload bytes)`.
pub type BlobId = [u8; 32];

/// Retention policy encoded in PutBlob / BlobAnnounce and sidecar metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlobRetention {
    /// Node may delete after `included_height + ttl_blocks`.
    Prunable { ttl_blocks: u64 },
    /// Prepaid “best-effort forever” on full nodes (higher fee + optional pin fee). Not a
    /// cryptographic eternity guarantee until a full pin market.
    Forever,
}

impl BlobRetention {
    /// Multiplier applied to size-scaled blob gas.
    pub fn gas_multiplier(self) -> u64 {
        match self {
            BlobRetention::Prunable { .. } => 1,
            BlobRetention::Forever => MEDIA_BLOB_FOREVER_MULTIPLIER,
        }
    }

    /// Structural validation (independent of activation height).
    pub fn validate(self) -> Result<(), BlobError> {
        match self {
            BlobRetention::Forever => Ok(()),
            BlobRetention::Prunable { ttl_blocks } => {
                if ttl_blocks == 0 {
                    return Err(BlobError::TtlZero);
                }
                if ttl_blocks > MAX_MEDIA_BLOB_TTL_BLOCKS {
                    return Err(BlobError::TtlTooLarge {
                        ttl_blocks,
                        max: MAX_MEDIA_BLOB_TTL_BLOCKS,
                    });
                }
                Ok(())
            }
        }
    }

    /// Height at which a prunable blob may be GC'd (`None` = Forever).
    pub fn expiry_height(self, included_height: u64) -> Option<u64> {
        match self {
            BlobRetention::Forever => None,
            BlobRetention::Prunable { ttl_blocks } => {
                Some(included_height.saturating_add(ttl_blocks))
            }
        }
    }

    pub fn is_forever(self) -> bool {
        matches!(self, BlobRetention::Forever)
    }
}

/// Out-of-band gossip payload (P2P data plane). Not a consensus tx.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobGossip {
    pub blob_id: BlobId,
    pub content_type: String,
    pub data: Vec<u8>,
}

impl MediaBlobGossip {
    pub fn validate(&self) -> Result<(), BlobError> {
        if self.data.len() > MAX_MEDIA_BLOB_BYTES {
            return Err(BlobError::TooLarge {
                size: self.data.len(),
            });
        }
        if self.content_type.len() > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN {
            return Err(BlobError::ContentTypeTooLong {
                len: self.content_type.len(),
            });
        }
        let id = blob_id_of(&self.data);
        if id != self.blob_id {
            return Err(BlobError::IdMismatch {
                expected: blob_id_hex(&id),
                got: blob_id_hex(&self.blob_id),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BlobError {
    #[error("media blob exceeds MAX_MEDIA_BLOB_BYTES ({MAX_MEDIA_BLOB_BYTES})")]
    TooLarge { size: usize },
    #[error("media blob content_type exceeds {MAX_MEDIA_BLOB_CONTENT_TYPE_LEN} UTF-8 bytes")]
    ContentTypeTooLong { len: usize },
    #[error("prunable retention requires ttl_blocks >= 1")]
    TtlZero,
    #[error("prunable ttl_blocks {ttl_blocks} exceeds max {max}")]
    TtlTooLarge { ttl_blocks: u64, max: u64 },
    #[error("blob id mismatch: expected {expected}, got {got}")]
    IdMismatch { expected: String, got: String },
    #[error("announce size is zero")]
    AnnounceSizeZero,
    #[error("chunk index {index} out of range (count={count})")]
    ChunkIndexOutOfRange { index: u32, count: u32 },
    #[error("cannot recover blob: need all-but-one data chunks plus XOR parity (or all data chunks)")]
    RecoveryInsufficient,
    #[error("recovered bytes do not match blob id")]
    RecoveryIdMismatch,
    #[error("DAS sample proof does not verify against chunks_root")]
    DasProofInvalid,
    #[error("invalid Reed–Solomon params (data={data}, parity={parity})")]
    RsInvalidParams { data: usize, parity: usize },
    #[error("Reed–Solomon encode failed")]
    RsEncodeFailed,
    #[error("Reed–Solomon reconstruct failed")]
    RsReconstructFailed,
    #[error("Reed–Solomon shard count mismatch: expected {expected}, got {got}")]
    RsShardCountMismatch { expected: usize, got: usize },
    #[error("renew extend_blocks must be >= 1")]
    RenewExtendZero,
    #[error("renew extend_blocks {extend} exceeds max {max}")]
    RenewExtendTooLarge { extend: u64, max: u64 },
    #[error("sample chunk index {index} out of range (count={count})")]
    SampleChunkOutOfRange { index: u32, count: u32 },
    /// Phase 6 sampling-market: paid fee below the provider ask.
    #[error("sample fee {paid} below ask {ask}")]
    SampleFeeTooLow { paid: u128, ask: u128 },
    #[error("no matching sample offer")]
    SampleOfferNotFound,
    #[error("sample offer expired at height {expires_at_height} (tip {tip_height})")]
    SampleOfferExpired {
        expires_at_height: u64,
        tip_height: u64,
    },
    #[error("sample market full (max {max} offers)")]
    SampleMarketFull { max: usize },
    #[error("segment manifest empty or zero total_bytes")]
    SegmentManifestEmpty,
    #[error("segment manifest has {count} segments (max {max})")]
    SegmentManifestTooMany { count: usize, max: usize },
    #[error("segment manifest total_bytes {total_bytes} exceeds Phase 6 design max {max}")]
    SegmentManifestTooLarge { total_bytes: u64, max: u64 },
}

/// `BLAKE3(data)` as [`BlobId`].
#[must_use]
pub fn blob_id_of(data: &[u8]) -> BlobId {
    let mut h = hasher();
    h.update(data);
    *h.finalize().as_bytes()
}

/// Hex form without `0x` (64 chars).
#[must_use]
pub fn blob_id_hex(id: &BlobId) -> String {
    hex::encode(id)
}

/// `boing://blob/0x{64 hex}`.
#[must_use]
pub fn media_blob_uri(id: &BlobId) -> String {
    format!("{MEDIA_BLOB_URI_PREFIX}0x{}", blob_id_hex(id))
}

/// Parse `boing://blob/0x…` or bare 64-hex / `0x` hex into a [`BlobId`].
pub fn parse_media_blob_id(s: &str) -> Option<BlobId> {
    let s = s.trim();
    let hex_part = if let Some(rest) = s.strip_prefix(MEDIA_BLOB_URI_PREFIX) {
        rest.strip_prefix("0x")
            .or_else(|| rest.strip_prefix("0X"))
            .unwrap_or(rest)
    } else {
        s.strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s)
    };
    if hex_part.len() != 64 {
        return None;
    }
    let bytes = hex::decode(hex_part).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut id = [0u8; 32];
    id.copy_from_slice(&bytes);
    Some(id)
}

/// Validate PutBlob fields before admission / execution.
pub fn validate_put_blob(
    retention: BlobRetention,
    content_type: &str,
    data: &[u8],
) -> Result<BlobId, BlobError> {
    retention.validate()?;
    if data.len() > MAX_MEDIA_BLOB_BYTES {
        return Err(BlobError::TooLarge { size: data.len() });
    }
    if content_type.len() > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN {
        return Err(BlobError::ContentTypeTooLong {
            len: content_type.len(),
        });
    }
    Ok(blob_id_of(data))
}

/// Validate BlobAnnounce (commitment without inline bytes).
pub fn validate_blob_announce(
    blob_id: &BlobId,
    size: u32,
    retention: BlobRetention,
    content_type: &str,
) -> Result<(), BlobError> {
    retention.validate()?;
    if size == 0 {
        return Err(BlobError::AnnounceSizeZero);
    }
    if size as usize > MAX_MEDIA_BLOB_BYTES {
        return Err(BlobError::TooLarge {
            size: size as usize,
        });
    }
    if content_type.len() > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN {
        return Err(BlobError::ContentTypeTooLong {
            len: content_type.len(),
        });
    }
    let _ = blob_id;
    Ok(())
}

/// Validate Forever / prunable pin renewal (Phase 5).
pub fn validate_renew_media_blob_pin(
    blob_id: &BlobId,
    size: u32,
    extend_blocks: u64,
) -> Result<(), BlobError> {
    let _ = blob_id;
    if size == 0 || size as usize > MAX_MEDIA_BLOB_BYTES {
        return Err(BlobError::TooLarge {
            size: size as usize,
        });
    }
    if extend_blocks == 0 {
        return Err(BlobError::RenewExtendZero);
    }
    if extend_blocks > MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS {
        return Err(BlobError::RenewExtendTooLarge {
            extend: extend_blocks,
            max: MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS,
        });
    }
    Ok(())
}

/// Validate paid sample payment stub (Phase 5 market scaffolding).
pub fn validate_pay_blob_sample(
    blob_id: &BlobId,
    chunk_index: u32,
    chunk_count: u32,
) -> Result<(), BlobError> {
    validate_pay_blob_sample_with_max_chunks(
        blob_id,
        chunk_index,
        chunk_count,
        media_blob_das_chunk_count(MAX_MEDIA_BLOB_BYTES) as u32,
    )
}

/// Like [`validate_pay_blob_sample`], but with an explicit max chunk count (Phase 7 raised cap).
pub fn validate_pay_blob_sample_with_max_chunks(
    blob_id: &BlobId,
    chunk_index: u32,
    chunk_count: u32,
    max_chunks: u32,
) -> Result<(), BlobError> {
    let _ = blob_id;
    if chunk_count == 0 || chunk_index >= chunk_count {
        return Err(BlobError::SampleChunkOutOfRange {
            index: chunk_index,
            count: chunk_count,
        });
    }
    if chunk_count > max_chunks {
        return Err(BlobError::SampleChunkOutOfRange {
            index: chunk_index,
            count: chunk_count,
        });
    }
    Ok(())
}

/// Validate PutBlob against an explicit size cap (Phase 7 gated raise).
pub fn validate_put_blob_with_cap(
    retention: BlobRetention,
    content_type: &str,
    data: &[u8],
    max_bytes: usize,
) -> Result<BlobId, BlobError> {
    retention.validate()?;
    if data.len() > max_bytes {
        return Err(BlobError::TooLarge { size: data.len() });
    }
    if content_type.len() > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN {
        return Err(BlobError::ContentTypeTooLong {
            len: content_type.len(),
        });
    }
    Ok(blob_id_of(data))
}

/// Validate BlobAnnounce against an explicit size cap.
pub fn validate_blob_announce_with_cap(
    blob_id: &BlobId,
    size: u32,
    retention: BlobRetention,
    content_type: &str,
    max_bytes: usize,
) -> Result<(), BlobError> {
    retention.validate()?;
    if size == 0 {
        return Err(BlobError::AnnounceSizeZero);
    }
    if size as usize > max_bytes {
        return Err(BlobError::TooLarge {
            size: size as usize,
        });
    }
    if content_type.len() > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN {
        return Err(BlobError::ContentTypeTooLong {
            len: content_type.len(),
        });
    }
    let _ = blob_id;
    Ok(())
}

/// Base gas for renew / sample txs (metadata only).
#[must_use]
pub fn media_blob_meta_tx_gas() -> u64 {
    MEDIA_BLOB_GAS_BASE
}

/// Size-scaled blob gas before retention multiplier.
#[must_use]
pub fn media_blob_gas_raw(size: usize) -> u64 {
    let words = (size as u64).div_ceil(32).max(1);
    MEDIA_BLOB_GAS_BASE.saturating_add(words.saturating_mul(MEDIA_BLOB_GAS_PER_WORD))
}

/// Effective gas charged for PutBlob / BlobAnnounce (raw × retention multiplier).
#[must_use]
pub fn media_blob_gas(size: usize, retention: BlobRetention) -> u64 {
    media_blob_gas_raw(size).saturating_mul(retention.gas_multiplier())
}

/// Soft / header commitment over blob ids referenced by PutBlob / BlobAnnounce in a block (tx order).
#[must_use]
pub fn blobs_root_of_transactions(txs: &[Transaction]) -> Hash {
    let mut ids: Vec<BlobId> = Vec::new();
    for tx in txs {
        match &tx.payload {
            TransactionPayload::PutBlob { data, .. } => ids.push(blob_id_of(data)),
            TransactionPayload::BlobAnnounce { blob_id, .. } => ids.push(*blob_id),
            _ => {}
        }
    }
    if ids.is_empty() {
        return Hash::ZERO;
    }
    let mut h = hasher();
    h.update(b"BOING_BLOBS_ROOT_V1");
    for id in &ids {
        h.update(id);
    }
    Hash(*h.finalize().as_bytes())
}

/// How many DAS data chunks a payload of `size` bytes needs (ceil division).
#[must_use]
pub fn media_blob_das_chunk_count(size: usize) -> usize {
    if size == 0 {
        return 0;
    }
    size.div_ceil(MEDIA_BLOB_DAS_CHUNK_BYTES)
}

/// Whether `size` is within the live / Phase-3-target cap.
#[must_use]
pub fn within_phase3_target_cap(size: usize) -> bool {
    size <= PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES
}

/// Forever pin-rent units (KiB × blocks) for off-chain markets / docs.
#[must_use]
pub fn media_blob_pin_rent_kib_blocks(size: usize, ttl_blocks: u64) -> u128 {
    let kib = (size as u128).div_ceil(1024).max(1);
    kib.saturating_mul(ttl_blocks as u128)
}

/// Chunk descriptor for erasure-coded / DAS paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobChunkRef {
    pub blob_id: BlobId,
    pub chunk_index: u32,
    pub chunk_count: u32,
    /// BLAKE3 of this chunk’s bytes (when available).
    pub chunk_hash: [u8; 32],
}

/// Split payload into fixed-size data chunks plus one XOR parity chunk (simple erasure stub).
///
/// Not Reed–Solomon — single parity recovers **one** missing data chunk. Full coding markets later.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobErasureBundle {
    pub blob_id: BlobId,
    pub original_len: u32,
    pub data_chunks: Vec<Vec<u8>>,
    /// XOR of all data chunks (padded to [`MEDIA_BLOB_DAS_CHUNK_BYTES`]).
    pub parity_chunk: Vec<u8>,
    pub chunk_hashes: Vec<[u8; 32]>,
    /// Merkle root over `chunk_hashes` (data only; parity hashed separately as last leaf optional).
    pub chunks_root: Hash,
}

/// Light-client DAS sample: one data-chunk leaf + sibling path against [`MediaBlobErasureBundle::chunks_root`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobDasSample {
    pub blob_id: BlobId,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub chunk_hash: [u8; 32],
    pub chunks_root: Hash,
    /// Binary merkle siblings from leaf toward root (left/right ordered by index parity).
    pub siblings: Vec<[u8; 32]>,
}

fn chunk_hash_of(bytes: &[u8]) -> [u8; 32] {
    let mut h = hasher();
    h.update(bytes);
    *h.finalize().as_bytes()
}

fn hash_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut h = hasher();
    h.update(b"BOING_BLOB_CHUNK_MERKLE_V1");
    h.update(a);
    h.update(b);
    *h.finalize().as_bytes()
}

/// Binary merkle root over 32-byte leaves (odd last leaf hashed with itself). Empty → [`Hash::ZERO`].
#[must_use]
pub fn media_blob_chunks_merkle_root(leaves: &[[u8; 32]]) -> Hash {
    if leaves.is_empty() {
        return Hash::ZERO;
    }
    let mut layer: Vec<[u8; 32]> = leaves.to_vec();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len().div_ceil(2));
        for pair in layer.chunks(2) {
            let right = if pair.len() == 2 { &pair[1] } else { &pair[0] };
            next.push(hash_pair(&pair[0], right));
        }
        layer = next;
    }
    Hash(layer[0])
}

/// Sibling path for leaf `index` (same tree as [`media_blob_chunks_merkle_root`]).
pub fn media_blob_chunk_merkle_proof(
    leaves: &[[u8; 32]],
    index: usize,
) -> Result<Vec<[u8; 32]>, BlobError> {
    if leaves.is_empty() || index >= leaves.len() {
        return Err(BlobError::ChunkIndexOutOfRange {
            index: index as u32,
            count: leaves.len() as u32,
        });
    }
    let mut siblings = Vec::new();
    let mut layer: Vec<[u8; 32]> = leaves.to_vec();
    let mut idx = index;
    while layer.len() > 1 {
        let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        let sib = if sibling_idx < layer.len() {
            layer[sibling_idx]
        } else {
            layer[idx]
        };
        siblings.push(sib);
        let mut next = Vec::with_capacity(layer.len().div_ceil(2));
        for pair in layer.chunks(2) {
            let right = if pair.len() == 2 { &pair[1] } else { &pair[0] };
            next.push(hash_pair(&pair[0], right));
        }
        layer = next;
        idx /= 2;
    }
    Ok(siblings)
}

/// Verify a DAS sample against its claimed `chunks_root`.
pub fn verify_media_blob_das_sample(sample: &MediaBlobDasSample) -> Result<(), BlobError> {
    if sample.chunk_count == 0 || sample.chunk_index >= sample.chunk_count {
        return Err(BlobError::ChunkIndexOutOfRange {
            index: sample.chunk_index,
            count: sample.chunk_count,
        });
    }
    let mut hash = sample.chunk_hash;
    let mut idx = sample.chunk_index as usize;
    for sib in &sample.siblings {
        hash = if idx % 2 == 0 {
            hash_pair(&hash, sib)
        } else {
            hash_pair(sib, &hash)
        };
        idx /= 2;
    }
    if Hash(hash) != sample.chunks_root {
        return Err(BlobError::DasProofInvalid);
    }
    Ok(())
}

/// Chunk `data` into DAS-sized pieces + XOR parity; compute merkle root over data-chunk hashes.
pub fn chunk_media_blob(data: &[u8]) -> Result<MediaBlobErasureBundle, BlobError> {
    if data.len() > MAX_MEDIA_BLOB_BYTES {
        return Err(BlobError::TooLarge { size: data.len() });
    }
    let blob_id = blob_id_of(data);
    if data.is_empty() {
        return Ok(MediaBlobErasureBundle {
            blob_id,
            original_len: 0,
            data_chunks: vec![],
            parity_chunk: vec![],
            chunk_hashes: vec![],
            chunks_root: Hash::ZERO,
        });
    }
    let mut data_chunks: Vec<Vec<u8>> = data
        .chunks(MEDIA_BLOB_DAS_CHUNK_BYTES)
        .map(|c| {
            let mut v = c.to_vec();
            if v.len() < MEDIA_BLOB_DAS_CHUNK_BYTES {
                v.resize(MEDIA_BLOB_DAS_CHUNK_BYTES, 0);
            }
            v
        })
        .collect();
    let mut parity = vec![0u8; MEDIA_BLOB_DAS_CHUNK_BYTES];
    for ch in &data_chunks {
        for (i, b) in ch.iter().enumerate() {
            parity[i] ^= b;
        }
    }
    let chunk_hashes: Vec<[u8; 32]> = data_chunks.iter().map(|c| chunk_hash_of(c)).collect();
    let chunks_root = media_blob_chunks_merkle_root(&chunk_hashes);
    // Shrink last chunk padding is only for XOR; store original padded form for recovery.
    let _ = &mut data_chunks;
    Ok(MediaBlobErasureBundle {
        blob_id,
        original_len: data.len() as u32,
        data_chunks,
        parity_chunk: parity,
        chunk_hashes,
        chunks_root,
    })
}

/// Build a DAS sample for `chunk_index` from a chunked bundle (light-client hook).
pub fn media_blob_das_sample(
    bundle: &MediaBlobErasureBundle,
    chunk_index: u32,
) -> Result<MediaBlobDasSample, BlobError> {
    let count = bundle.chunk_hashes.len() as u32;
    if chunk_index >= count {
        return Err(BlobError::ChunkIndexOutOfRange {
            index: chunk_index,
            count,
        });
    }
    let siblings = media_blob_chunk_merkle_proof(&bundle.chunk_hashes, chunk_index as usize)?;
    Ok(MediaBlobDasSample {
        blob_id: bundle.blob_id,
        chunk_index,
        chunk_count: count,
        chunk_hash: bundle.chunk_hashes[chunk_index as usize],
        chunks_root: bundle.chunks_root,
        siblings,
    })
}

/// Recover original bytes when at most one data chunk is missing (XOR parity) or all data present.
pub fn recover_media_blob_xor(
    blob_id: &BlobId,
    original_len: usize,
    data_chunks: &[Option<Vec<u8>>],
    parity: &[u8],
) -> Result<Vec<u8>, BlobError> {
    if original_len == 0 {
        let empty = Vec::new();
        if blob_id_of(&empty) != *blob_id {
            return Err(BlobError::RecoveryIdMismatch);
        }
        return Ok(empty);
    }
    let n = data_chunks.len();
    if n == 0 {
        return Err(BlobError::RecoveryInsufficient);
    }
    let missing: Vec<usize> = data_chunks
        .iter()
        .enumerate()
        .filter_map(|(i, c)| if c.is_none() { Some(i) } else { None })
        .collect();
    let mut chunks: Vec<Vec<u8>> = Vec::with_capacity(n);
    if missing.is_empty() {
        for c in data_chunks {
            chunks.push(c.as_ref().expect("checked").clone());
        }
    } else if missing.len() == 1 {
        if parity.len() != MEDIA_BLOB_DAS_CHUNK_BYTES {
            return Err(BlobError::RecoveryInsufficient);
        }
        let miss = missing[0];
        let mut recovered = parity.to_vec();
        for (i, c) in data_chunks.iter().enumerate() {
            if i == miss {
                continue;
            }
            let Some(bytes) = c else {
                return Err(BlobError::RecoveryInsufficient);
            };
            if bytes.len() != MEDIA_BLOB_DAS_CHUNK_BYTES {
                return Err(BlobError::RecoveryInsufficient);
            }
            for (j, b) in bytes.iter().enumerate() {
                recovered[j] ^= b;
            }
        }
        for i in 0..n {
            if i == miss {
                chunks.push(recovered.clone());
            } else {
                chunks.push(data_chunks[i].as_ref().expect("present").clone());
            }
        }
    } else {
        return Err(BlobError::RecoveryInsufficient);
    }
    let mut out = Vec::with_capacity(original_len);
    for ch in &chunks {
        out.extend_from_slice(ch);
    }
    out.truncate(original_len);
    if blob_id_of(&out) != *blob_id {
        return Err(BlobError::RecoveryIdMismatch);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_blake3() {
        let data = b"hello media blob";
        let id = blob_id_of(data);
        let mut h = hasher();
        h.update(data);
        assert_eq!(id, *h.finalize().as_bytes());
        let uri = media_blob_uri(&id);
        assert!(uri.starts_with(MEDIA_BLOB_URI_PREFIX));
        assert_eq!(parse_media_blob_id(&uri), Some(id));
        assert_eq!(parse_media_blob_id(&blob_id_hex(&id)), Some(id));
    }

    #[test]
    fn caps_and_gas_phase4_16mib() {
        assert_eq!(MAX_MEDIA_BLOB_BYTES, 16 * 1024 * 1024);
        assert_eq!(PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES, MAX_MEDIA_BLOB_BYTES);
        assert_eq!(MAX_MEDIA_BLOB_INLINE_PREFER_BYTES, 1024 * 1024);
        assert!(validate_put_blob(BlobRetention::Forever, "image/png", &[0u8; 10]).is_ok());
        assert!(matches!(
            validate_put_blob(BlobRetention::Prunable { ttl_blocks: 0 }, "", &[1]),
            Err(BlobError::TtlZero)
        ));
        let big = vec![0u8; MAX_MEDIA_BLOB_BYTES + 1];
        assert!(matches!(
            validate_put_blob(BlobRetention::Forever, "", &big),
            Err(BlobError::TooLarge { .. })
        ));
        let g = media_blob_gas(32, BlobRetention::Forever);
        assert_eq!(g, media_blob_gas_raw(32) * MEDIA_BLOB_FOREVER_MULTIPLIER);
        assert_eq!(
            BlobRetention::Prunable { ttl_blocks: 10 }.expiry_height(100),
            Some(110)
        );
        assert_eq!(BlobRetention::Forever.expiry_height(100), None);
    }

    #[test]
    fn gossip_and_announce_validate() {
        let data = b"gif".to_vec();
        let id = blob_id_of(&data);
        let g = MediaBlobGossip {
            blob_id: id,
            content_type: "image/gif".into(),
            data: data.clone(),
        };
        g.validate().unwrap();
        let mut bad = g.clone();
        bad.blob_id[0] ^= 1;
        assert!(bad.validate().is_err());
        validate_blob_announce(&id, data.len() as u32, BlobRetention::Forever, "image/gif")
            .unwrap();
        assert!(validate_blob_announce(&id, 0, BlobRetention::Forever, "").is_err());
    }

    #[test]
    fn blobs_root_empty_is_zero() {
        assert_eq!(blobs_root_of_transactions(&[]), Hash::ZERO);
    }

    #[test]
    fn chunk_xor_recover_and_das_sample() {
        let data: Vec<u8> = (0..(MEDIA_BLOB_DAS_CHUNK_BYTES + 100))
            .map(|i| (i % 251) as u8)
            .collect();
        let bundle = chunk_media_blob(&data).unwrap();
        assert_eq!(bundle.data_chunks.len(), 2);
        assert_eq!(bundle.original_len as usize, data.len());

        let mut parts: Vec<Option<Vec<u8>>> = bundle
            .data_chunks
            .iter()
            .cloned()
            .map(Some)
            .collect();
        parts[1] = None;
        let recovered =
            recover_media_blob_xor(&bundle.blob_id, data.len(), &parts, &bundle.parity_chunk)
                .unwrap();
        assert_eq!(recovered, data);

        let sample = media_blob_das_sample(&bundle, 0).unwrap();
        verify_media_blob_das_sample(&sample).unwrap();
        let mut bad = sample.clone();
        bad.chunk_hash[0] ^= 1;
        assert!(verify_media_blob_das_sample(&bad).is_err());
    }
}
