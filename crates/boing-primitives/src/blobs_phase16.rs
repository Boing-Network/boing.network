//! Phase 16: committee threshold resolve for slash appeals.
//!
//! ResolveSlashAppeal (31) becomes an N-of-M vote cast over host-pinned
//! `slash_appeal_resolvers`. Finalize (restore or keep burn) only when
//! ⌈2M/3⌉ approve **or** reject is met (`votes * DEN >= NUM * M`).
//!
//! Honest scope:
//! - Reuse payload index 31 — no new variants.
//! - Eligible voters unchanged (pinned resolvers / fallback).
//! - When fallback eligibility without a pinned set, treat M=1 so one vote
//!   finalizes (solo-dev UX).
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Submit/Resolve (30/31) remain always admitted (Phase 15).

use crate::blobs_phase15::{
    phase15_cap_activation_checklist, phase15_das_succinct_proof_plan,
    PHASE15_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE15_CAP_ACTIVATION_ENV,
    PHASE15_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE15_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE15_MAX_SETTLE_RECEIPTS, PHASE15_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE15_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE15_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE15_SUBMIT_SLASH_APPEAL_TX_INDEX,
};

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE16_CAP_ACTIVATION_ENV: &str = PHASE15_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE16_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE15_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE16_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE15_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE16_MAX_SETTLE_RECEIPTS: usize = PHASE15_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE16_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE15_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE16_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE15_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE16_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE15_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE16_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE15_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`].
pub const PHASE16_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE15_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Numerator of the fixed resolve quorum (⌈2M/3⌉ via integer compare).
pub const PHASE16_RESOLVE_THRESHOLD_NUM: u64 = 2;

/// Denominator of the fixed resolve quorum.
pub const PHASE16_RESOLVE_THRESHOLD_DEN: u64 = 3;

/// True when `votes` meets ⌈2M/3⌉ of electorate `M` using integer
/// `votes * DEN >= NUM * M` (no f64).
///
/// `M == 0` never meets the threshold (empty electorate is ops-invalid).
pub fn phase16_resolve_threshold_met(votes: usize, electorate_m: usize) -> bool {
    if electorate_m == 0 {
        return false;
    }
    let votes = votes as u128;
    let m = electorate_m as u128;
    votes.saturating_mul(PHASE16_RESOLVE_THRESHOLD_DEN as u128)
        >= (PHASE16_RESOLVE_THRESHOLD_NUM as u128).saturating_mul(m)
}

/// Operator-facing plan strings (Phase 16 committee threshold resolve).
pub fn phase16_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase15_das_succinct_proof_plan();
    v.extend([
        "Phase 16: ResolveSlashAppeal (31) is an N-of-M committee vote over host-pinned slash_appeal_resolvers (⌈2M/3⌉ approve or reject).",
        "Phase 16: finalize (restore balance/stake or keep burn) only when the approve or reject threshold is met; one vote per voter; no flip.",
        "Phase 16: when fallback eligibility without a pinned resolver set, M=1 so one eligible vote finalizes (solo-dev UX).",
        "Phase 16: ResolveSlashAppeal remains ops/validator-approved voting — not SNARK/IPA/KZG verification of appeal evidence.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 15 + Phase 16 ops). Do not rewrite live admission height.
pub fn phase16_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase15_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 16: ResolveSlashAppeal (31) casts a vote; multi-validator nets need ⌈2M/3⌉ of pinned slash_appeal_resolvers before finalize/restore.",
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
        assert_eq!(PHASE16_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE16_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE16_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(PHASE16_RESOLVE_THRESHOLD_NUM, 2);
        assert_eq!(PHASE16_RESOLVE_THRESHOLD_DEN, 3);
        assert_eq!(
            PHASE16_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE15_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
    }

    #[test]
    fn threshold_m1_m3_m4() {
        // M=1: one vote meets ⌈2/3⌉
        assert!(!phase16_resolve_threshold_met(0, 1));
        assert!(phase16_resolve_threshold_met(1, 1));
        // M=3: need 2 (2*3 >= 2*3)
        assert!(!phase16_resolve_threshold_met(1, 3));
        assert!(phase16_resolve_threshold_met(2, 3));
        // M=4: need 3 (2*3=6 < 8; 3*3=9 >= 8)
        assert!(!phase16_resolve_threshold_met(2, 4));
        assert!(phase16_resolve_threshold_met(3, 4));
        assert!(!phase16_resolve_threshold_met(1, 0));
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase16_cap_activation_checklist(None).len() >= 20);
        let plan = phase16_das_succinct_proof_plan();
        assert!(plan.len() >= 18);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan.iter().any(|s| s.contains("N-of-M")));
        assert!(plan
            .iter()
            .any(|s| s.contains("⌈2M/3⌉") || s.contains("2M/3")));
        assert!(plan.iter().any(|s| s.contains("M=1")));
    }
}
