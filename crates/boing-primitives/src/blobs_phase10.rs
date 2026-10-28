//! Phase 10: sample-serve challenge helpers, ReservedRealV1 opaque DAS proof path,
//! and activation ops notes (Fly-safe default unchanged).
//!
//! Honest scope:
//! - Challenge / respond **availability** after settle (registry + txs).
//! - Bond / FailedServe slash economics are Phase 11 (`blobs_phase11`).
//! - ReservedRealV1 stores opaque proof bytes but **always rejects** verify (not a SNARK).
//! - Cap activation checklist extended; default remains unset → 16 MiB.

use serde::{Deserialize, Serialize};

use crate::blobs::{verify_media_blob_das_sample, BlobError, BlobId, MediaBlobDasSample};
use crate::blobs_phase8::{DasSuccinctProofKind, PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES};
use crate::blobs_phase9::phase9_cap_activation_checklist;
use crate::hasher;
use crate::AccountId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE10_CAP_ACTIVATION_ENV: &str = "BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT";

/// Activated max after coordinated height.
pub const PHASE10_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Blocks after a serve challenge during which the provider may respond.
pub const PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = 256;

/// Max open serve challenges in the SMT registry.
pub const PHASE10_MAX_SERVE_CHALLENGES: usize = 256;

/// Max opaque proof bytes accepted for ReservedRealV1 placeholders.
pub const PHASE10_MAX_OPAQUE_PROOF_BYTES: usize = 16_384;

/// Domain for the serve-challenge registry SMT leaf.
pub const PHASE10_SERVE_CHALLENGE_REGISTRY_DOMAIN: &[u8] = b"BOING_BLOB_SERVE_CHALLENGE_V1";

/// Domain for ReservedRealV1 opaque commitment binding.
pub const PHASE10_DAS_RESERVED_OPAQUE_DOMAIN: &[u8] = b"BOING_BLOB_DAS_RESERVED_OPAQUE_V0";

/// Status of a sample-serve challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlobSampleServeChallengeStatus {
    Open,
    Responded,
    /// Deadline passed without a valid respond (availability failure; Phase 11 settles bond/slash).
    FailedServe,
}

/// On-chain serve challenge record fields (shared with `boing_state`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobSampleServeChallenge {
    pub blob_id: BlobId,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub provider: AccountId,
    pub challenger: AccountId,
    pub opened_at_height: u64,
    pub deadline_height: u64,
    pub status: BlobSampleServeChallengeStatus,
    /// Challenger bond escrowed at open (Phase 11); 0 = legacy / no bond.
    pub bond_amount: u128,
    /// Provider penalty burned on FailedServe (Phase 11); 0 = none.
    pub provider_penalty: u128,
    /// True after bond (and FailedServe penalty) have been settled.
    pub bond_settled: bool,
    /// Consensus FailedServe slash id (Phase 12); `None` until FailedServe settle assigns one.
    #[serde(default)]
    pub slash_id: Option<u64>,
}

impl BlobSampleServeChallenge {
    pub fn market_key(&self) -> String {
        serve_challenge_key(
            &self.blob_id,
            self.chunk_index,
            &self.provider,
            &self.challenger,
        )
    }
}

#[must_use]
pub fn serve_challenge_key(
    blob_id: &BlobId,
    chunk_index: u32,
    provider: &AccountId,
    challenger: &AccountId,
) -> String {
    format!(
        "{}:{}:{}:{}",
        hex::encode(blob_id),
        chunk_index,
        hex::encode(provider.0),
        hex::encode(challenger.0)
    )
}

pub fn validate_challenge_blob_sample_serve(
    blob_id: &BlobId,
    chunk_index: u32,
    chunk_count: u32,
    provider: &AccountId,
    challenger: &AccountId,
) -> Result<(), BlobError> {
    crate::blobs::validate_pay_blob_sample(blob_id, chunk_index, chunk_count)?;
    if provider == challenger {
        return Err(BlobError::SampleOfferNotFound);
    }
    Ok(())
}

/// Build an open challenge at `tip_height`.
pub fn open_serve_challenge(
    blob_id: BlobId,
    chunk_index: u32,
    chunk_count: u32,
    provider: AccountId,
    challenger: AccountId,
    tip_height: u64,
) -> Result<BlobSampleServeChallenge, BlobError> {
    open_serve_challenge_with_bond(
        blob_id,
        chunk_index,
        chunk_count,
        provider,
        challenger,
        tip_height,
        0,
        0,
    )
}

/// Build an open challenge with explicit bond / provider-penalty amounts (Phase 11).
pub fn open_serve_challenge_with_bond(
    blob_id: BlobId,
    chunk_index: u32,
    chunk_count: u32,
    provider: AccountId,
    challenger: AccountId,
    tip_height: u64,
    bond_amount: u128,
    provider_penalty: u128,
) -> Result<BlobSampleServeChallenge, BlobError> {
    validate_challenge_blob_sample_serve(
        &blob_id,
        chunk_index,
        chunk_count,
        &provider,
        &challenger,
    )?;
    Ok(BlobSampleServeChallenge {
        blob_id,
        chunk_index,
        chunk_count,
        provider,
        challenger,
        opened_at_height: tip_height,
        deadline_height: tip_height.saturating_add(PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS),
        status: BlobSampleServeChallengeStatus::Open,
        bond_amount,
        provider_penalty,
        bond_settled: false,
        slash_id: None,
    })
}

