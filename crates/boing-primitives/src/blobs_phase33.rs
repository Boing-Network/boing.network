//! Phase 33: ReservedRealV1 IPA / KZG / SNARK verifier **scaffolding**.
//!
//! Builds on Phase 32 (opt-in BLS multi-pin committee flip 32→33).
//! Scaffolds the ReservedRealV1 IPA/KZG/SNARK path only — does not change flip policy.
//!
//! Honest scope (smallest coherent slice):
//! - Enumerate future backends (`IpaV1` / `KzgV1` / `SnarkV1`) behind `ReservedRealV1`.
//! - `PHASE33_RESERVED_REAL_VERIFIER_WIRED=false` — no real crypto; no heavy crates.
//! - Stub verify APIs run existing envelope validation, then return honest **`NotWired`**.
//! - **Do not** flip [`DasSuccinctProofKind::ReservedRealV1`](crate::blobs_phase8::DasSuccinctProofKind::ReservedRealV1).[`is_implemented`](crate::blobs_phase8::DasSuccinctProofKind::is_implemented) to true.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Preserve Phase 32 flip helpers / env; do not change default unset flip.
//!
//! Remaining after this slice:
//! 1. Wire a real IPA / KZG / SNARK verifier and flip `is_implemented` when production-ready.
//! 2. Coordinated 256 MiB activation.

