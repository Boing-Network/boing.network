//! Phase 25: optional prune of ancient Approved/Rejected slash appeals + list filters.
//!
//! Phase 23 pruned **Expired** after `resolve_deadline + 4096` and left Approved/Rejected
//! for audit. Bound those terminal rows the same way once
//! `tip > resolve_deadline + PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS` in media-blob
//! block housekeeping. Extend `boing_listSlashRecords` /
//! `boing_reverifySlashAppealBallots` with `includeResolved`.
//!
//! Honest scope:
//! - Reuse payload indices **30** / **31** / **32** — no new tx.
//! - Registry stays **V5** (row deletion only; no schema change); **do not clear**
//!   `slash_appeals_registry.bin`.
//! - Do **not** prune Pending; do **not** delete slash records.
//! - Expired prune (Phase 23) and serve-challenge prune (Phase 24) unchanged.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Phase 19–24 policies unchanged.
//! - True BLS, SNARK/IPA/KZG, live 256 MiB — still later.

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase24::{
    phase24_cap_activation_checklist, phase24_committee_requires_attested,
    phase24_das_succinct_proof_plan, phase24_expired_appeal_prunable,
    phase24_is_legacy_electorate_snapshot, phase24_pin_set_commitment, phase24_resolve_deadline,
    phase24_resolve_expired, phase24_resolve_threshold_met, phase24_snapshot_electorate_w,
    phase24_terminal_serve_challenge_prunable, PHASE24_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
    PHASE24_CAP_ACTIVATION_ENV, PHASE24_EXPIRED_APPEAL_RETENTION_BLOCKS,
    PHASE24_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE24_LEGACY_ELECTORATE_W_SENTINEL,
    PHASE24_LEGACY_PIN_SET_COMMITMENT, PHASE24_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE24_MAX_SETTLE_RECEIPTS, PHASE24_PIN_SET_DOMAIN,
    PHASE24_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE24_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE24_RESOLVE_THRESHOLD_DEN, PHASE24_RESOLVE_THRESHOLD_NUM, PHASE24_RESOLVE_WINDOW_BLOCKS,
    PHASE24_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE24_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE24_SUBMIT_SLASH_APPEAL_TX_INDEX, PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::types::AccountId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE25_CAP_ACTIVATION_ENV: &str = PHASE24_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE25_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE24_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE25_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE24_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Resolve window after Submit (alias).
pub const PHASE25_RESOLVE_WINDOW_BLOCKS: u64 = PHASE24_RESOLVE_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE25_MAX_SETTLE_RECEIPTS: usize = PHASE24_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE25_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE24_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE25_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE24_SETTLE_RECEIPT_TTL_BLOCKS;

/// Expired-appeal retention alias (Phase 23).
pub const PHASE25_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE24_EXPIRED_APPEAL_RETENTION_BLOCKS;

/// Terminal serve-challenge retention alias (Phase 24).
pub const PHASE25_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;

/// Retention for **Approved** / **Rejected** slash appeals before housekeeping prune.
///
/// Eligible when status is Approved or Rejected and
/// `tip_height > resolve_deadline + PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS`.
/// Same order as Expired / settle-receipt / serve-challenge TTL.
pub const PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = 4096;

/// Max evidence bytes on SubmitSlashAppeal (alias).
pub const PHASE25_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE24_MAX_APPEAL_EVIDENCE_BYTES;

/// Payload index aliases.
pub const PHASE25_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE24_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE25_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE24_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE25_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE24_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;

pub const PHASE25_RESOLVE_THRESHOLD_NUM: u64 = PHASE24_RESOLVE_THRESHOLD_NUM;
pub const PHASE25_RESOLVE_THRESHOLD_DEN: u64 = PHASE24_RESOLVE_THRESHOLD_DEN;
pub const PHASE25_PIN_SET_DOMAIN: &[u8] = PHASE24_PIN_SET_DOMAIN;
pub const PHASE25_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE24_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE25_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE24_LEGACY_PIN_SET_COMMITMENT;

/// `resolve_deadline` from Submit tip height (alias).
pub fn phase25_resolve_deadline(submit_height: u64) -> u64 {
    phase24_resolve_deadline(submit_height)
}

/// True when tip is strictly past the resolve deadline (alias).
pub fn phase25_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase24_resolve_expired(tip_height, resolve_deadline)
}

