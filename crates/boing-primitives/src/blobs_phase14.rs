//! Phase 14: consensus slash-appeal txs (`SubmitSlashAppeal` / `ResolveSlashAppeal`),
//! evidence cap, activation notes (Fly-safe 16 MiB default; admission height unchanged).
//!
//! Honest scope:
//! - Submit/Resolve are consensus txs (indices 30/31); FailedServe unburn replays with the block.
//! - Resolve is **ops/validator-approved**, not IPA/KZG/SNARK verification of evidence.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.

use crate::blobs_phase13::{
    phase13_cap_activation_checklist, phase13_das_succinct_proof_plan,
    PHASE13_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE13_CAP_ACTIVATION_ENV,
    PHASE13_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE13_MAX_SETTLE_RECEIPTS,
    PHASE13_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE13_SETTLE_RECEIPT_TTL_BLOCKS,
};

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE14_CAP_ACTIVATION_ENV: &str = PHASE13_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE14_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE13_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE13_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE14_MAX_SETTLE_RECEIPTS: usize = PHASE13_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE14_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE13_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE14_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE13_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE14_MAX_APPEAL_EVIDENCE_BYTES: usize = 4096;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE14_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = 30;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`].
pub const PHASE14_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = 31;

/// Operator-facing plan strings (Phase 14 consensus appeals; ReservedRealV1 still reject-until-wired).
pub fn phase14_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase13_das_succinct_proof_plan();
    v.extend([
        "Phase 14: SubmitSlashAppeal (30) / ResolveSlashAppeal (31) are consensus txs; FailedServe unburn replays with the block.",
        "Phase 14: ResolveSlashAppeal is ops/validator-approved — not SNARK/IPA/KZG verification of appeal evidence.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 13 + Phase 14 ops). Do not rewrite live admission height.
pub fn phase14_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase13_cap_activation_checklist(activation_height);
    v.extend([
        "Prefer SubmitSlashAppeal / ResolveSlashAppeal txs (30/31) for FailedServe unburn on multi-validator nets; operator RPC is a thin secondary path.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_cap_and_aliases() {
        assert_eq!(PHASE14_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE14_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE14_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(
            PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
            PHASE13_FAILED_SERVE_APPEAL_WINDOW_BLOCKS
        );
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase14_cap_activation_checklist(None).len() >= 16);
        let plan = phase14_das_succinct_proof_plan();
        assert!(plan.len() >= 10);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan.iter().any(|s| s.contains("ops/validator-approved")));
    }
}
