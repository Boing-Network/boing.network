//! Phase 20: snapshot electorate W (and pin-set commitment) at SubmitSlashAppeal.
//!
//! When **SubmitSlashAppeal (30)** succeeds, persist `electorate_w_snapshot` (and a
//! domain-separated pin-set commitment) on the appeal. **Resolve (31/32)** uses the
//! frozen W for ⌈2W/3⌉ instead of live restaking mid-tally. Voter weight remains
//! bonded stake **at cast**.
//!
//! Honest scope:
//! - Reuse payload indices **30** / **31** / **32** — no new tx.
//! - Registry **V4** (V3/V2/legacy load); **do not clear** `slash_appeals_registry.bin`.
//! - Legacy Pending rows: `electorate_w_snapshot == 0` **and** zero pin commitment
//!   ⇒ live-W fallback each cast (do not strand mid-tally upgrades).
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Phase 19 multi-pin → require **32** unchanged.
//! - Appeal/resolve expiry, true BLS, SNARK/IPA/KZG, live 256 MiB — still later (21+).

use crate::blobs_phase19::{
    phase19_cap_activation_checklist, phase19_committee_requires_attested,
    phase19_das_succinct_proof_plan, phase19_resolve_threshold_met,
    PHASE19_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE19_CAP_ACTIVATION_ENV,
    PHASE19_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE19_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE19_MAX_SETTLE_RECEIPTS, PHASE19_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX,
    PHASE19_RESOLVE_SLASH_APPEAL_TX_INDEX, PHASE19_RESOLVE_THRESHOLD_DEN,
    PHASE19_RESOLVE_THRESHOLD_NUM, PHASE19_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE19_SETTLE_RECEIPT_TTL_BLOCKS, PHASE19_SUBMIT_SLASH_APPEAL_TX_INDEX,
};
use crate::hash::hasher;
use crate::types::AccountId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE20_CAP_ACTIVATION_ENV: &str = PHASE19_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE20_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE19_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE20_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE19_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE20_MAX_SETTLE_RECEIPTS: usize = PHASE19_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE20_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE19_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE20_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE19_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE20_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE19_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE20_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE19_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`] (unattested).
pub const PHASE20_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE19_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppealAttested`].
pub const PHASE20_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE19_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;

/// Numerator of the fixed stake-weighted resolve quorum (⌈2W/3⌉).
pub const PHASE20_RESOLVE_THRESHOLD_NUM: u64 = PHASE19_RESOLVE_THRESHOLD_NUM;

/// Denominator of the fixed stake-weighted resolve quorum.
pub const PHASE20_RESOLVE_THRESHOLD_DEN: u64 = PHASE19_RESOLVE_THRESHOLD_DEN;

/// Domain tag for pin-set commitment at Submit.
pub const PHASE20_PIN_SET_DOMAIN: &[u8] = b"BOING_SLASH_APPEAL_PIN_SET_V1";

/// Sentinel: missing snapshot on migrated pre-V4 Pending appeals (live-W fallback).
pub const PHASE20_LEGACY_ELECTORATE_W_SENTINEL: u128 = 0;

/// Zero pin commitment paired with [`PHASE20_LEGACY_ELECTORATE_W_SENTINEL`] ⇒ legacy row.
pub const PHASE20_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = [0u8; 32];

/// Snapshot electorate W from bonded stakes over the pin set at Submit.
///
/// Empty pin (unpinned / unset) → **1** (solo one-shot). Non-empty → saturating sum
/// (may be **0**, which stays Pending until a new appeal after rebond — do not rewrite).
pub fn phase20_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    if pinned_stakes.is_empty() {
        return 1;
    }
    pinned_stakes
        .iter()
        .fold(0u128, |acc, &s| acc.saturating_add(s))
}

/// Domain-separated commitment over the sorted pinned `AccountId` list.
///
/// Empty / unpinned → `H(domain || "UNPINNED")` (never all-zero, so distinct from legacy).
pub fn phase20_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    let mut h = hasher();
    h.update(PHASE20_PIN_SET_DOMAIN);
    if pinned.is_empty() {
        h.update(b"UNPINNED");
        return *h.finalize().as_bytes();
    }
    let mut sorted = pinned.to_vec();
    sorted.sort();
    h.update(&(sorted.len() as u64).to_le_bytes());
    for id in &sorted {
        h.update(&id.0);
    }
    *h.finalize().as_bytes()
}

/// True when a migrated pre-V4 appeal should recompute live W each cast.
pub fn phase20_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    electorate_w_snapshot == PHASE20_LEGACY_ELECTORATE_W_SENTINEL
        && *pin_set_commitment == PHASE20_LEGACY_PIN_SET_COMMITMENT
}

