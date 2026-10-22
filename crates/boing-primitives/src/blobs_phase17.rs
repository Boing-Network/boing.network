//! Phase 17: stake-weighted threshold resolve for slash appeals.
//!
//! ResolveSlashAppeal (31) weights each vote by the voter's bonded stake at cast.
//! Finalize (restore or keep burn) only when ⌈2W/3⌉ approve **or** reject is met
//! among host-pinned `slash_appeal_resolvers` (`weight * DEN >= NUM * W`).
//!
//! Honest scope:
//! - Reuse payload index 31 — no new variants; stake is read from state (not payload).
//! - Eligible voters unchanged (pinned resolvers / fallback).
//! - When fallback eligibility without a pinned set, treat effective W=1 so one vote
//!   finalizes (solo-dev UX).
//! - W=0 with a non-empty pinned set never finalizes (ops-invalid until stakes bond).
//! - Versioned `slash_appeals_registry.bin` (magic + u32 + bincode); legacy Phase 14–16
//!   load without required clear.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Submit/Resolve (30/31) remain always admitted (Phase 15).

use crate::blobs_phase16::{
    phase16_cap_activation_checklist, phase16_das_succinct_proof_plan,
    PHASE16_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE16_CAP_ACTIVATION_ENV,
    PHASE16_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE16_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE16_MAX_SETTLE_RECEIPTS, PHASE16_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE16_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE16_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE16_SUBMIT_SLASH_APPEAL_TX_INDEX,
};

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE17_CAP_ACTIVATION_ENV: &str = PHASE16_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE17_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE16_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE17_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE16_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE17_MAX_SETTLE_RECEIPTS: usize = PHASE16_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE17_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE16_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE17_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE16_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE17_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE16_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE17_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE16_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`].
pub const PHASE17_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE16_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Numerator of the fixed stake-weighted resolve quorum (⌈2W/3⌉ via integer compare).
pub const PHASE17_RESOLVE_THRESHOLD_NUM: u64 = 2;

/// Denominator of the fixed stake-weighted resolve quorum.
pub const PHASE17_RESOLVE_THRESHOLD_DEN: u64 = 3;

/// True when `weight` meets ⌈2W/3⌉ of electorate total weight `W` using integer
/// `weight * DEN >= NUM * W` (no f64).
///
/// `W == 0` never meets the threshold (empty or zero-stake electorate is ops-invalid;
/// stay Pending — do not auto-finalize).
pub fn phase17_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    if electorate_w == 0 {
        return false;
    }
    weight.saturating_mul(PHASE17_RESOLVE_THRESHOLD_DEN as u128)
        >= (PHASE17_RESOLVE_THRESHOLD_NUM as u128).saturating_mul(electorate_w)
}

/// Operator-facing plan strings (Phase 17 stake-weighted resolve + versioned registry).
pub fn phase17_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase16_das_succinct_proof_plan();
    v.extend([
        "Phase 17: ResolveSlashAppeal (31) is stake-weighted ⌈2W/3⌉ over host-pinned slash_appeal_resolvers (weight = bonded stake at cast).",
        "Phase 17: finalize (restore balance/stake or keep burn) only when approve or reject weight meets the threshold; one vote per voter; no flip.",
        "Phase 17: when fallback eligibility without a pinned resolver set, effective W=1 so one eligible vote finalizes (solo-dev UX).",
        "Phase 17: W=0 with a non-empty pinned set never finalizes — pin bonded resolvers or rebond.",
        "Phase 17: slash_appeals_registry.bin is versioned (magic+u32+bincode); legacy Phase 14–16 loads migrate without required clear.",
        "Phase 17: ResolveSlashAppeal remains ops/validator-approved voting — not SNARK/IPA/KZG verification of appeal evidence.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 16 + Phase 17 ops). Do not rewrite live admission height.
pub fn phase17_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase16_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 17: ResolveSlashAppeal (31) is stake-weighted; multi-validator nets need ⌈2W/3⌉ of bonded stake among pinned slash_appeal_resolvers before finalize/restore.",
        "Phase 17: do not clear slash_appeals_registry.bin on upgrade — node loads V2 or migrates legacy bare bincode in memory and rewrites V2 on next save.",
        "Phase 17: mid-tally migrated Phase 16 bool votes get weight=0; re-cast from bonded resolvers if an appeal was mid-vote at upgrade.",
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
        assert_eq!(PHASE17_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE17_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE17_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(PHASE17_RESOLVE_THRESHOLD_NUM, 2);
        assert_eq!(PHASE17_RESOLVE_THRESHOLD_DEN, 3);
        assert_eq!(
            PHASE17_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE16_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
    }

    #[test]
    fn threshold_equal_stakes_and_whale() {
        // W=1: one unit meets ⌈2/3⌉
        assert!(!phase17_resolve_threshold_met(0, 1));
        assert!(phase17_resolve_threshold_met(1, 1));
        // Equal stakes W=300 (3×100): need 200 (200*3 >= 2*300)
        assert!(!phase17_resolve_threshold_met(100, 300));
        assert!(phase17_resolve_threshold_met(200, 300));
        // Whale W=1000 (900+50+50): 900 alone meets
        assert!(phase17_resolve_threshold_met(900, 1000));
        assert!(!phase17_resolve_threshold_met(50, 1000));
        // W=0 never meets
        assert!(!phase17_resolve_threshold_met(0, 0));
        assert!(!phase17_resolve_threshold_met(1, 0));
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase17_cap_activation_checklist(None).len() >= 24);
        let plan = phase17_das_succinct_proof_plan();
        assert!(plan.len() >= 22);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan.iter().any(|s| s.contains("stake-weighted") || s.contains("⌈2W/3⌉")));
        assert!(plan.iter().any(|s| s.contains("versioned") || s.contains("slash_appeals_registry")));
        assert!(plan.iter().any(|s| s.contains("W=0")));
    }
}
