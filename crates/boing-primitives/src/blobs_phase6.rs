//! Phase 6 helpers (not consensus-activated): sampling-market book, DAS light-client batches,
//! and multi-minute / segmented video path utilities.
//!
//! Live Put/Announce cap remains [`crate::MAX_MEDIA_BLOB_BYTES`] (16 MiB). Design targets below
//! are **not** live consensus rules — do not raise the live cap without an explicit activation gate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::blobs::{
    blob_id_hex, blob_id_of, chunk_media_blob, media_blob_das_chunk_count, media_blob_das_sample,
    validate_pay_blob_sample, verify_media_blob_das_sample, BlobError, BlobId, MediaBlobDasSample,
    MediaBlobErasureBundle, MAX_MEDIA_BLOB_BYTES, MAX_MEDIA_BLOB_CONTENT_TYPE_LEN,
};
use crate::hasher;
use crate::AccountId;

/// Design target for a future activation (not live). Roughly enough for short multi-minute
/// compressed video when paired with Announce + segments — still not planet-scale DA.
pub const PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES: usize = 256 * 1024 * 1024;

/// Suggested max bytes per video **segment** blob under the Phase 6 path (still ≤ live cap today).
pub const PHASE6_VIDEO_SEGMENT_TARGET_BYTES: usize = MAX_MEDIA_BLOB_BYTES;

/// Max segments in a [`MediaBlobSegmentManifest`].
pub const PHASE6_MAX_VIDEO_SEGMENTS: usize = 256;

/// Max paid-sample offers tracked in one in-memory market book.
pub const PHASE6_MAX_SAMPLE_OFFERS: usize = 1_024;

/// Light-client: max random samples verified in one batch.
pub const PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES: usize = 32;

/// Domain tag for DAS light-client seed binding / future succinct commitments.
pub const PHASE6_DAS_LIGHT_CLIENT_DOMAIN: &[u8] = b"BOING_BLOB_DAS_LC_V0";

/// Domain tag for segment-manifest content addressing.
pub const PHASE6_SEGMENT_MANIFEST_DOMAIN: &[u8] = b"BOING_BLOB_SEG_MANIFEST_V0";

/// Off-chain / sidecar sample **offer** (provider advertises availability).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobSampleOffer {
    pub blob_id: BlobId,
    pub chunk_index: u32,
    pub chunk_count: u32,
    /// Provider identity (sampler / full node).
    pub provider: AccountId,
    /// Asking price in ledger units (0 = free / tip-only).
    pub ask_fee: u128,
    /// Offer expiry height (0 = no expiry).
    pub expires_at_height: u64,
}

/// Matched sample: payer + provider + fee (pairs with on-chain [`crate::TransactionPayload::PayBlobSample`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobSampleMatch {
    pub offer: MediaBlobSampleOffer,
    pub payer: AccountId,
    pub paid_fee: u128,
    pub matched_at_height: u64,
}

/// Ordered multi-blob video / long-media manifest (each segment is its own BlobId ≤ live cap).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobSegmentManifest {
    pub content_type: String,
    pub total_bytes: u64,
    pub segment_blob_ids: Vec<BlobId>,
}

/// Batch of DAS merkle samples for a light client (no succinct proof yet).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobDasLightClientBatch {
    pub blob_id: BlobId,
    pub chunks_root: [u8; 32],
    /// Height / tip binding used for [`das_light_client_seed`].
    pub seed_height: u64,
    pub samples: Vec<MediaBlobDasSample>,
}

/// In-memory (or sidecar-persisted) sample offer book — **not** consensus state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobSampleMarket {
    /// Key: `(blob_id_hex, chunk_index, provider_hex)` → offer.
    offers: BTreeMap<String, MediaBlobSampleOffer>,
}

impl MediaBlobSampleOffer {
    pub fn validate(&self) -> Result<(), BlobError> {
        validate_pay_blob_sample(&self.blob_id, self.chunk_index, self.chunk_count)
    }

    pub fn is_expired(&self, tip_height: u64) -> bool {
        self.expires_at_height > 0 && tip_height > self.expires_at_height
    }

