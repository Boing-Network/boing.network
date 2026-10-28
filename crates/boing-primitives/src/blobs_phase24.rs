//! Phase 24: serve-challenge registry TTL prune + list/get UX.
//!
//! Soft-cap **256** fill with terminal Responded / FailedServe rows. Prune those
//! once `bond_settled` and `tip > deadline_height + PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS`
//! in media-blob block housekeeping. Extend list filters and add get-by-key RPC.
//!
//! Honest scope:
//! - No new tx index (28/29 reuse).
//! - Do **not** prune Open challenges; do **not** prune unsettled FailedServe.
//! - Slash-appeals registry stays **V5** (untouched); **do not clear**.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Phase 19–23 policies unchanged.
//! - True BLS, SNARK/IPA/KZG, live 256 MiB — still later (25+).

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase23::{
    phase23_cap_activation_checklist, phase23_committee_requires_attested,
    phase23_das_succinct_proof_plan, phase23_expired_appeal_prunable,
    phase23_is_legacy_electorate_snapshot, phase23_pin_set_commitment, phase23_resolve_deadline,
    phase23_resolve_expired, phase23_resolve_threshold_met, phase23_snapshot_electorate_w,
    PHASE23_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE23_CAP_ACTIVATION_ENV,
    PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS, PHASE23_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE23_LEGACY_ELECTORATE_W_SENTINEL, PHASE23_LEGACY_PIN_SET_COMMITMENT,
    PHASE23_MAX_APPEAL_EVIDENCE_BYTES, PHASE23_MAX_SETTLE_RECEIPTS, PHASE23_PIN_SET_DOMAIN,
    PHASE23_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE23_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE23_RESOLVE_THRESHOLD_DEN, PHASE23_RESOLVE_THRESHOLD_NUM, PHASE23_RESOLVE_WINDOW_BLOCKS,
    PHASE23_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE23_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE23_SUBMIT_SLASH_APPEAL_TX_INDEX,
};
use crate::types::AccountId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE24_CAP_ACTIVATION_ENV: &str = PHASE23_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE24_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE23_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE24_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE23_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Resolve window after Submit (alias).
pub const PHASE24_RESOLVE_WINDOW_BLOCKS: u64 = PHASE23_RESOLVE_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE24_MAX_SETTLE_RECEIPTS: usize = PHASE23_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE24_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE23_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE24_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE23_SETTLE_RECEIPT_TTL_BLOCKS;

/// Expired-appeal retention alias (Phase 23).
pub const PHASE24_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS;

/// Retention for **terminal** serve challenges (Responded / settled FailedServe)
/// before housekeeping prune. Same order as settle-receipt / Expired-appeal TTL.
pub const PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 = 4096;

/// Max evidence bytes on SubmitSlashAppeal (alias).
pub const PHASE24_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE23_MAX_APPEAL_EVIDENCE_BYTES;

/// Payload index aliases.
pub const PHASE24_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE23_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE24_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE23_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE24_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE23_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;

pub const PHASE24_RESOLVE_THRESHOLD_NUM: u64 = PHASE23_RESOLVE_THRESHOLD_NUM;
pub const PHASE24_RESOLVE_THRESHOLD_DEN: u64 = PHASE23_RESOLVE_THRESHOLD_DEN;
pub const PHASE24_PIN_SET_DOMAIN: &[u8] = PHASE23_PIN_SET_DOMAIN;
pub const PHASE24_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE23_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE24_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE23_LEGACY_PIN_SET_COMMITMENT;

/// `resolve_deadline` from Submit tip height (alias).
pub fn phase24_resolve_deadline(submit_height: u64) -> u64 {
    phase23_resolve_deadline(submit_height)
}

/// True when tip is strictly past the resolve deadline (alias).
pub fn phase24_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase23_resolve_expired(tip_height, resolve_deadline)
}

/// Expired-appeal prunable helper (Phase 23 alias).
pub fn phase24_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase23_expired_appeal_prunable(tip_height, resolve_deadline)
}

