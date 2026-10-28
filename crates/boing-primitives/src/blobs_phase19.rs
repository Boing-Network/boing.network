//! Phase 19: require attested resolve for multi-pin slash-appeal committees.
//!
//! When host-pinned `|slash_appeal_resolvers| > 1`, resolve casts must use
//! **ResolveSlashAppealAttested (32)** with a valid Phase 18 Ed25519 ballot.
//! Unattested **ResolveSlashAppeal (31)** is rejected in that configuration.
//! Solo / unpinned / `|pinned| ≤ 1` keep **31** for UX and historical replay.
//!
//! Honest scope:
//! - Reuse payload indices **30** / **31** / **32** — no new tx.
//! - Threshold / weight / registry V3 unchanged; do not clear registry.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Do not strip weight from historical unattested votes mid-tally.
//! - True BLS aggregate of resolve ballots is still later (20+).

use crate::blobs_phase18::{
    phase18_cap_activation_checklist, phase18_das_succinct_proof_plan,
    phase18_resolve_threshold_met, PHASE18_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
    PHASE18_CAP_ACTIVATION_ENV, PHASE18_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE18_MAX_APPEAL_EVIDENCE_BYTES, PHASE18_MAX_SETTLE_RECEIPTS,
    PHASE18_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE18_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE18_RESOLVE_THRESHOLD_DEN, PHASE18_RESOLVE_THRESHOLD_NUM,
    PHASE18_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE18_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE18_SUBMIT_SLASH_APPEAL_TX_INDEX,
};

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE19_CAP_ACTIVATION_ENV: &str = PHASE18_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE19_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE18_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE19_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE18_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE19_MAX_SETTLE_RECEIPTS: usize = PHASE18_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE19_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE18_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE19_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE18_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE19_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE18_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE19_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE18_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`] (unattested).
pub const PHASE19_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE18_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppealAttested`].
pub const PHASE19_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE18_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;

/// Numerator of the fixed stake-weighted resolve quorum (⌈2W/3⌉).
pub const PHASE19_RESOLVE_THRESHOLD_NUM: u64 = PHASE18_RESOLVE_THRESHOLD_NUM;

/// Denominator of the fixed stake-weighted resolve quorum.
pub const PHASE19_RESOLVE_THRESHOLD_DEN: u64 = PHASE18_RESOLVE_THRESHOLD_DEN;

/// True when a multi-pin committee must use attested resolve (**32**).
///
/// `pinned_len` is `|slash_appeal_resolvers|` when the host pin set is present.
/// Solo (`1`) and empty (`0`) keep unattested **31**.
pub fn phase19_committee_requires_attested(pinned_len: usize) -> bool {
    pinned_len > 1
}

/// True when `weight` meets ⌈2W/3⌉ (alias of Phase 17/18 integer compare).
pub fn phase19_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase18_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 19 committee attested-required policy).
pub fn phase19_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase18_das_succinct_proof_plan();
    v.extend([
        "Phase 19: when |pinned slash_appeal_resolvers| > 1, ResolveSlashAppealAttested (32) is required; unattested ResolveSlashAppeal (31) is rejected.",
        "Phase 19: solo / unpinned / |pinned| ≤ 1 keep 31 for UX and historical replay; stake-weighted ⌈2W/3⌉ finalize math unchanged.",
        "Phase 19: mid-tally historical votes with ballot_signature=None keep their weight; new multi-pin casts must use 32.",
        "Phase 19: slash_appeals_registry.bin remains V3 (V2/legacy load); do not clear on upgrade — policy is execution-time only.",
        "Phase 19: true BLS aggregate of resolve ballots is still later — do not invent BLS crates or fake-BLS naming.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 18 + Phase 19 ops). Do not rewrite live admission height.
pub fn phase19_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase18_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 19: committee nets (|pinned slash_appeal_resolvers| > 1) must use ResolveSlashAppealAttested (32); reject unattested 31 in that config.",
        "Phase 19: solo / unpinned / |pinned| ≤ 1 may still cast unattested 31; prefer 32 when attest keys are available.",
        "Phase 19: do not clear slash_appeals_registry.bin on upgrade — policy is execution-time; historical unattested weight is retained mid-tally.",
        "Phase 19: BLS aggregate resolve is still later — this phase hardens attested-required policy only (Ed25519 verify-each).",
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
        assert_eq!(PHASE19_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE19_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE19_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE19_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(PHASE19_RESOLVE_THRESHOLD_NUM, 2);
        assert_eq!(PHASE19_RESOLVE_THRESHOLD_DEN, 3);
        assert_eq!(
            PHASE19_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE18_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
    }

    #[test]
    fn committee_requires_attested_rule() {
        assert!(!phase19_committee_requires_attested(0));
        assert!(!phase19_committee_requires_attested(1));
        assert!(phase19_committee_requires_attested(2));
        assert!(phase19_committee_requires_attested(3));
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase19_cap_activation_checklist(None).len() >= 34);
        let plan = phase19_das_succinct_proof_plan();
        assert!(plan.len() >= 34);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan
            .iter()
            .any(|s| s.contains("|pinned") && s.contains("> 1") && s.contains("32")));
        assert!(plan
            .iter()
            .any(|s| s.contains("not BLS") || s.contains("still later")));
        assert!(plan
            .iter()
            .any(|s| s.contains("do not clear") || s.contains("V3")));
    }
}