/// True when a multi-pin committee must use attested resolve (**32**) (Phase 19 alias).
pub fn phase20_committee_requires_attested(pinned_len: usize) -> bool {
    phase19_committee_requires_attested(pinned_len)
}

/// True when `weight` meets ⌈2W/3⌉ (alias of Phase 17/18/19 integer compare).
pub fn phase20_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase19_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 20 electorate snapshot at Submit).
pub fn phase20_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase19_das_succinct_proof_plan();
    v.extend([
        "Phase 20: SubmitSlashAppeal (30) snapshots electorate_w (Σ bonded stake over pinned slash_appeal_resolvers) and a pin-set commitment; Resolve (31/32) uses frozen W for ⌈2W/3⌉.",
        "Phase 20: voter weight remains bonded stake at cast; denominator is Submit-height W (not live restake mid-tally).",
        "Phase 20: unpinned / empty pin → snapshot W=1; W=0 with non-empty pin stays Pending (no rewrite); pin-set change vs snapshot commitment rejects Resolve.",
        "Phase 20: slash_appeals_registry.bin is V4; V3/V2/legacy load migrates without required clear — pre-V4 Pending uses live-W fallback (sentinel 0 + zero pin commit).",
        "Phase 20: appeal/resolve expiry, true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 19 + Phase 20 ops). Do not rewrite live admission height.
pub fn phase20_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase19_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 20: do not clear slash_appeals_registry.bin — save V4; load V4 or migrate V3/V2/legacy in memory.",
        "Phase 20: prefer stable pin set + bonded resolvers before Submit; pin mutation mid-Pending rejects Resolve (commitment mismatch).",
        "Phase 20: mid-upgrade Pending appeals without snapshot use live-W fallback; new Submits always write a real snapshot.",
        "Phase 20: appeal/resolve expiry cleanup and true BLS are still later — this phase freezes electorate W only.",
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
        assert_eq!(PHASE20_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE20_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE20_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE20_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(PHASE20_RESOLVE_THRESHOLD_NUM, 2);
        assert_eq!(PHASE20_RESOLVE_THRESHOLD_DEN, 3);
        assert_eq!(
            PHASE20_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE19_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
    }

    #[test]
    fn snapshot_w_empty_and_sum() {
        assert_eq!(phase20_snapshot_electorate_w(&[]), 1);
        assert_eq!(phase20_snapshot_electorate_w(&[100, 100, 100]), 300);
        assert_eq!(phase20_snapshot_electorate_w(&[0, 0]), 0);
    }

    #[test]
    fn pin_set_commitment_stable_and_sorted() {
        let a = AccountId([1u8; 32]);
        let b = AccountId([2u8; 32]);
        let c1 = phase20_pin_set_commitment(&[a, b]);
        let c2 = phase20_pin_set_commitment(&[b, a]);
        assert_eq!(c1, c2);
        assert_ne!(c1, PHASE20_LEGACY_PIN_SET_COMMITMENT);
        let unpinned = phase20_pin_set_commitment(&[]);
        assert_ne!(unpinned, PHASE20_LEGACY_PIN_SET_COMMITMENT);
        assert_ne!(unpinned, c1);
    }

    #[test]
    fn legacy_sentinel_detection() {
        assert!(phase20_is_legacy_electorate_snapshot(
            0,
            &PHASE20_LEGACY_PIN_SET_COMMITMENT
        ));
        let real_pin = phase20_pin_set_commitment(&[AccountId([9u8; 32])]);
        // Real W=0 snapshot with a non-legacy pin commit is NOT legacy.
        assert!(!phase20_is_legacy_electorate_snapshot(0, &real_pin));
        assert!(!phase20_is_legacy_electorate_snapshot(
            300,
            &PHASE20_LEGACY_PIN_SET_COMMITMENT
        ));
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase20_cap_activation_checklist(None).len() >= 40);
        let plan = phase20_das_succinct_proof_plan();
        assert!(plan.len() >= 40);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan.iter().any(|s| s.contains("snapshots electorate")
            || s.contains("electorate_w")
            || s.contains("frozen W")));
        assert!(plan
            .iter()
            .any(|s| s.contains("V4") && (s.contains("do not clear") || s.contains("without required clear") || s.contains("migrates"))));
        assert!(plan.iter().any(|s| s.contains("still later") || s.contains("BLS")));
        assert!(phase20_committee_requires_attested(3));
        assert!(!phase20_committee_requires_attested(1));
    }
}