    fn market_key(&self) -> String {
        format!(
            "{}:{}:{}",
            blob_id_hex(&self.blob_id),
            self.chunk_index,
            hex::encode(self.provider.0)
        )
    }
}

impl MediaBlobSampleMarket {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.offers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.offers.is_empty()
    }

    /// Insert or replace an offer. Enforces [`PHASE6_MAX_SAMPLE_OFFERS`] for new keys.
    pub fn upsert_offer(&mut self, offer: MediaBlobSampleOffer) -> Result<(), BlobError> {
        offer.validate()?;
        let key = offer.market_key();
        if !self.offers.contains_key(&key) && self.offers.len() >= PHASE6_MAX_SAMPLE_OFFERS {
            return Err(BlobError::SampleMarketFull {
                max: PHASE6_MAX_SAMPLE_OFFERS,
            });
        }
        self.offers.insert(key, offer);
        Ok(())
    }

    pub fn remove_offer(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
    ) -> bool {
        let key = format!(
            "{}:{}:{}",
            blob_id_hex(blob_id),
            chunk_index,
            hex::encode(provider.0)
        );
        self.offers.remove(&key).is_some()
    }

    /// Drop offers with `expires_at_height > 0` and tip past expiry.
    pub fn prune_expired(&mut self, tip_height: u64) -> usize {
        let before = self.offers.len();
        self.offers.retain(|_, o| !o.is_expired(tip_height));
        before.saturating_sub(self.offers.len())
    }

    /// All non-expired offers for a blob (optional chunk filter).
    pub fn list_offers(
        &self,
        blob_id: &BlobId,
        chunk_index: Option<u32>,
        tip_height: u64,
    ) -> Vec<&MediaBlobSampleOffer> {
        let want = blob_id_hex(blob_id);
        self.offers
            .values()
            .filter(|o| {
                blob_id_hex(&o.blob_id) == want
                    && !o.is_expired(tip_height)
                    && chunk_index.is_none_or(|i| o.chunk_index == i)
            })
            .collect()
    }

    /// Cheapest non-expired offer for `(blob_id, chunk_index)`.
    pub fn best_offer(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        tip_height: u64,
    ) -> Option<&MediaBlobSampleOffer> {
        self.list_offers(blob_id, Some(chunk_index), tip_height)
            .into_iter()
            .min_by_key(|o| o.ask_fee)
    }

    /// Match a payer's `PayBlobSample` payment against the best offer (or a specific provider).
    pub fn match_pay_blob_sample(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        chunk_count: u32,
        payer: AccountId,
        paid_fee: u128,
        tip_height: u64,
        provider: Option<&AccountId>,
    ) -> Result<MediaBlobSampleMatch, BlobError> {
        validate_pay_blob_sample(blob_id, chunk_index, chunk_count)?;
        let offer = if let Some(p) = provider {
            let key = format!(
                "{}:{}:{}",
                blob_id_hex(blob_id),
                chunk_index,
                hex::encode(p.0)
            );
            self.offers
                .get(&key)
                .ok_or(BlobError::SampleOfferNotFound)?
        } else {
            self.best_offer(blob_id, chunk_index, tip_height)
                .ok_or(BlobError::SampleOfferNotFound)?
        };
        if offer.chunk_count != chunk_count {
            return Err(BlobError::SampleChunkOutOfRange {
                index: chunk_index,
                count: chunk_count,
            });
        }
        if offer.is_expired(tip_height) {
            return Err(BlobError::SampleOfferExpired {
                expires_at_height: offer.expires_at_height,
                tip_height,
            });
        }
        match_sample_offer(offer, payer, paid_fee, tip_height)
    }
}

/// Accept when `paid_fee >= ask_fee` and the offer is structurally valid / not expired.
pub fn match_sample_offer(
    offer: &MediaBlobSampleOffer,
    payer: AccountId,
    paid_fee: u128,
    tip_height: u64,
) -> Result<MediaBlobSampleMatch, BlobError> {
    offer.validate()?;
    if offer.is_expired(tip_height) {
        return Err(BlobError::SampleOfferExpired {
            expires_at_height: offer.expires_at_height,
            tip_height,
        });
    }
    if paid_fee < offer.ask_fee {
        return Err(BlobError::SampleFeeTooLow {
            paid: paid_fee,
            ask: offer.ask_fee,
        });
    }
    Ok(MediaBlobSampleMatch {
        offer: offer.clone(),
        payer,
        paid_fee,
        matched_at_height: tip_height,
    })
}