use crate::blobs::BlobError;
use crate::blobs_phase10::{
    BlobSampleServeChallengeStatus, DasSuccinctProofReserved,
};
use crate::blobs_phase11::validate_das_succinct_proof_reserved_envelope;
use crate::blobs_phase26::{BlsPublicKeyG1, DualKeyIdentity};
use crate::blobs_phase29::BlsAggregateResolvePayload;
use crate::blobs_phase30::BlsAggregateResolveSelfContainedPayload;
use crate::blobs_phase32::{
    phase32_admit_bls_aggregate_resolve_tx, phase32_bls_aggregate_ballot_preimage,
    phase32_bls_committee_flip_active, phase32_cap_activation_checklist,
    phase32_committee_requires_attested, phase32_committee_requires_bls_aggregate,
    phase32_das_succinct_proof_plan, phase32_decode_signer_bitset, phase32_encode_signer_bitset,
    phase32_expired_appeal_prunable, phase32_identities_from_self_contained,
    phase32_is_legacy_electorate_snapshot, phase32_pin_set_commitment, phase32_resolve_deadline,
    phase32_resolve_expired, phase32_resolve_threshold_met, phase32_resolved_appeal_prunable,
    phase32_snapshot_electorate_w, phase32_terminal_serve_challenge_prunable,
    phase32_verify_aggregate_resolve_self_contained, phase32_verify_aggregate_resolve_with_registry,
    Phase32BlsError, PHASE32_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE32_BLS_AGGREGATE_BALLOT_DOMAIN,
    PHASE32_BLS_AGGREGATE_SELF_CONTAINED_WIRED, PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED,
    PHASE32_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED, PHASE32_BLS_AGGREGATE_WIRED,
    PHASE32_BLS_COMMITTEE_FLIP_ENV, PHASE32_BLS_DUAL_KEY_REGISTRY_FILE,
    PHASE32_BLS_DUAL_KEY_REGISTRY_MAGIC, PHASE32_BLS_DUAL_KEY_REGISTRY_VERSION,
    PHASE32_BLS_LIVE_TX32_STILL_ED25519, PHASE32_BLS_POP_REQUIRED,
    PHASE32_BLS_PUBLIC_KEY_G1_BYTES, PHASE32_BLS_SIGNATURE_G2_BYTES, PHASE32_CAP_ACTIVATION_ENV,
    PHASE32_EXPIRED_APPEAL_RETENTION_BLOCKS, PHASE32_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE32_LEGACY_ELECTORATE_W_SENTINEL, PHASE32_LEGACY_PIN_SET_COMMITMENT,
    PHASE32_MAX_APPEAL_EVIDENCE_BYTES, PHASE32_MAX_PIN_SET_LEN, PHASE32_MAX_SETTLE_RECEIPTS,
    PHASE32_PIN_SET_DOMAIN, PHASE32_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX,
    PHASE32_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX, PHASE32_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE32_RESOLVE_THRESHOLD_DEN, PHASE32_RESOLVE_THRESHOLD_NUM, PHASE32_RESOLVE_WINDOW_BLOCKS,
    PHASE32_RESOLVED_APPEAL_RETENTION_BLOCKS, PHASE32_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE32_SETTLE_RECEIPT_TTL_BLOCKS, PHASE32_SUBMIT_SLASH_APPEAL_TX_INDEX,
    PHASE32_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::types::AccountId;

pub const PHASE33_CAP_ACTIVATION_ENV: &str = PHASE32_CAP_ACTIVATION_ENV;
pub const PHASE33_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE32_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
pub const PHASE33_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE32_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
pub const PHASE33_RESOLVE_WINDOW_BLOCKS: u64 = PHASE32_RESOLVE_WINDOW_BLOCKS;
pub const PHASE33_MAX_SETTLE_RECEIPTS: usize = PHASE32_MAX_SETTLE_RECEIPTS;
pub const PHASE33_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE32_SERVE_CHALLENGE_WINDOW_BLOCKS;
pub const PHASE33_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE32_SETTLE_RECEIPT_TTL_BLOCKS;
pub const PHASE33_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE32_EXPIRED_APPEAL_RETENTION_BLOCKS;
pub const PHASE33_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE32_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
pub const PHASE33_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = PHASE32_RESOLVED_APPEAL_RETENTION_BLOCKS;
pub const PHASE33_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE32_MAX_APPEAL_EVIDENCE_BYTES;
pub const PHASE33_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE32_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE33_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE32_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE33_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE32_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
pub const PHASE33_RESOLVE_THRESHOLD_NUM: u64 = PHASE32_RESOLVE_THRESHOLD_NUM;
pub const PHASE33_RESOLVE_THRESHOLD_DEN: u64 = PHASE32_RESOLVE_THRESHOLD_DEN;
pub const PHASE33_PIN_SET_DOMAIN: &[u8] = PHASE32_PIN_SET_DOMAIN;
pub const PHASE33_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE32_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE33_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE32_LEGACY_PIN_SET_COMMITMENT;

pub const PHASE33_BLS_PUBLIC_KEY_G1_BYTES: usize = PHASE32_BLS_PUBLIC_KEY_G1_BYTES;
pub const PHASE33_BLS_SIGNATURE_G2_BYTES: usize = PHASE32_BLS_SIGNATURE_G2_BYTES;
pub const PHASE33_BLS_AGGREGATE_BALLOT_DOMAIN: &[u8] = PHASE32_BLS_AGGREGATE_BALLOT_DOMAIN;
pub const PHASE33_BLS_AGGREGATE_WIRED: bool = PHASE32_BLS_AGGREGATE_WIRED;
pub const PHASE33_BLS_DUAL_KEY_REGISTRY_FILE: &str = PHASE32_BLS_DUAL_KEY_REGISTRY_FILE;
pub const PHASE33_BLS_DUAL_KEY_REGISTRY_MAGIC: &[u8; 4] = PHASE32_BLS_DUAL_KEY_REGISTRY_MAGIC;
pub const PHASE33_BLS_DUAL_KEY_REGISTRY_VERSION: u32 = PHASE32_BLS_DUAL_KEY_REGISTRY_VERSION;
pub const PHASE33_BLS_POP_REQUIRED: bool = PHASE32_BLS_POP_REQUIRED;
pub const PHASE33_MAX_PIN_SET_LEN: usize = PHASE32_MAX_PIN_SET_LEN;
pub const PHASE33_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX: u32 =
    PHASE32_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX;
pub const PHASE33_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED: bool =
    PHASE32_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED;
pub const PHASE33_BLS_AGGREGATE_SELF_CONTAINED_WIRED: bool =
    PHASE32_BLS_AGGREGATE_SELF_CONTAINED_WIRED;
pub const PHASE33_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED: bool =
    PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED;
pub const PHASE33_BLS_LIVE_TX32_STILL_ED25519: bool = PHASE32_BLS_LIVE_TX32_STILL_ED25519;

/// Future IPA / KZG / SNARK backends behind `ReservedRealV1` (names only — none wired).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DasRealVerifierBackend {
    IpaV1,
    KzgV1,
    SnarkV1,
}

