//! Phase 15: seed equivocation + liveness into the consensus slash-appeal book so
//! existing SubmitSlashAppeal / ResolveSlashAppeal txs (30/31) restore **stake** on
//! multi-validator replay (Phase 14 covered FailedServe balance unburn only).
//!
//! Honest scope:
//! - Reuse payload indices 30/31 — no new variants.
//! - Equivocation/liveness **detection** remains node-local (evidence gossip + apply);
//!   Phase 15 seeds the consensus book at slash-apply time so subsequent 30/31 txs
//!   replay identically. Validators that saw the same evidence apply the same slash
//!   (same social/ops model as today's local SlashRegistry mirror).
//! - Resolve is still **ops/validator-approved**, not IPA/KZG/SNARK verification.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Submit/Resolve (30/31) are **always admitted** once the chain is running (not gated
//!   on media-blobs activation), so equivocation/liveness appeals work when media is off.

use crate::blobs_phase14::{
    phase14_cap_activation_checklist, phase14_das_succinct_proof_plan,
    PHASE14_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE14_CAP_ACTIVATION_ENV,
    PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE14_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE14_MAX_SETTLE_RECEIPTS, PHASE14_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE14_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE14_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE14_SUBMIT_SLASH_APPEAL_TX_INDEX,
};

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE15_CAP_ACTIVATION_ENV: &str = PHASE14_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE15_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE14_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE15_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE15_MAX_SETTLE_RECEIPTS: usize = PHASE14_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE15_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE14_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE15_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE14_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE15_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE14_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE15_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE14_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`].
pub const PHASE15_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE14_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Operator-facing plan strings (Phase 15 consensus equivocation/liveness appeals).
pub fn phase15_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase14_das_succinct_proof_plan();
    v.extend([
        "Phase 15: equivocation/liveness slashes seed the consensus SlashAppealRegistry; Submit/Resolve (30/31) restore stake on multi-validator replay.",
        "Phase 15: slash detection/apply remains node-local (evidence gossip); appeal submit/resolve txs are consensus-replayable once a row exists.",
        "Phase 15: Submit/Resolve (30/31) are always admitted (not gated on media-blobs activation) so equivocation/liveness appeals work when media is inactive.",
        "Phase 15: ResolveSlashAppeal remains ops/validator-approved — not SNARK/IPA/KZG verification of appeal evidence.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 14 + Phase 15 ops). Do not rewrite live admission height.
pub fn phase15_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase14_cap_activation_checklist(activation_height);
    v.extend([
        "Prefer SubmitSlashAppeal / ResolveSlashAppeal txs (30/31) for equivocation/liveness stake restore and FailedServe balance unburn; operator RPC is secondary for all consensus-book reasons.",
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
        assert_eq!(PHASE15_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE15_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE15_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(
            PHASE15_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
            PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS
        );
        assert_eq!(
            PHASE15_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE14_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase15_cap_activation_checklist(None).len() >= 18);
        let plan = phase15_das_succinct_proof_plan();
        assert!(plan.len() >= 14);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan.iter().any(|s| s.contains("equivocation/liveness")));
        assert!(plan.iter().any(|s| s.contains("always admitted")));
        assert!(plan.iter().any(|s| s.contains("ops/validator-approved")));
    }
}