impl MediaBlobSegmentManifest {
    pub fn validate(&self) -> Result<(), BlobError> {
        if self.segment_blob_ids.is_empty() {
            return Err(BlobError::SegmentManifestEmpty);
        }
        if self.segment_blob_ids.len() > PHASE6_MAX_VIDEO_SEGMENTS {
            return Err(BlobError::SegmentManifestTooMany {
                count: self.segment_blob_ids.len(),
                max: PHASE6_MAX_VIDEO_SEGMENTS,
            });
        }
        if self.total_bytes == 0 {
            return Err(BlobError::SegmentManifestEmpty);
        }
        if self.content_type.len() > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN {
            return Err(BlobError::ContentTypeTooLong {
                len: self.content_type.len(),
            });
        }
        // Design check only — live Put/Announce still rejects > 16 MiB.
        let max_target = PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES as u64;
        if self.total_bytes > max_target {
            return Err(BlobError::SegmentManifestTooLarge {
                total_bytes: self.total_bytes,
                max: max_target,
            });
        }
        Ok(())
    }

    /// Suggested segment count if splitting `total_bytes` into live-cap chunks.
    pub fn suggested_segment_count(total_bytes: u64) -> usize {
        if total_bytes == 0 {
            return 0;
        }
        let seg = PHASE6_VIDEO_SEGMENT_TARGET_BYTES as u64;
        total_bytes.div_ceil(seg) as usize
    }

    /// Content-addressed id of the ordered manifest (not a live blob id of concatenated bytes).
    pub fn manifest_id(&self) -> BlobId {
        let mut h = hasher();
        h.update(PHASE6_SEGMENT_MANIFEST_DOMAIN);
        h.update(&(self.total_bytes.to_le_bytes()));
        h.update(self.content_type.as_bytes());
        h.update(&(self.segment_blob_ids.len() as u32).to_le_bytes());
        for id in &self.segment_blob_ids {
            h.update(id);
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        out
    }
}

/// Split raw bytes into segments each ≤ live [`MAX_MEDIA_BLOB_BYTES`].
///
/// Refuses totals above [`PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES`] (design gate, not consensus).
pub fn split_into_live_cap_segments(data: &[u8]) -> Result<Vec<Vec<u8>>, BlobError> {
    if data.is_empty() {
        return Err(BlobError::SegmentManifestEmpty);
    }
    if data.len() > PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES {
        return Err(BlobError::SegmentManifestTooLarge {
            total_bytes: data.len() as u64,
            max: PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES as u64,
        });
    }
    let seg = PHASE6_VIDEO_SEGMENT_TARGET_BYTES;
    let mut out = Vec::new();
    for chunk in data.chunks(seg) {
        // Each chunk is ≤ live cap by construction — honest for today's PutBlob/Announce.
        debug_assert!(chunk.len() <= MAX_MEDIA_BLOB_BYTES);
        out.push(chunk.to_vec());
    }
    if out.len() > PHASE6_MAX_VIDEO_SEGMENTS {
        return Err(BlobError::SegmentManifestTooMany {
            count: out.len(),
            max: PHASE6_MAX_VIDEO_SEGMENTS,
        });
    }
    Ok(out)
}

/// Build a manifest from already-split segment bytes (each segment hashed under live rules).
pub fn manifest_from_segment_bytes(
    content_type: impl Into<String>,
    segments: &[Vec<u8>],
) -> Result<MediaBlobSegmentManifest, BlobError> {
    if segments.is_empty() {
        return Err(BlobError::SegmentManifestEmpty);
    }
    let mut total = 0u64;
    let mut ids = Vec::with_capacity(segments.len());
    for seg in segments {
        if seg.len() > MAX_MEDIA_BLOB_BYTES {
            return Err(BlobError::TooLarge { size: seg.len() });
        }
        if seg.is_empty() {
            return Err(BlobError::SegmentManifestEmpty);
        }
        total = total.saturating_add(seg.len() as u64);
        ids.push(blob_id_of(seg));
    }
    let m = MediaBlobSegmentManifest {
        content_type: content_type.into(),
        total_bytes: total,
        segment_blob_ids: ids,
    };
    m.validate()?;
    Ok(m)
}

impl MediaBlobDasLightClientBatch {
    pub fn validate_structure(&self) -> Result<(), BlobError> {
        if self.samples.is_empty() {
            return Err(BlobError::ChunkIndexOutOfRange { index: 0, count: 0 });
        }
        if self.samples.len() > PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES {
            return Err(BlobError::DasProofInvalid);
        }
        for s in &self.samples {
            if s.blob_id != self.blob_id {
                return Err(BlobError::DasProofInvalid);
            }
            if s.chunks_root.0 != self.chunks_root {
                return Err(BlobError::DasProofInvalid);
            }
            verify_media_blob_das_sample(s)?;
        }
        Ok(())
    }