impl DasRealVerifierBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IpaV1 => "ipaV1",
            Self::KzgV1 => "kzgV1",
            Self::SnarkV1 => "snarkV1",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::IpaV1, Self::KzgV1, Self::SnarkV1]
    }
}

/// Real ReservedRealV1 verifier is **not** wired — stubs only; no fake crypto.
pub const PHASE33_RESERVED_REAL_VERIFIER_WIRED: bool = false;

/// Stable RPC / plan names for future backends (order matches [`DasRealVerifierBackend::all`]).
pub const PHASE33_RESERVED_REAL_VERIFIER_BACKEND_NAMES: &[&str] = &["ipaV1", "kzgV1", "snarkV1"];

/// Errors from Phase 33 ReservedRealV1 scaffold APIs.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Phase33ReservedRealError {
    #[error("ReservedRealV1 opaque envelope invalid: {0}")]
    Envelope(#[from] BlobError),
    #[error(
        "ReservedRealV1 {backend} verifier not wired (PHASE33_RESERVED_REAL_VERIFIER_WIRED=false); \
         DasSuccinctProofKind::ReservedRealV1.is_implemented() remains false — do not treat opaque proofs as availability"
    )]
    NotWired { backend: &'static str },
}

pub type Phase33BlsError = Phase32BlsError;

/// Active real verifier backend — `None` while [`PHASE33_RESERVED_REAL_VERIFIER_WIRED`] is false.
#[must_use]
pub fn phase33_reserved_real_verifier_backend() -> Option<DasRealVerifierBackend> {
    if PHASE33_RESERVED_REAL_VERIFIER_WIRED {
        // Placeholder for a future single default; never returns Some while flag is false.
        None
    } else {
        None
    }
}

fn not_wired(backend: DasRealVerifierBackend) -> Phase33ReservedRealError {
    Phase33ReservedRealError::NotWired {
        backend: backend.as_str(),
    }
}

/// Map scaffold NotWired into the historical [`BlobError::DasProofInvalid`] reject path.
#[must_use]
pub fn phase33_not_wired_as_blob_error(err: &Phase33ReservedRealError) -> BlobError {
    match err {
        Phase33ReservedRealError::Envelope(e) => e.clone(),
        Phase33ReservedRealError::NotWired { .. } => BlobError::DasProofInvalid,
    }
}

/// Envelope-validate then honestly reject — IPA backend stub.
pub fn phase33_verify_reserved_real_v1_ipa(
    proof: &DasSuccinctProofReserved,
) -> Result<(), Phase33ReservedRealError> {
    validate_das_succinct_proof_reserved_envelope(proof)?;
    Err(not_wired(DasRealVerifierBackend::IpaV1))
}

/// Envelope-validate then honestly reject — KZG backend stub.
pub fn phase33_verify_reserved_real_v1_kzg(
    proof: &DasSuccinctProofReserved,
) -> Result<(), Phase33ReservedRealError> {
    validate_das_succinct_proof_reserved_envelope(proof)?;
    Err(not_wired(DasRealVerifierBackend::KzgV1))
}

/// Envelope-validate then honestly reject — SNARK backend stub.
pub fn phase33_verify_reserved_real_v1_snark(
    proof: &DasSuccinctProofReserved,
) -> Result<(), Phase33ReservedRealError> {
    validate_das_succinct_proof_reserved_envelope(proof)?;
    Err(not_wired(DasRealVerifierBackend::SnarkV1))
}

/// Dispatch ReservedRealV1 verify for a named backend (always NotWired while flag is false).
///
/// Always runs [`validate_das_succinct_proof_reserved_envelope`] first.
pub fn phase33_verify_das_succinct_proof_reserved_real(
    proof: &DasSuccinctProofReserved,
    backend: DasRealVerifierBackend,
) -> Result<(), Phase33ReservedRealError> {
    match backend {
        DasRealVerifierBackend::IpaV1 => phase33_verify_reserved_real_v1_ipa(proof),
        DasRealVerifierBackend::KzgV1 => phase33_verify_reserved_real_v1_kzg(proof),
        DasRealVerifierBackend::SnarkV1 => phase33_verify_reserved_real_v1_snark(proof),
    }
}

