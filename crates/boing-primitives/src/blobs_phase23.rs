//! Phase 23: prune Expired slash appeals (retention TTL) + list/re-verify filters.
//!
//! After Phase 21 left **Expired** rows forever and Phase 22 added ballot re-verify
//! over the live book, prune Expired appeals once
//! `tip > resolve_deadline + PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS` in media-blob
//! block housekeeping (mirroring settle-receipt TTL). Extend
//! `boing_listSlashRecords` / `boing_reverifySlashAppealBallots` with Expired filters.
//!
//! Honest scope:
//! - Reuse payload indices **30** / **31** / **32** — no new tx.
//! - Registry stays **V5** (row deletion only; no schema change); **do not clear**
//!   `slash_appeals_registry.bin`.
//! - Do **not** prune Pending / Approved / Rejected; do **not** delete slash records.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Phase 19–22 policies unchanged (multi-pin→32, frozen W, resolve expiry, ballot V2).
//! - True BLS, SNARK/IPA/KZG, live 256 MiB, serve-challenge TTL — still later (24+).

use crate::blobs_phase22::{
    phase22_cap_activation_checklist, phase22_committee_requires_attested,
    phase22_das_succinct_proof_plan, phase22_is_legacy_electorate_snapshot,
    phase22_pin_set_commitment, phase22_resolve_deadline, phase22_resolve_expired,
    phase22_resolve_threshold_met, phase22_snapshot_electorate_w,
    PHASE22_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE22_CAP_ACTIVATION_ENV,
    PHASE22_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE22_LEGACY_ELECTORATE_W_SENTINEL,
    PHASE22_LEGACY_PIN_SET_COMMITMENT, PHASE22_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE22_MAX_SETTLE_RECEIPTS, PHASE22_PIN_SET_DOMAIN,
    PHASE22_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE22_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE22_RESOLVE_THRESHOLD_DEN, PHASE22_RESOLVE_THRESHOLD_NUM, PHASE22_RESOLVE_WINDOW_BLOCKS,
    PHASE22_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE22_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE22_SUBMIT_SLASH_APPEAL_TX_INDEX,
};
use crate::types::AccountId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE23_CAP_ACTIVATION_ENV: &str = PHASE22_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE23_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE22_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE23_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE22_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Resolve window after Submit (Phase 21/22 alias).
pub const PHASE23_RESOLVE_WINDOW_BLOCKS: u64 = PHASE22_RESOLVE_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE23_MAX_SETTLE_RECEIPTS: usize = PHASE22_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE23_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE22_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias (same order as Expired-appeal retention).
pub const PHASE23_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE22_SETTLE_RECEIPT_TTL_BLOCKS;

/// Retention for **Expired** slash appeals before housekeeping prune.
///
/// Eligible when `status == Expired` and
/// `tip_height > resolve_deadline + PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS`.
pub const PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = 4096;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE23_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE22_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE23_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE22_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`] (unattested).
pub const PHASE23_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE22_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppealAttested`].
pub const PHASE23_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE22_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;

/// Numerator of the fixed stake-weighted resolve quorum (⌈2W/3⌉).
pub const PHASE23_RESOLVE_THRESHOLD_NUM: u64 = PHASE22_RESOLVE_THRESHOLD_NUM;

/// Denominator of the fixed stake-weighted resolve quorum.
pub const PHASE23_RESOLVE_THRESHOLD_DEN: u64 = PHASE22_RESOLVE_THRESHOLD_DEN;

/// Domain tag for pin-set commitment at Submit (Phase 20–22 alias).
pub const PHASE23_PIN_SET_DOMAIN: &[u8] = PHASE22_PIN_SET_DOMAIN;

/// Sentinel: missing snapshot on migrated pre-V4 Pending appeals (live-W fallback).
pub const PHASE23_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE22_LEGACY_ELECTORATE_W_SENTINEL;

/// Zero pin commitment paired with legacy W sentinel ⇒ legacy row.
pub const PHASE23_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE22_LEGACY_PIN_SET_COMMITMENT;

/// `resolve_deadline` from Submit tip height (Phase 21 alias).
pub fn phase23_resolve_deadline(submit_height: u64) -> u64 {
    phase22_resolve_deadline(submit_height)
}

/// True when tip is strictly past the resolve deadline (Phase 21 alias).
pub fn phase23_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase22_resolve_expired(tip_height, resolve_deadline)
}

