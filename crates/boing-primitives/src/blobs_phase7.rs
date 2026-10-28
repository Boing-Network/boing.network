//! Phase 7: on-chain sample offer/settle path, VRF-bound DAS seed helpers, and **gated**
//! live-cap activation toward the Phase 6 design target.
//!
//! Default live Put/Announce cap remains [`crate::MAX_MEDIA_BLOB_BYTES`] (16 MiB). Raising the
//! effective cap requires an explicit activation height — unset keeps Fly / historical boot safe.

use serde::{Deserialize, Serialize};

use crate::blobs::{
    media_blob_das_chunk_count, validate_pay_blob_sample, BlobError, BlobId, MAX_MEDIA_BLOB_BYTES,
};
use crate::blobs_phase6::{
    das_light_client_sample_indices, MediaBlobDasLightClientBatch, PHASE6_DAS_LIGHT_CLIENT_DOMAIN,
    PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES,
};
use crate::hasher;
use crate::AccountId;

/// Env / node knob name (documented): `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT`.
pub const PHASE7_CAP_ACTIVATION_ENV: &str = "BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT";

/// When Phase 7 cap activation is live, Put/Announce admit up to this many bytes.
/// Same numeric target as Phase 6 design (`PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES`).
pub const PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES;

/// Domain for VRF-bound DAS light-client seeds.
pub const PHASE7_DAS_VRF_SEED_DOMAIN: &[u8] = b"BOING_BLOB_DAS_VRF_SEED_V0";

/// Domain for the honest succinct-commitment **stub** (not a real SNARK).
pub const PHASE7_DAS_SUCCINCT_STUB_DOMAIN: &[u8] = b"BOING_BLOB_DAS_SUCCINCT_STUB_V0";

/// Effective Put/Announce max at `height` given optional Phase 7 cap activation.
///
/// - `phase7_cap_from = None` → always [`MAX_MEDIA_BLOB_BYTES`] (16 MiB) — **Fly-safe default**
/// - `Some(h)` and `height >= h` → [`PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES`] (256 MiB)
#[must_use]
pub fn media_blob_max_bytes_at(height: u64, phase7_cap_from: Option<u64>) -> usize {
    match phase7_cap_from {
        Some(from) if height >= from => PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
        _ => MAX_MEDIA_BLOB_BYTES,
    }
}

/// True when the raised cap applies at `height`.
#[must_use]
pub fn phase7_cap_active(height: u64, phase7_cap_from: Option<u64>) -> bool {
    media_blob_max_bytes_at(height, phase7_cap_from) > MAX_MEDIA_BLOB_BYTES
}

/// On-chain sample offer fields (provider = tx sender at registration).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnChainBlobSampleOffer {
    pub blob_id: BlobId,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub provider: AccountId,
    pub ask_fee: u128,
    pub expires_at_height: u64,
    pub registered_at_height: u64,
}

impl OnChainBlobSampleOffer {
    pub fn validate(&self) -> Result<(), BlobError> {
        validate_pay_blob_sample(&self.blob_id, self.chunk_index, self.chunk_count)?;
        Ok(())
    }

    pub fn is_expired(&self, tip_height: u64) -> bool {
        self.expires_at_height > 0 && tip_height > self.expires_at_height
    }
}

/// Validate register-tx fields (provider supplied by caller / tx.sender).
pub fn validate_register_blob_sample_offer(
    blob_id: &BlobId,
    chunk_index: u32,
    chunk_count: u32,
    ask_fee: u128,
    expires_at_height: u64,
) -> Result<(), BlobError> {
    let _ = (ask_fee, expires_at_height);
    validate_pay_blob_sample(blob_id, chunk_index, chunk_count)
}

/// Validate settle-tx fields against a known offer.
pub fn validate_settle_blob_sample_match(
    offer: &OnChainBlobSampleOffer,
    payer: &AccountId,
    tip_height: u64,
    provider: &AccountId,
) -> Result<(), BlobError> {
    let _ = payer;
    offer.validate()?;
    if &offer.provider != provider {
        return Err(BlobError::SampleOfferNotFound);
    }
    if offer.is_expired(tip_height) {
        return Err(BlobError::SampleOfferExpired {
            expires_at_height: offer.expires_at_height,
            tip_height,
        });
    }
    Ok(())
}