/// Convenience: envelope + NotWired → [`BlobError::DasProofInvalid`] (RPC / historical callers).
pub fn phase33_verify_das_succinct_proof_reserved_real_as_blob_error(
    proof: &DasSuccinctProofReserved,
    backend: DasRealVerifierBackend,
) -> Result<(), BlobError> {
    phase33_verify_das_succinct_proof_reserved_real(proof, backend)
        .map_err(|e| phase33_not_wired_as_blob_error(&e))
}

pub fn phase33_admit_bls_aggregate_resolve_tx() -> Result<(), Phase33BlsError> {
    phase32_admit_bls_aggregate_resolve_tx()
}

pub fn phase33_bls_aggregate_ballot_preimage(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    pin_set_commitment: &[u8; 32],
) -> [u8; 32] {
    phase32_bls_aggregate_ballot_preimage(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        pin_set_commitment,
    )
}

pub fn phase33_encode_signer_bitset(
    pinned: &[AccountId],
    signers: &[AccountId],
) -> Result<Vec<u8>, Phase33BlsError> {
    phase32_encode_signer_bitset(pinned, signers)
}

pub fn phase33_decode_signer_bitset(
    pinned: &[AccountId],
    bitset: &[u8],
) -> Result<Vec<AccountId>, Phase33BlsError> {
    phase32_decode_signer_bitset(pinned, bitset)
}

pub fn phase33_identities_from_self_contained(
    pinned: &[AccountId],
    bitset: &[u8],
    pubkeys: &[BlsPublicKeyG1],
) -> Result<Vec<DualKeyIdentity>, Phase33BlsError> {
    phase32_identities_from_self_contained(pinned, bitset, pubkeys)
}

pub fn phase33_verify_aggregate_resolve_self_contained(
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolveSelfContainedPayload,
) -> Result<(), Phase33BlsError> {
    phase32_verify_aggregate_resolve_self_contained(
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        payload,
    )
}

pub fn phase33_verify_aggregate_resolve_with_registry(
    registry: &crate::blobs_phase27::BlsDualKeyRegistry,
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolvePayload,
) -> Result<(), Phase33BlsError> {
    phase32_verify_aggregate_resolve_with_registry(
        registry,
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        payload,
    )
}

pub fn phase33_resolve_deadline(submit_height: u64) -> u64 {
    phase32_resolve_deadline(submit_height)
}

pub fn phase33_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase32_resolve_expired(tip_height, resolve_deadline)
}

pub fn phase33_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase32_expired_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase33_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase32_resolved_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase33_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase32_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase33_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase32_snapshot_electorate_w(pinned_stakes)
}

pub fn phase33_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase32_pin_set_commitment(pinned)
}

pub fn phase33_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase32_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase33_committee_requires_attested(
    pinned_len: usize,
    height: u64,
    flip_from: Option<u64>,
) -> bool {
    phase32_committee_requires_attested(pinned_len, height, flip_from)
}

pub fn phase33_bls_committee_flip_active(height: u64, flip_from: Option<u64>) -> bool {
    phase32_bls_committee_flip_active(height, flip_from)
}

pub fn phase33_committee_requires_bls_aggregate(
    pinned_len: usize,
    height: u64,
    flip_from: Option<u64>,
) -> bool {
    phase32_committee_requires_bls_aggregate(pinned_len, height, flip_from)
}

pub const PHASE33_BLS_COMMITTEE_FLIP_ENV: &str = PHASE32_BLS_COMMITTEE_FLIP_ENV;

pub fn phase33_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase32_resolve_threshold_met(weight, electorate_w)
}

