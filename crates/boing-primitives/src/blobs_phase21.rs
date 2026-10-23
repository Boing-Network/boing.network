//! Phase 21: slash-appeal resolve expiry + stuck Pending cleanup.
//!
//! When **SubmitSlashAppeal (30)** succeeds, persist `resolve_deadline =
//! tip_height + PHASE21_RESOLVE_WINDOW_BLOCKS` (1000). **Resolve (31/32)** after
//! that height expires Pending → **Expired** (no vote) and rejects with
//! `ResolveWindowClosed`. Block housekeeping also expires overdue Pending
//! (including W=0 / incomplete tallies). No fund restore on Expired.
//!
//! Honest scope:
//! - Reuse payload indices **30** / **31** / **32** — no new tx.
//! - Registry **V5** (V4/V3/V2/legacy load); **do not clear** `slash_appeals_registry.bin`.
//! - Pre-V5 Pending: derive `resolve_deadline` from `slash.appeal_deadline`.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Phase 19 multi-pin → require **32** unchanged; Phase 20 frozen W unchanged.
//! - True BLS, SNARK/IPA/KZG, live 256 MiB — still later (22+).

use crate::blobs_phase20::{
    phase20_cap_activation_checklist, phase20_committee_requires_attested,
    phase20_das_succinct_proof_plan, phase20_is_legacy_electorate_snapshot,
    phase20_pin_set_commitment, phase20_resolve_threshold_met, phase20_snapshot_electorate_w,
    PHASE20_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE20_CAP_ACTIVATION_ENV,
    PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE20_LEGACY_ELECTORATE_W_SENTINEL,
    PHASE20_LEGACY_PIN_SET_COMMITMENT, PHASE20_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE20_MAX_SETTLE_RECEIPTS, PHASE20_PIN_SET_DOMAIN,
    PHASE20_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE20_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE20_RESOLVE_THRESHOLD_DEN, PHASE20_RESOLVE_THRESHOLD_NUM,
    PHASE20_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE20_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE20_SUBMIT_SLASH_APPEAL_TX_INDEX,
};
use crate::types::AccountId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE21_CAP_ACTIVATION_ENV: &str = PHASE20_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE21_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE20_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE21_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Resolve window after Submit: tip + this many blocks (alias of FailedServe appeal window).
pub const PHASE21_RESOLVE_WINDOW_BLOCKS: u64 = PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE21_MAX_SETTLE_RECEIPTS: usize = PHASE20_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE21_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE20_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE21_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE20_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE21_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE20_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE21_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE20_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`] (unattested).
pub const PHASE21_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE20_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppealAttested`].
pub const PHASE21_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE20_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;

/// Numerator of the fixed stake-weighted resolve quorum (⌈2W/3⌉).
pub const PHASE21_RESOLVE_THRESHOLD_NUM: u64 = PHASE20_RESOLVE_THRESHOLD_NUM;

/// Denominator of the fixed stake-weighted resolve quorum.
pub const PHASE21_RESOLVE_THRESHOLD_DEN: u64 = PHASE20_RESOLVE_THRESHOLD_DEN;

/// Domain tag for pin-set commitment at Submit (Phase 20 alias).
pub const PHASE21_PIN_SET_DOMAIN: &[u8] = PHASE20_PIN_SET_DOMAIN;

/// Sentinel: missing snapshot on migrated pre-V4 Pending appeals (live-W fallback).
pub const PHASE21_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE20_LEGACY_ELECTORATE_W_SENTINEL;

/// Zero pin commitment paired with legacy W sentinel ⇒ legacy row.
pub const PHASE21_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE20_LEGACY_PIN_SET_COMMITMENT;

/// `resolve_deadline` from Submit tip height: `submit_height + PHASE21_RESOLVE_WINDOW_BLOCKS`.
pub fn phase21_resolve_deadline(submit_height: u64) -> u64 {
    submit_height.saturating_add(PHASE21_RESOLVE_WINDOW_BLOCKS)
}