/// True when a serve challenge is terminal, settled, and past retention.
///
/// Eligibility:
/// - status is **Responded** or **FailedServe**
/// - `bond_settled == true` (economics finished; never drop unsettled FailedServe)
/// - `tip_height > deadline_height + PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS`
///
/// Open challenges are never prunable.
pub fn phase24_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    if !bond_settled {
        return false;
    }
    match status {
        BlobSampleServeChallengeStatus::Open => false,
        BlobSampleServeChallengeStatus::Responded | BlobSampleServeChallengeStatus::FailedServe => {
            tip_height
                > deadline_height.saturating_add(PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS)
        }
    }
}

pub fn phase24_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase23_snapshot_electorate_w(pinned_stakes)
}

pub fn phase24_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase23_pin_set_commitment(pinned)
}

pub fn phase24_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase23_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase24_committee_requires_attested(pinned_len: usize) -> bool {
    phase23_committee_requires_attested(pinned_len)
}

pub fn phase24_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase23_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 24 serve-challenge TTL + list/get UX).
pub fn phase24_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase23_das_succinct_proof_plan();
    v.extend([
        "Phase 24: block housekeeping prune_terminal_serve_challenges drops Responded / settled FailedServe when tip > deadline + 4096 (frees soft-cap 256); Open and unsettled FailedServe stay.",
        "Phase 24: boing_listBlobSampleServeChallenges gains status / includeTerminal filters; boing_getBlobSampleServeChallenge looks up by blobId+chunkIndex+provider+challenger.",
        "Phase 24: slash_appeals_registry.bin untouched (stay V5); do not clear; txs 28/29/30/31/32 unchanged; Phase 19–23 policies unchanged.",
        "Phase 24: true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 23 + Phase 24 ops).
pub fn phase24_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase23_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 24: do not clear slash_appeals_registry.bin — stay V5; serve-challenge prune is separate (sample_serve_challenges.bin).",
        "Phase 24: terminal serve challenges remain until deadline + 4096 after bond settle; roll validators together so prune matches.",
        "Phase 24: pass status / includeTerminal on list; use getBlobSampleServeChallenge for key lookup.",
        "Phase 24: true BLS / SNARK / 256 MiB still later — this phase bounds serve-challenge retention + list/get UX only.",
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
        assert_eq!(PHASE24_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE24_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE24_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS, 4096);
        assert_eq!(
            PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
            PHASE24_SETTLE_RECEIPT_TTL_BLOCKS
        );
        assert_eq!(
            PHASE24_EXPIRED_APPEAL_RETENTION_BLOCKS,
            PHASE23_EXPIRED_APPEAL_RETENTION_BLOCKS
        );
    }

    #[test]
    fn terminal_serve_challenge_prunable_rules() {
        let d = 100u64;
        let retain = PHASE24_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
        // Open never
        assert!(!phase24_terminal_serve_challenge_prunable(
            d + retain + 10,
            d,
            BlobSampleServeChallengeStatus::Open,
            true
        ));
        // Unsettled FailedServe never
        assert!(!phase24_terminal_serve_challenge_prunable(
            d + retain + 10,
            d,
            BlobSampleServeChallengeStatus::FailedServe,
            false
        ));
        // Settled FailedServe at boundary keep / past prune
        assert!(!phase24_terminal_serve_challenge_prunable(
            d + retain,
            d,
            BlobSampleServeChallengeStatus::FailedServe,
            true
        ));
        assert!(phase24_terminal_serve_challenge_prunable(
            d + retain + 1,
            d,
            BlobSampleServeChallengeStatus::FailedServe,
            true
        ));
        // Responded settled
        assert!(phase24_terminal_serve_challenge_prunable(
            d + retain + 1,
            d,
            BlobSampleServeChallengeStatus::Responded,
            true
        ));
        assert!(!phase24_terminal_serve_challenge_prunable(
            d + retain + 1,
            d,
            BlobSampleServeChallengeStatus::Responded,
            false
        ));
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase24_cap_activation_checklist(None).len() >= 60);
        let plan = phase24_das_succinct_proof_plan();
        assert!(plan.len() >= 60);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan
            .iter()
            .any(|s| s.contains("prune_terminal_serve") || s.contains("serve-challenge")));
        assert!(plan
            .iter()
            .any(|s| s.contains("getBlobSampleServeChallenge") || s.contains("includeTerminal")));
        assert!(plan
            .iter()
            .any(|s| s.contains("still later") || s.contains("BLS")));
        assert!(phase24_committee_requires_attested(3));
    }
}