/// Bind DAS sample indices to a verified VRF output (32-byte gamma/output).
///
/// Honest scope: uses the VRF **output bytes** as entropy; does not verify the proof here —
/// callers must verify ECVRF before trusting `vrf_output`.
pub fn das_light_client_seed_from_vrf(blob_id: &BlobId, vrf_output: &[u8; 32]) -> u64 {
    let mut h = hasher();
    h.update(PHASE7_DAS_VRF_SEED_DOMAIN);
    h.update(PHASE6_DAS_LIGHT_CLIENT_DOMAIN);
    h.update(blob_id);
    h.update(vrf_output);
    let dig = h.finalize();
    let b = dig.as_bytes();
    u64::from_le_bytes(b[0..8].try_into().unwrap())
}

/// VRF-bound sample indices (same mixer as Phase 6, different seed domain).
pub fn das_light_client_sample_indices_vrf(
    blob_size: usize,
    sample_count: usize,
    blob_id: &BlobId,
    vrf_output: &[u8; 32],
) -> Vec<u32> {
    let seed = das_light_client_seed_from_vrf(blob_id, vrf_output);
    das_light_client_sample_indices(blob_size, sample_count, seed)
}

/// Honest succinct-commitment **stub**: BLAKE3 over batch roots + sample indices.
/// Not a SNARK / IPA / KZG — placeholder for a future proof system.
pub fn das_succinct_commitment_stub(batch: &MediaBlobDasLightClientBatch) -> [u8; 32] {
    let mut h = hasher();
    h.update(PHASE7_DAS_SUCCINCT_STUB_DOMAIN);
    h.update(&batch.blob_id);
    h.update(&batch.chunks_root);
    h.update(&batch.seed_height.to_le_bytes());
    h.update(&(batch.samples.len() as u32).to_le_bytes());
    for s in &batch.samples {
        h.update(&s.chunk_index.to_le_bytes());
        h.update(&s.chunk_hash);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    out
}

/// Max DAS chunks for fee / sample validation at the effective size cap.
#[must_use]
pub fn media_blob_max_das_chunks_at(height: u64, phase7_cap_from: Option<u64>) -> u32 {
    media_blob_das_chunk_count(media_blob_max_bytes_at(height, phase7_cap_from)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::{chunk_media_blob, media_blob_das_sample};
    use crate::blobs_phase6::build_das_light_client_batch;

    #[test]
    fn cap_gate_defaults_to_16mib() {
        assert_eq!(media_blob_max_bytes_at(0, None), MAX_MEDIA_BLOB_BYTES);
        assert_eq!(
            media_blob_max_bytes_at(1_000_000, None),
            MAX_MEDIA_BLOB_BYTES
        );
        assert!(!phase7_cap_active(10, None));
        assert_eq!(media_blob_max_bytes_at(99, Some(100)), MAX_MEDIA_BLOB_BYTES);
        assert_eq!(
            media_blob_max_bytes_at(100, Some(100)),
            PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
        assert!(phase7_cap_active(100, Some(100)));
    }

    #[test]
    fn vrf_seed_differs_from_height_seed_domain() {
        let blob = [1u8; 32];
        let vrf = [9u8; 32];
        let a = das_light_client_seed_from_vrf(&blob, &vrf);
        let b = das_light_client_seed_from_vrf(&blob, &[8u8; 32]);
        assert_ne!(a, b);
        let idxs = das_light_client_sample_indices_vrf(200_000, 4, &blob, &vrf);
        assert!(!idxs.is_empty());
    }

    #[test]
    fn succinct_stub_stable() {
        let data = vec![3u8; 90_000];
        let bundle = chunk_media_blob(&data).unwrap();
        let batch = build_das_light_client_batch(&bundle, 7, 4).unwrap();
        let c1 = das_succinct_commitment_stub(&batch);
        let c2 = das_succinct_commitment_stub(&batch);
        assert_eq!(c1, c2);
        let _ = media_blob_das_sample(&bundle, 0).unwrap();
    }

    #[test]
    fn settle_validates_provider_and_expiry() {
        let offer = OnChainBlobSampleOffer {
            blob_id: [2u8; 32],
            chunk_index: 0,
            chunk_count: 2,
            provider: AccountId([1u8; 32]),
            ask_fee: 1,
            expires_at_height: 50,
            registered_at_height: 10,
        };
        assert!(validate_settle_blob_sample_match(
            &offer,
            &AccountId([3u8; 32]),
            40,
            &AccountId([1u8; 32])
        )
        .is_ok());
        assert!(validate_settle_blob_sample_match(
            &offer,
            &AccountId([3u8; 32]),
            40,
            &AccountId([9u8; 32])
        )
        .is_err());
        assert!(validate_settle_blob_sample_match(
            &offer,
            &AccountId([3u8; 32]),
            51,
            &AccountId([1u8; 32])
        )
        .is_err());
    }
}
