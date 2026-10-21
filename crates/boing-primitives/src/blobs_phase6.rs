//! Phase 6 scaffolds (not consensus-activated): sampling market matching, DAS light-client
//! batch verification, and multi-minute / segmented video path helpers.
//!
//! Live cap remains [`crate::MAX_MEDIA_BLOB_BYTES`] (16 MiB). Targets below are design hooks only.

use serde::{Deserialize, Serialize};

use crate::blobs::{
    media_blob_das_chunk_count, verify_media_blob_das_sample, BlobError, BlobId, MediaBlobDasSample,
    MAX_MEDIA_BLOB_BYTES,
};
use crate::AccountId;

/// Design target for a future activation (not live). Roughly enough for short multi-minute
/// compressed video when paired with Announce + segments — still not planet-scale DA.
pub const PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES: usize = 256 * 1024 * 1024;

/// Suggested max bytes per video **segment** blob under the Phase 6 path (still ≤ live cap today).
pub const PHASE6_VIDEO_SEGMENT_TARGET_BYTES: usize = MAX_MEDIA_BLOB_BYTES;

/// Max segments in a [`MediaBlobSegmentManifest`] scaffold.
pub const PHASE6_MAX_VIDEO_SEGMENTS: usize = 256;

/// Max paid-sample offers tracked in one in-memory market batch (scaffold).
pub const PHASE6_MAX_SAMPLE_OFFERS: usize = 1_024;

/// Light-client: max random samples verified in one batch.
pub const PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES: usize = 32;

/// Domain tag for future succinct / aggregated DAS commitments (placeholder).
pub const PHASE6_DAS_LIGHT_CLIENT_DOMAIN: &[u8] = b"BOING_BLOB_DAS_LC_V0";

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
    /// Offer expiry height (0 = no expiry in scaffold).
    pub expires_at_height: u64,
}

/// Matched sample: payer + provider + fee (feeds future matcher; Phase 5 only has `PayBlobSample`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobSampleMatch {
    pub offer: MediaBlobSampleOffer,
    pub payer: AccountId,
    pub paid_fee: u128,
}

/// Ordered multi-blob video / long-media manifest (each segment is its own BlobId).
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
    pub samples: Vec<MediaBlobDasSample>,
}

impl MediaBlobSampleOffer {
    pub fn validate(&self) -> Result<(), BlobError> {
        if self.chunk_count == 0 || self.chunk_index >= self.chunk_count {
            return Err(BlobError::ChunkIndexOutOfRange {
                index: self.chunk_index,
                count: self.chunk_count,
            });
        }
        Ok(())
    }
}

/// Scaffold matcher: accept when `paid_fee >= ask_fee` and indices align with `PayBlobSample`.
pub fn match_sample_offer(
    offer: &MediaBlobSampleOffer,
    payer: AccountId,
    paid_fee: u128,
) -> Result<MediaBlobSampleMatch, BlobError> {
    offer.validate()?;
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
    })
}

impl MediaBlobSegmentManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.segment_blob_ids.is_empty() {
            return Err("segment manifest empty".into());
        }
        if self.segment_blob_ids.len() > PHASE6_MAX_VIDEO_SEGMENTS {
            return Err(format!(
                "too many segments (max {})",
                PHASE6_MAX_VIDEO_SEGMENTS
            ));
        }
        if self.total_bytes == 0 {
            return Err("total_bytes must be > 0".into());
        }
        // Design check only: live consensus still rejects Put/Announce above 16 MiB.
        let max_target = PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES as u64;
        if self.total_bytes > max_target {
            return Err(format!(
                "total_bytes exceeds Phase 6 target {max_target} (not a live consensus rule)"
            ));
        }
        Ok(())
    }

    /// Suggested segment count if splitting `total_bytes` into live-cap chunks.
    pub fn suggested_segment_count(total_bytes: u64) -> usize {
        if total_bytes == 0 {
            return 0;
        }
        let seg = PHASE6_VIDEO_SEGMENT_TARGET_BYTES as u64;
        ((total_bytes + seg - 1) / seg) as usize
    }
}

impl MediaBlobDasLightClientBatch {
    pub fn validate_structure(&self) -> Result<(), BlobError> {
        if self.samples.is_empty() {
            return Err(BlobError::ChunkIndexOutOfRange {
                index: 0,
                count: 0,
            });
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
}

/// Deterministic sample indices for a light client (scaffold; not VRF-binding yet).
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
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        out.push((x % n as u64) as u32);
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::{chunk_media_blob, media_blob_das_sample};

    #[test]
    fn sample_offer_match_requires_fee() {
        let offer = MediaBlobSampleOffer {
            blob_id: [9u8; 32],
            chunk_index: 0,
            chunk_count: 4,
            provider: AccountId([1u8; 32]),
            ask_fee: 10,
            expires_at_height: 0,
        };
        assert!(match_sample_offer(&offer, AccountId([2u8; 32]), 9).is_err());
        let m = match_sample_offer(&offer, AccountId([2u8; 32]), 10).unwrap();
        assert_eq!(m.paid_fee, 10);
    }

    #[test]
    fn segment_manifest_suggests_counts() {
        assert_eq!(MediaBlobSegmentManifest::suggested_segment_count(0), 0);
        assert_eq!(
            MediaBlobSegmentManifest::suggested_segment_count(MAX_MEDIA_BLOB_BYTES as u64),
            1
        );
        assert_eq!(
            MediaBlobSegmentManifest::suggested_segment_count(
                MAX_MEDIA_BLOB_BYTES as u64 * 3 + 1
            ),
            4
        );
        let m = MediaBlobSegmentManifest {
            content_type: "video/mp4".into(),
            total_bytes: 32 * 1024 * 1024,
            segment_blob_ids: vec![[1u8; 32]; 2],
        };
        m.validate().unwrap();
    }

    #[test]
    fn light_client_batch_verifies_merkle_samples() {
        let data = vec![7u8; 130_000];
        let bundle = chunk_media_blob(&data).unwrap();
        let idxs = das_light_client_sample_indices(data.len(), 3, 42);
        assert!(!idxs.is_empty());
        let samples: Vec<_> = idxs
            .iter()
            .map(|&i| media_blob_das_sample(&bundle, i).unwrap())
            .collect();
        let batch = MediaBlobDasLightClientBatch {
            blob_id: bundle.blob_id,
            chunks_root: bundle.chunks_root.0,
            samples,
        };
        batch.validate_structure().unwrap();
    }

    #[test]
    fn phase6_target_is_above_live_cap() {
        assert!(PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES > MAX_MEDIA_BLOB_BYTES);
    }
}