/// True when an **Expired** appeal is past retention and may be pruned.
///
/// `tip_height > resolve_deadline + PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS`.
pub fn phase23_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    tip_height
        > resolve_deadline.saturating_add(PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS)
}

/// Snapshot electorate W (Phase 20–22 alias).
pub fn phase23_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase22_snapshot_electorate_w(pinned_stakes)
}

/// Pin-set commitment (Phase 20–22 alias).
pub fn phase23_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase22_pin_set_commitment(pinned)
}

/// Legacy electorate snapshot detection (Phase 20–22 alias).
pub fn phase23_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase22_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

/// True when a multi-pin committee must use attested resolve (**32**) (Phase 19 alias).
pub fn phase23_committee_requires_attested(pinned_len: usize) -> bool {
    phase22_committee_requires_attested(pinned_len)
}

/// True when `weight` meets ⌈2W/3⌉ (alias).
pub fn phase23_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase22_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 23 Expired retention prune + list filters).
pub fn phase23_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase22_das_succinct_proof_plan();
    v.extend([
        "Phase 23: block housekeeping prune_expired_appeals drops Expired rows when tip > resolve_deadline + 4096 (mirrors settle-receipt TTL); slash records stay; no fund restore.",
        "Phase 23: do not prune Pending / Approved / Rejected — keep finalize + ballot audit trail for Phase 22 re-verify.",
        "Phase 23: boing_listSlashRecords gains appealStatus / includeExpired (default true for back-compat); boing_reverifySlashAppealBallots gains includeExpired (default false).",
        "Phase 23: registry stays V5 (row deletion only, no schema change, no clear); txs 30/31/32 unchanged; Phase 19–22 policies unchanged.",
        "Phase 23: true BLS aggregate, SNARK/IPA/KZG, serve-challenge TTL, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 22 + Phase 23 ops). Do not rewrite live admission height.
pub fn phase23_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase22_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 23: do not clear slash_appeals_registry.bin — stay V5 save/load (no V6); prune shrinks in-memory book then next save.",
        "Phase 23: Expired appeals remain until resolve_deadline + 4096; then drop on housekeeping — roll validators together so prune matches.",
        "Phase 23: pass includeExpired:false on list/re-verify when auditing live Pending/Approved/Rejected only; re-verify defaults to skipping Expired.",
        "Phase 23: true BLS / SNARK / serve-challenge TTL / 256 MiB still later — this phase bounds Expired retention + list filters only.",
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
        assert_eq!(PHASE23_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE23_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE23_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE23_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(PHASE23_RESOLVE_THRESHOLD_NUM, 2);
        assert_eq!(PHASE23_RESOLVE_THRESHOLD_DEN, 3);
        assert_eq!(PHASE23_RESOLVE_WINDOW_BLOCKS, 1000);
        assert_eq!(PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS, 4096);
        assert_eq!(
            PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS,
            PHASE23_SETTLE_RECEIPT_TTL_BLOCKS
        );
        assert_eq!(
            PHASE23_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE22_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
    }

    #[test]
    fn expired_appeal_prunable_boundary() {
        // resolve_deadline = 1100 → prunable after tip > 1100 + 4096 = 5196
        assert!(!phase23_expired_appeal_prunable(5196, 1100));
        assert!(!phase23_expired_appeal_prunable(5195, 1100));
        assert!(phase23_expired_appeal_prunable(5197, 1100));
        assert!(!phase23_expired_appeal_prunable(0, 0));
        assert!(phase23_expired_appeal_prunable(
            PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS + 1,
            0
        ));
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase23_cap_activation_checklist(None).len() >= 55);
        let plan = phase23_das_succinct_proof_plan();
        assert!(plan.len() >= 55);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan
            .iter()
            .any(|s| s.contains("prune_expired") || s.contains("4096")));
        assert!(plan.iter().any(|s| s.contains("includeExpired")));
        assert!(plan
            .iter()
            .any(|s| s.contains("V5")
                && (s.contains("no clear")
                    || s.contains("no schema")
                    || s.contains("stays V5")
                    || s.contains("do not clear"))));
        assert!(plan.iter().any(|s| s.contains("still later") || s.contains("BLS")));
        assert!(phase23_committee_requires_attested(3));
        assert!(!phase23_committee_requires_attested(1));
    }
}