    /// Fraction of distinct chunk indices covered (`samples / chunk_count` of first sample).
    pub fn coverage_ratio(&self) -> f64 {
        let Some(first) = self.samples.first() else {
            return 0.0;
        };
        if first.chunk_count == 0 {
            return 0.0;
        }
        let mut uniq: Vec<u32> = self.samples.iter().map(|s| s.chunk_index).collect();
        uniq.sort_unstable();
        uniq.dedup();
        uniq.len() as f64 / first.chunk_count as f64
    }
}

/// Bind light-client sampling to `blob_id` + chain height (honest: not VRF; deterministic).
pub fn das_light_client_seed(blob_id: &BlobId, tip_height: u64) -> u64 {
    let mut h = hasher();
    h.update(PHASE6_DAS_LIGHT_CLIENT_DOMAIN);
    h.update(blob_id);
    h.update(&tip_height.to_le_bytes());
    let dig = h.finalize();
    let b = dig.as_bytes();
    u64::from_le_bytes(b[0..8].try_into().unwrap())
}

/// Deterministic sample indices for a light client.
pub fn das_light_client_sample_indices(
    blob_size: usize,
    sample_count: usize,
    seed: u64,
) -> Vec<u32> {
    let n = media_blob_das_chunk_count(blob_size);
    if n == 0 || sample_count == 0 {
        return vec![];
    }
    let take = sample_count.min(PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES).min(n);
    let mut out = Vec::with_capacity(take);
    let mut x = seed ^ (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for _ in 0..take {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
        out.push((x % n as u64) as u32);
    }
    out.sort_unstable();
    out.dedup();
    // Fill up to `take` if collisions collapsed the set.
    let mut fill = seed.wrapping_add(1);
    while out.len() < take {
        fill = fill.wrapping_mul(6364136223846793005).wrapping_add(1);
        let i = (fill % n as u64) as u32;
        if !out.contains(&i) {
            out.push(i);
        }
        if fill == seed {
            break;
        }
    }
    out.sort_unstable();
    out
}

/// Build + verify a DAS light-client batch from a chunked bundle.
pub fn build_das_light_client_batch(
    bundle: &MediaBlobErasureBundle,
    tip_height: u64,
    sample_count: usize,
) -> Result<MediaBlobDasLightClientBatch, BlobError> {
    let seed = das_light_client_seed(&bundle.blob_id, tip_height);
    let idxs = das_light_client_sample_indices(bundle.original_len as usize, sample_count, seed);
    if idxs.is_empty() {
        return Err(BlobError::ChunkIndexOutOfRange { index: 0, count: 0 });
    }
    let mut samples = Vec::with_capacity(idxs.len());
    for i in idxs {
        samples.push(media_blob_das_sample(bundle, i)?);
    }
    let batch = MediaBlobDasLightClientBatch {
        blob_id: bundle.blob_id,
        chunks_root: bundle.chunks_root.0,
        seed_height: tip_height,
        samples,
    };
    batch.validate_structure()?;
    Ok(batch)
}

/// Convenience: chunk bytes then build a light-client batch.
pub fn build_das_light_client_batch_from_bytes(
    data: &[u8],
    tip_height: u64,
    sample_count: usize,
) -> Result<MediaBlobDasLightClientBatch, BlobError> {
    let bundle = chunk_media_blob(data)?;
    build_das_light_client_batch(&bundle, tip_height, sample_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_market_best_offer_and_match() {
        let mut m = MediaBlobSampleMarket::new();
        let blob = [9u8; 32];
        let cheap = MediaBlobSampleOffer {
            blob_id: blob,
            chunk_index: 0,
            chunk_count: 4,
            provider: AccountId([1u8; 32]),
            ask_fee: 5,
            expires_at_height: 100,
        };
        let dear = MediaBlobSampleOffer {
            blob_id: blob,
            chunk_index: 0,
            chunk_count: 4,
            provider: AccountId([2u8; 32]),
            ask_fee: 50,
            expires_at_height: 100,
        };
        m.upsert_offer(cheap.clone()).unwrap();
        m.upsert_offer(dear).unwrap();
        assert_eq!(m.best_offer(&blob, 0, 50).unwrap().ask_fee, 5);
        assert!(m
            .match_pay_blob_sample(&blob, 0, 4, AccountId([3u8; 32]), 4, 50, None)
            .is_err());
        let matched = m
            .match_pay_blob_sample(&blob, 0, 4, AccountId([3u8; 32]), 5, 50, None)
            .unwrap();
        assert_eq!(matched.offer.provider, cheap.provider);
        // Expired
        assert!(m
            .match_pay_blob_sample(&blob, 0, 4, AccountId([3u8; 32]), 5, 101, None)
            .is_err());
        assert_eq!(m.prune_expired(101), 2);
    }

    #[test]
    fn segment_split_respects_live_cap() {
        let data = vec![1u8; MAX_MEDIA_BLOB_BYTES * 2 + 10];
        let segs = split_into_live_cap_segments(&data).unwrap();
        assert_eq!(segs.len(), 3);
        assert!(segs.iter().all(|s| s.len() <= MAX_MEDIA_BLOB_BYTES));
        let manifest = manifest_from_segment_bytes("video/mp4", &segs).unwrap();
        assert_eq!(manifest.total_bytes, data.len() as u64);
        assert_eq!(manifest.segment_blob_ids.len(), 3);
        let _ = manifest.manifest_id();
        // Over design target
        let huge = vec![0u8; PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES + 1];
        assert!(split_into_live_cap_segments(&huge).is_err());
        // Single segment over live cap is rejected by manifest_from_segment_bytes
        assert!(manifest_from_segment_bytes("x", &[vec![0u8; MAX_MEDIA_BLOB_BYTES + 1]]).is_err());
    }

    #[test]
    fn light_client_batch_from_bytes() {
        let data = vec![7u8; 200_000];
        let batch = build_das_light_client_batch_from_bytes(&data, 42, 8).unwrap();
        assert_eq!(batch.seed_height, 42);
        assert!(batch.coverage_ratio() > 0.0);
        // Same height → same indices
        let batch2 = build_das_light_client_batch_from_bytes(&data, 42, 8).unwrap();
        assert_eq!(
            batch
                .samples
                .iter()
                .map(|s| s.chunk_index)
                .collect::<Vec<_>>(),
            batch2
                .samples
                .iter()
                .map(|s| s.chunk_index)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn phase6_target_is_above_live_cap() {
        assert!(PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES > MAX_MEDIA_BLOB_BYTES);
        assert_eq!(PHASE6_VIDEO_SEGMENT_TARGET_BYTES, MAX_MEDIA_BLOB_BYTES);
    }

    #[test]
    fn match_sample_offer_legacy_helper() {
        let offer = MediaBlobSampleOffer {
            blob_id: [9u8; 32],
            chunk_index: 0,
            chunk_count: 4,
            provider: AccountId([1u8; 32]),
            ask_fee: 10,
            expires_at_height: 0,
        };
        assert!(match_sample_offer(&offer, AccountId([2u8; 32]), 9, 1).is_err());
        let m = match_sample_offer(&offer, AccountId([2u8; 32]), 10, 1).unwrap();
        assert_eq!(m.paid_fee, 10);
    }
}
