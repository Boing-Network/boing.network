//! Phase 11: serve-challenge bond + FailedServe balance slash helpers,
//! ReservedRealV1 envelope validation progress, activation notes.
//!
//! Honest scope:
//! - Challenge escrows challenger bond + provider penalty; Respond pays both to provider;
//!   FailedServe refunds bond to challenger and burns the escrowed provider penalty.
//! - ReservedRealV1 gains envelope structure checks; **verify still rejects**
//!   as availability evidence (not a SNARK / IPA / KZG verifier).
//! - Cap activation checklist extended; default remains unset → 16 MiB Fly-safe.

use crate::blobs::BlobError;
use crate::blobs_phase10::{
    build_das_succinct_proof_reserved_placeholder, phase10_cap_activation_checklist,
    phase10_das_succinct_proof_plan, verify_das_succinct_proof_reserved, DasSuccinctProofReserved,
    PHASE10_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE10_CAP_ACTIVATION_ENV,
    PHASE10_MAX_OPAQUE_PROOF_BYTES, PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS,
};
use crate::blobs_phase8::DasSuccinctProofKind;
use crate::denomination::BASE_UNITS_PER_BOING;
use crate::hasher;
use crate::AccountId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE11_CAP_ACTIVATION_ENV: &str = PHASE10_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE11_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE10_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Serve-challenge response window (same as Phase 10).
pub const PHASE11_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Domain for the system serve-challenge bond escrow account id.
pub const PHASE11_SERVE_CHALLENGE_BOND_ESCROW_DOMAIN: &[u8] = b"BOING_BLOB_SERVE_CHALLENGE_BOND_V1";

/// Fixed challenger bond escrowed on Challenge (1 BOING).
pub const PHASE11_SERVE_CHALLENGE_BOND: u128 = BASE_UNITS_PER_BOING;

/// Fixed provider balance burned to the fee sink on FailedServe (1 BOING).
pub const PHASE11_FAILED_SERVE_PENALTY: u128 = BASE_UNITS_PER_BOING;

/// Appeal-window hint for operator SlashRegistry wiring (blocks).
pub const PHASE11_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 = 1000;

/// SlashReason string / typed reason tag for FailedServe balance burns.
pub const PHASE11_FAILED_SERVE_SLASH_REASON: &str = "media_blob_failed_serve";

/// Fixed system account that holds escrowed serve-challenge bonds.
#[must_use]
pub fn blob_sample_serve_challenge_bond_escrow_account() -> AccountId {
    let mut h = hasher();
    h.update(PHASE11_SERVE_CHALLENGE_BOND_ESCROW_DOMAIN);
    AccountId(*h.finalize().as_bytes())
}

/// Validate ReservedRealV1 opaque envelope structure (kind, size, commitment rebind).
///
/// This does **not** prove availability. Callers must still treat
/// [`verify_das_succinct_proof_reserved_phase11`] as rejecting.
pub fn validate_das_succinct_proof_reserved_envelope(
    proof: &DasSuccinctProofReserved,
) -> Result<(), BlobError> {
    if proof.kind != DasSuccinctProofKind::ReservedRealV1.as_u8() {
        return Err(BlobError::DasProofInvalid);
    }
    if proof.opaque_proof.len() > PHASE10_MAX_OPAQUE_PROOF_BYTES {
        return Err(BlobError::DasProofInvalid);
    }
    let rebuilt = build_das_succinct_proof_reserved_placeholder(
        &proof.blob_id,
        &proof.chunks_root,
        proof.seed_height,
        &proof.sample_indices,
        &proof.opaque_proof,
    )?;
    if rebuilt.commitment != proof.commitment {
        return Err(BlobError::DasProofInvalid);
    }
    Ok(())
}

/// Phase 11 ReservedRealV1 verify: envelope may validate, but availability verify **always** rejects.
pub fn verify_das_succinct_proof_reserved_phase11(
    proof: &DasSuccinctProofReserved,
) -> Result<(), BlobError> {
    validate_das_succinct_proof_reserved_envelope(proof)?;
    verify_das_succinct_proof_reserved(proof)
}

/// Operator-facing plan strings for the succinct DAS path (Phase 11 progress).
pub fn phase11_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase10_das_succinct_proof_plan();
    v.extend([
        "Phase 11: ReservedRealV1 envelope structure checks (kind/size/commitment rebind) — still not availability evidence.",
        "ReservedRealV1 verify remains reject-until-wired; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 10 + Phase 11 bond/slash notes).
pub fn phase11_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase10_cap_activation_checklist(activation_height);
    v.extend([
        "After activation, serve-challenge bond (challenger) and FailedServe provider penalty are live; keep identical binaries before raising caps.",
        "FailedServe settles automatically at block end and on challenge/respond touch — bond refund + provider penalty burn.",
        "ReservedRealV1 envelope checks are structural only; do not treat opaque proofs as DAS availability until verify is implemented.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bond_escrow_account_stable() {
        let a = blob_sample_serve_challenge_bond_escrow_account();
        let b = blob_sample_serve_challenge_bond_escrow_account();
        assert_eq!(a, b);
        assert_ne!(a, AccountId([0u8; 32]));
        assert_eq!(PHASE11_SERVE_CHALLENGE_BOND, BASE_UNITS_PER_BOING);
        assert_eq!(PHASE11_FAILED_SERVE_PENALTY, BASE_UNITS_PER_BOING);
    }

    #[test]
    fn reserved_envelope_validates_but_verify_rejects() {
        let p = build_das_succinct_proof_reserved_placeholder(
            &[9u8; 32],
            &[8u8; 32],
            1,
            &[0, 1],
            &[0u8; 64],
        )
        .unwrap();
        validate_das_succinct_proof_reserved_envelope(&p).unwrap();
        assert!(verify_das_succinct_proof_reserved_phase11(&p).is_err());
    }

    #[test]
    fn reserved_envelope_rejects_tampered_commitment() {
        let mut p = build_das_succinct_proof_reserved_placeholder(
            &[9u8; 32],
            &[8u8; 32],
            1,
            &[0],
            &[1u8; 8],
        )
        .unwrap();
        p.commitment[0] ^= 0xff;
        assert!(validate_das_succinct_proof_reserved_envelope(&p).is_err());
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase11_cap_activation_checklist(None).len() >= 10);
        assert!(phase11_das_succinct_proof_plan().len() >= 5);
    }
}
