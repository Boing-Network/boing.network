//! Phase 12: FailedServe SlashRegistry audit helpers, settle-before-challenge receipts,
//! and activation ops notes (Fly-safe 16 MiB default unchanged).
//!
//! Honest scope:
//! - Consensus assigns deterministic `slash_id` on FailedServe; nodes mirror into SlashRegistry.
//! - Challenge (28) requires a prior SettleBlobSampleMatch receipt (payer = challenger).
//! - ReservedRealV1 still rejects verify; do not flip `is_implemented`.
//! - Cap activation checklist extended; default remains unset → 16 MiB.

use crate::blobs::BlobError;
use crate::blobs_phase11::{
    phase11_cap_activation_checklist, phase11_das_succinct_proof_plan,
    PHASE11_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE11_CAP_ACTIVATION_ENV,
    PHASE11_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE11_FAILED_SERVE_PENALTY,
    PHASE11_SERVE_CHALLENGE_BOND, PHASE11_SERVE_CHALLENGE_WINDOW_BLOCKS,
};
use crate::AccountId;
use crate::BlobId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE12_CAP_ACTIVATION_ENV: &str = PHASE11_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE12_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE11_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Serve-challenge response window (same as Phase 11).
pub const PHASE12_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE11_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Challenger bond (alias).
pub const PHASE12_SERVE_CHALLENGE_BOND: u128 = PHASE11_SERVE_CHALLENGE_BOND;

/// Provider FailedServe penalty (alias).
pub const PHASE12_FAILED_SERVE_PENALTY: u128 = PHASE11_FAILED_SERVE_PENALTY;

/// Appeal-window hint for FailedServe SlashRegistry records.
pub const PHASE12_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE11_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Domain for SMT settle-receipt registry leaf.
pub const PHASE12_SETTLE_RECEIPT_DOMAIN: &[u8] = b"BOING_BLOB_SETTLE_RECEIPT_V1";

/// Max retained settle receipts (same order as sample-offer market).
pub const PHASE12_MAX_SETTLE_RECEIPTS: usize = 256;

/// Market key for a settle receipt: `blob:chunk:provider:payer`.
#[must_use]
pub fn settle_receipt_key(
    blob_id: &BlobId,
    chunk_index: u32,
    provider: &AccountId,
    payer: &AccountId,
) -> String {
    format!(
        "{}:{}:{}:{}",
        hex::encode(blob_id),
        chunk_index,
        hex::encode(provider.0),
        hex::encode(payer.0)
    )
}

/// Challenge requires a prior settle receipt for `(blob, chunk, provider)` paid by `challenger`.
pub fn validate_challenge_requires_settle(has_settle_receipt: bool) -> Result<(), BlobError> {
    if !has_settle_receipt {
        return Err(BlobError::SampleOfferNotFound);
    }
    Ok(())
}

/// Operator-facing plan strings (Phase 12 audit notes; ReservedRealV1 still reject-until-wired).
pub fn phase12_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase11_das_succinct_proof_plan();
    v.extend([
        "Phase 12: FailedServe assigns consensus slash_id and mirrors SlashRegistry (BlobFailedServe); balance burn remains the economic slash.",
        "Phase 12: Challenge requires prior SettleBlobSampleMatch receipt (payer = challenger).",
    ]);
    v
}

/// Extended activation checklist (Phase 11 + Phase 12 ops).
pub fn phase12_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase11_cap_activation_checklist(activation_height);
    v.extend([
        "Admission activation (Put/Announce): set identical BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT on every validator (Fly: both boing-testnet-1 and boing-testnet-2) then restart/redeploy.",
        "Choose H = tip+N (e.g. tip+50) so all nodes restart before tip reaches H; mismatched heights split admission.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset on public testnet (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
        "After FailedServe, boing_listSlashRecords should show reason blobFailedServe joined via challenge slashId.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settle_receipt_key_stable() {
        let k = settle_receipt_key(&[1u8; 32], 0, &AccountId([2u8; 32]), &AccountId([3u8; 32]));
        assert!(k.contains(':'));
        assert_eq!(
            k,
            settle_receipt_key(&[1u8; 32], 0, &AccountId([2u8; 32]), &AccountId([3u8; 32]))
        );
    }

    #[test]
    fn challenge_requires_settle() {
        assert!(validate_challenge_requires_settle(true).is_ok());
        assert!(validate_challenge_requires_settle(false).is_err());
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase12_cap_activation_checklist(None).len() >= 12);
        assert!(phase12_das_succinct_proof_plan().len() >= 6);
    }
}