pub fn phase33_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase32_das_succinct_proof_plan();
    v.extend([
        "Phase 33: scaffold ReservedRealV1 IPA/KZG/SNARK verifier backends (ipaV1/kzgV1/snarkV1) — PHASE33_RESERVED_REAL_VERIFIER_WIRED=false; stubs return NotWired after envelope checks.",
        "Phase 33: DasSuccinctProofKind::ReservedRealV1.is_implemented() remains false — do not flip without a real verifier; no arkworks/halo2 deps yet.",
        "Phase 33: builds on Phase 32 opt-in BLS committee flip — ReservedRealV1 scaffold only; flip env default remains unset.",
        "Phase 33: remaining — wire a real IPA/KZG/SNARK verifier and flip is_implemented when production-ready; coordinated 256 MiB still later.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

pub fn phase33_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase32_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 33: ReservedRealV1 verifier scaffold only — leave PHASE33_RESERVED_REAL_VERIFIER_WIRED=false; do not treat opaque proofs as DAS availability.",
        "Phase 33: do not clear slash_appeals_registry.bin — stay V5; dual-keys remain host-local BDKR v1.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs_phase10::build_das_succinct_proof_reserved_placeholder;
    use crate::blobs_phase8::DasSuccinctProofKind;

    #[test]
    fn reserved_real_not_wired_and_is_implemented_false() {
        assert!(!PHASE33_RESERVED_REAL_VERIFIER_WIRED);
        assert!(phase33_reserved_real_verifier_backend().is_none());
        assert!(!DasSuccinctProofKind::ReservedRealV1.is_implemented());
        assert!(DasSuccinctProofKind::StubBlake3V0.is_implemented());
        assert_eq!(
            PHASE33_RESERVED_REAL_VERIFIER_BACKEND_NAMES,
            &["ipaV1", "kzgV1", "snarkV1"]
        );
        assert_eq!(DasRealVerifierBackend::IpaV1.as_str(), "ipaV1");
        assert_eq!(DasRealVerifierBackend::KzgV1.as_str(), "kzgV1");
        assert_eq!(DasRealVerifierBackend::SnarkV1.as_str(), "snarkV1");
    }

    #[test]
    fn stubs_envelope_then_not_wired() {
        let p = build_das_succinct_proof_reserved_placeholder(
            &[9u8; 32],
            &[8u8; 32],
            1,
            &[0, 1],
            &[0u8; 64],
        )
        .unwrap();
        validate_das_succinct_proof_reserved_envelope(&p).unwrap();

        let ipa = phase33_verify_reserved_real_v1_ipa(&p).unwrap_err();
        assert!(matches!(
            ipa,
            Phase33ReservedRealError::NotWired { backend: "ipaV1" }
        ));
        let kzg = phase33_verify_reserved_real_v1_kzg(&p).unwrap_err();
        assert!(matches!(
            kzg,
            Phase33ReservedRealError::NotWired { backend: "kzgV1" }
        ));
        let snark = phase33_verify_reserved_real_v1_snark(&p).unwrap_err();
        assert!(matches!(
            snark,
            Phase33ReservedRealError::NotWired { backend: "snarkV1" }
        ));
        let disp = phase33_verify_das_succinct_proof_reserved_real(
            &p,
            DasRealVerifierBackend::SnarkV1,
        )
        .unwrap_err();
        let msg = disp.to_string();
        assert!(msg.contains("not wired"));
        assert!(msg.contains("is_implemented() remains false"));
        assert_eq!(
            phase33_verify_das_succinct_proof_reserved_real_as_blob_error(
                &p,
                DasRealVerifierBackend::IpaV1
            ),
            Err(BlobError::DasProofInvalid)
        );
    }

    #[test]
    fn stubs_reject_bad_envelope_before_not_wired() {
        let mut p = build_das_succinct_proof_reserved_placeholder(
            &[9u8; 32],
            &[8u8; 32],
            1,
            &[0],
            &[1u8; 8],
        )
        .unwrap();
        p.commitment[0] ^= 0xff;
        let err = phase33_verify_reserved_real_v1_ipa(&p).unwrap_err();
        assert!(matches!(err, Phase33ReservedRealError::Envelope(_)));
    }

    #[test]
    fn plan_and_checklist_note_scaffold() {
        let plan = phase33_das_succinct_proof_plan();
        assert!(plan.iter().any(|s| s.contains("Phase 33")));
        assert!(plan.iter().any(|s| s.contains("PHASE33_RESERVED_REAL_VERIFIER_WIRED=false")));
        assert!(plan.iter().any(|s| s.contains("is_implemented() remains false")));
        assert!(plan.iter().any(|s| s.contains("Phase 32")));
        let checklist = phase33_cap_activation_checklist(None);
        assert!(checklist
            .iter()
            .any(|s| s.contains("PHASE7") || s.contains("16 MiB")));
        assert!(checklist.iter().any(|s| s.contains("Phase 33")));
    }
}