/// True when tip is strictly past the resolve deadline (Resolve must expire / reject).
pub fn phase21_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    tip_height > resolve_deadline
}

/// Snapshot electorate W (Phase 20 alias).
pub fn phase21_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase20_snapshot_electorate_w(pinned_stakes)
}

/// Pin-set commitment (Phase 20 alias).
pub fn phase21_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase20_pin_set_commitment(pinned)
}

/// Legacy electorate snapshot detection (Phase 20 alias).
pub fn phase21_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase20_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

/// True when a multi-pin committee must use attested resolve (**32**) (Phase 19 alias).
pub fn phase21_committee_requires_attested(pinned_len: usize) -> bool {
    phase20_committee_requires_attested(pinned_len)
}

/// True when `weight` meets ⌈2W/3⌉ (alias).
pub fn phase21_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase20_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 21 resolve expiry + Pending cleanup).
pub fn phase21_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase20_das_succinct_proof_plan();
    v.extend([
        "Phase 21: SubmitSlashAppeal (30) stores resolve_deadline = tip + 1000; Resolve (31/32) after deadline expires Pending → Expired (no vote) and rejects ResolveWindowClosed.",
        "Phase 21: block housekeeping expire_pending_appeals cleans stuck Pending (W=0 / incomplete ⌈2W/3⌉); Expired never restores balance/stake (economically like Rejected).",
        "Phase 21: slash_appeals_registry.bin is V5; V4/V3/V2/legacy load migrates without required clear — pre-V5 Pending derives resolve_deadline from slash.appeal_deadline.",
        "Phase 21: Phase 19 multi-pin → 32 and Phase 20 frozen W unchanged; true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 20 + Phase 21 ops). Do not rewrite live admission height.
pub fn phase21_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase20_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 21: do not clear slash_appeals_registry.bin — save V5; load V5 or migrate V4/V3/V2/legacy in memory.",
        "Phase 21: prefer Resolve before resolve_deadline (submit height + 1000); W=0 / idle committees Expired instead of hanging forever.",
        "Phase 21: mid-upgrade pre-V5 Pending derives resolve_deadline from slash.appeal_deadline (may be shorter than submit+1000); new Submits always write tip+1000.",
        "Phase 21: true BLS / SNARK / 256 MiB still later — this phase closes stuck Pending via resolve expiry only.",
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
        assert_eq!(PHASE21_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE21_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE21_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE21_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(PHASE21_RESOLVE_THRESHOLD_NUM, 2);
        assert_eq!(PHASE21_RESOLVE_THRESHOLD_DEN, 3);
        assert_eq!(PHASE21_RESOLVE_WINDOW_BLOCKS, 1000);
        assert_eq!(
            PHASE21_RESOLVE_WINDOW_BLOCKS,
            PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS
        );
        assert_eq!(
            PHASE21_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE20_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
    }

    #[test]
    fn resolve_deadline_and_expired() {
        assert_eq!(phase21_resolve_deadline(100), 1100);
        assert_eq!(phase21_resolve_deadline(u64::MAX), u64::MAX);
        assert!(!phase21_resolve_expired(1100, 1100));
        assert!(!phase21_resolve_expired(1099, 1100));
        assert!(phase21_resolve_expired(1101, 1100));
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase21_cap_activation_checklist(None).len() >= 45);
        let plan = phase21_das_succinct_proof_plan();
        assert!(plan.len() >= 45);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan.iter().any(|s| s.contains("resolve_deadline")
            || s.contains("ResolveWindowClosed")
            || s.contains("Expired")));
        assert!(plan
            .iter()
            .any(|s| s.contains("V5")
                && (s.contains("do not clear")
                    || s.contains("without required clear")
                    || s.contains("migrates"))));
        assert!(plan.iter().any(|s| s.contains("still later") || s.contains("BLS")));
        assert!(phase21_committee_requires_attested(3));
        assert!(!phase21_committee_requires_attested(1));
    }
}