/// Expired-appeal prunable helper (Phase 23 alias).
pub fn phase25_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase24_expired_appeal_prunable(tip_height, resolve_deadline)
}

/// True when an **Approved** / **Rejected** appeal is past retention and may be pruned.
///
/// `tip_height > resolve_deadline + PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS`.
/// Callers must also require status ∈ {Approved, Rejected}; Pending never uses this.
pub fn phase25_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    tip_height > resolve_deadline.saturating_add(PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS)
}

/// Terminal serve-challenge prunable helper (Phase 24 alias).
pub fn phase25_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase24_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase25_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase24_snapshot_electorate_w(pinned_stakes)
}

pub fn phase25_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase24_pin_set_commitment(pinned)
}

pub fn phase25_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase24_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase25_committee_requires_attested(pinned_len: usize) -> bool {
    phase24_committee_requires_attested(pinned_len)
}

pub fn phase25_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase24_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 25 ancient Approved/Rejected prune + list filters).
pub fn phase25_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase24_das_succinct_proof_plan();
    v.extend([
        "Phase 25: block housekeeping prune_resolved_appeals drops Approved / Rejected rows when tip > resolve_deadline + 4096 (mirrors Expired TTL); slash records stay; Pending untouched.",
        "Phase 25: boing_listSlashRecords gains includeResolved (default true for back-compat); boing_reverifySlashAppealBallots gains includeResolved (default false).",
        "Phase 25: registry stays V5 (row deletion only, no schema change, no clear); txs 30/31/32 unchanged; Phase 19–24 policies unchanged.",
        "Phase 25: true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 24 + Phase 25 ops).
pub fn phase25_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase24_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 25: do not clear slash_appeals_registry.bin — stay V5 save/load (no V6); prune shrinks in-memory book then next save.",
        "Phase 25: Approved/Rejected appeals remain until resolve_deadline + 4096; then drop on housekeeping — roll validators together so prune matches.",
        "Phase 25: pass includeResolved:false on list/re-verify when auditing live Pending (and optionally Expired) only; re-verify defaults to skipping resolved.",
        "Phase 25: true BLS / SNARK / 256 MiB still later — this phase bounds ancient Approved/Rejected retention + list filters only.",
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
        assert_eq!(PHASE25_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE25_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE25_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS, 4096);
        assert_eq!(
            PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS,
            PHASE25_EXPIRED_APPEAL_RETENTION_BLOCKS
        );
        assert_eq!(
            PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS,
            PHASE25_SETTLE_RECEIPT_TTL_BLOCKS
        );
        assert_eq!(
            PHASE25_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
            PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS
        );
    }

    #[test]
    fn resolved_appeal_prunable_rules() {
        let d = 1100u64;
        let retain = PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS;
        assert!(!phase25_resolved_appeal_prunable(d + retain, d));
        assert!(phase25_resolved_appeal_prunable(d + retain + 1, d));
        assert!(!phase25_resolved_appeal_prunable(d, d));
    }

    #[test]
    fn plan_mentions_resolved_prune() {
        let plan = phase25_das_succinct_proof_plan();
        assert!(plan
            .iter()
            .any(|s| s.contains("prune_resolved") || s.contains("Approved")));
        assert!(plan.iter().any(|s| s.contains("includeResolved")));
        let checklist = phase25_cap_activation_checklist(Some(1200));
        assert!(checklist
            .iter()
            .any(|s| s.contains("slash_appeals_registry.bin") && s.contains("V5")));
    }
}