/// Validate a provider response DAS sample against the open challenge.
pub fn validate_respond_blob_sample_serve(
    challenge: &BlobSampleServeChallenge,
    tip_height: u64,
    sample: &MediaBlobDasSample,
) -> Result<(), BlobError> {
    if challenge.status != BlobSampleServeChallengeStatus::Open {
        return Err(BlobError::SampleOfferNotFound);
    }
    if tip_height > challenge.deadline_height {
        return Err(BlobError::SampleOfferExpired {
            expires_at_height: challenge.deadline_height,
            tip_height,
        });
    }
    if sample.blob_id != challenge.blob_id
        || sample.chunk_index != challenge.chunk_index
        || sample.chunk_count != challenge.chunk_count
    {
        return Err(BlobError::DasProofInvalid);
    }
    verify_media_blob_das_sample(sample)?;
    Ok(())
}

/// Opaque ReservedRealV1 envelope (proof bytes stored; verify always fails until a real system).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DasSuccinctProofReserved {
    pub kind: u8,
    pub commitment: [u8; 32],
    pub blob_id: BlobId,
    pub chunks_root: [u8; 32],
    pub seed_height: u64,
    pub sample_indices: Vec<u32>,
    /// Opaque proof bytes (IPA/KZG/SNARK placeholder). Max [`PHASE10_MAX_OPAQUE_PROOF_BYTES`].
    pub opaque_proof: Vec<u8>,
}

/// Build a ReservedRealV1 placeholder (honest: not verifiable as a SNARK).
pub fn build_das_succinct_proof_reserved_placeholder(
    blob_id: &BlobId,
    chunks_root: &[u8; 32],
    seed_height: u64,
    sample_indices: &[u32],
    opaque_proof: &[u8],
) -> Result<DasSuccinctProofReserved, BlobError> {
    if opaque_proof.len() > PHASE10_MAX_OPAQUE_PROOF_BYTES {
        return Err(BlobError::DasProofInvalid);
    }
    let mut h = hasher();
    h.update(PHASE10_DAS_RESERVED_OPAQUE_DOMAIN);
    h.update(&[DasSuccinctProofKind::ReservedRealV1.as_u8()]);
    h.update(blob_id);
    h.update(chunks_root);
    h.update(&seed_height.to_le_bytes());
    h.update(&(sample_indices.len() as u32).to_le_bytes());
    for i in sample_indices {
        h.update(&i.to_le_bytes());
    }
    h.update(&(opaque_proof.len() as u32).to_le_bytes());
    h.update(opaque_proof);
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(h.finalize().as_bytes());
    Ok(DasSuccinctProofReserved {
        kind: DasSuccinctProofKind::ReservedRealV1.as_u8(),
        commitment,
        blob_id: *blob_id,
        chunks_root: *chunks_root,
        seed_height,
        sample_indices: sample_indices.to_vec(),
        opaque_proof: opaque_proof.to_vec(),
    })
}

/// Verify ReservedRealV1 — **always** errors until a real succinct system is wired.
pub fn verify_das_succinct_proof_reserved(
    _proof: &DasSuccinctProofReserved,
) -> Result<(), BlobError> {
    Err(BlobError::DasProofInvalid)
}

/// Operator-facing plan strings for the succinct DAS path.
pub fn phase10_das_succinct_proof_plan() -> Vec<&'static str> {
    vec![
        "StubBlake3V0: BLAKE3 commitment over merkle LC batch — implemented; not a SNARK.",
        "ReservedRealV1: opaque proof envelope accepted for encoding/RPC scaffolding; verify always rejects.",
        "Next: wire IPA/KZG/SNARK verifier behind ReservedRealV1 and flip is_implemented when production-ready.",
        "Leader-VRF LC: use boing_getLeaderVrfOutput / useLeaderVrf when consensus is in VRF mode.",
    ]
}

/// Extended activation checklist (Phase 9 + Phase 10 serve-challenge / DAS notes).
pub fn phase10_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase9_cap_activation_checklist(activation_height);
    v.extend([
        "After activation, sample-serve challenges (txs 28/29) remain available; failed-serve is a signal only — no automatic slash yet.",
        "ReservedRealV1 opaque DAS proofs must not be treated as availability evidence until verify is implemented.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::{chunk_media_blob, media_blob_das_sample};

    #[test]
    fn open_challenge_window() {
        let c = open_serve_challenge(
            [1u8; 32],
            0,
            2,
            AccountId([2u8; 32]),
            AccountId([3u8; 32]),
            10,
        )
        .unwrap();
        assert_eq!(
            c.deadline_height,
            10 + PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS
        );
        assert_eq!(c.status, BlobSampleServeChallengeStatus::Open);
    }

    #[test]
    fn respond_validates_das_sample() {
        let data = vec![7u8; 70_000];
        let bundle = chunk_media_blob(&data).unwrap();
        let sample = media_blob_das_sample(&bundle, 0).unwrap();
        let c = open_serve_challenge(
            bundle.blob_id,
            0,
            sample.chunk_count,
            AccountId([2u8; 32]),
            AccountId([3u8; 32]),
            1,
        )
        .unwrap();
        validate_respond_blob_sample_serve(&c, 2, &sample).unwrap();
    }

    #[test]
    fn reserved_proof_builds_but_verify_rejects() {
        let p = build_das_succinct_proof_reserved_placeholder(
            &[9u8; 32],
            &[8u8; 32],
            1,
            &[0, 1],
            &[0u8; 64],
        )
        .unwrap();
        assert_eq!(p.kind, 1);
        assert!(verify_das_succinct_proof_reserved(&p).is_err());
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase10_cap_activation_checklist(None).len() >= 8);
        assert!(!phase10_das_succinct_proof_plan().is_empty());
    }
}
