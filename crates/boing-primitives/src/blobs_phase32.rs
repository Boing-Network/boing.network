//! Phase 32: opt-in flip of live multi-pin committee verify from Ed25519 tx **32** → BLS aggregate tx **33**.
//!
//! Builds on Phase 31 (consensus-admit/execute tx **33** as an optional path). Smallest coherent safe slice:
//! - Env / CLI gated activation height (same pattern as Phase 7 cap): `BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT`.
//! - **Unset flip** → identical to Phase 31: multi-pin still **must** use ResolveSlashAppealAttested (**32**);
//!   tx **33** remains an optional alternate path.
//! - **Flip active** (`Some(h)` and `height >= h`) and `pinned_len > 1` → multi-pin **requires** tx **33**;
//!   reject **31** and **32**. Solo (`pinned_len <= 1`) always keeps Ed25519 / unattested policy.
//! - Do **not** clear `slash_appeals_registry.bin` (stay V5); BDKR v1 unchanged.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Historical replay of tx **32** before the flip height remains valid.
//! - ReservedRealV1 verify still rejects; SNARK / 256 MiB still later.
//!
//! Remaining after this slice:
//! 1. Optional Ed25519 account-side binding on a consensus tx.
//! 2. Real IPA/KZG/SNARK for ReservedRealV1; coordinated 256 MiB activation.

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase26::{BlsPublicKeyG1, DualKeyIdentity};
use crate::blobs_phase29::BlsAggregateResolvePayload;
use crate::blobs_phase30::BlsAggregateResolveSelfContainedPayload;
use crate::blobs_phase31::{
    phase31_admit_bls_aggregate_resolve_tx, phase31_bls_aggregate_ballot_preimage,
    phase31_cap_activation_checklist, phase31_committee_requires_attested,
    phase31_das_succinct_proof_plan, phase31_decode_signer_bitset, phase31_encode_signer_bitset,
    phase31_expired_appeal_prunable, phase31_identities_from_self_contained,
    phase31_is_legacy_electorate_snapshot, phase31_pin_set_commitment, phase31_resolve_deadline,
    phase31_resolve_expired, phase31_resolve_threshold_met, phase31_resolved_appeal_prunable,
    phase31_snapshot_electorate_w, phase31_terminal_serve_challenge_prunable,
    phase31_verify_aggregate_resolve_self_contained,
    phase31_verify_aggregate_resolve_with_registry, Phase31BlsError,
    PHASE31_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE31_BLS_AGGREGATE_BALLOT_DOMAIN,
    PHASE31_BLS_AGGREGATE_SELF_CONTAINED_WIRED, PHASE31_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED,
    PHASE31_BLS_AGGREGATE_WIRED, PHASE31_BLS_DUAL_KEY_REGISTRY_FILE,
    PHASE31_BLS_DUAL_KEY_REGISTRY_MAGIC, PHASE31_BLS_DUAL_KEY_REGISTRY_VERSION,
    PHASE31_BLS_POP_REQUIRED, PHASE31_BLS_PUBLIC_KEY_G1_BYTES, PHASE31_BLS_SIGNATURE_G2_BYTES,
    PHASE31_CAP_ACTIVATION_ENV, PHASE31_EXPIRED_APPEAL_RETENTION_BLOCKS,
    PHASE31_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE31_LEGACY_ELECTORATE_W_SENTINEL,
    PHASE31_LEGACY_PIN_SET_COMMITMENT, PHASE31_MAX_APPEAL_EVIDENCE_BYTES, PHASE31_MAX_PIN_SET_LEN,
    PHASE31_MAX_SETTLE_RECEIPTS, PHASE31_PIN_SET_DOMAIN, PHASE31_RESOLVED_APPEAL_RETENTION_BLOCKS,
    PHASE31_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX,
    PHASE31_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX, PHASE31_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE31_RESOLVE_THRESHOLD_DEN, PHASE31_RESOLVE_THRESHOLD_NUM, PHASE31_RESOLVE_WINDOW_BLOCKS,
    PHASE31_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE31_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE31_SUBMIT_SLASH_APPEAL_TX_INDEX, PHASE31_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::types::AccountId;

/// Env / node knob name: opt-in BLS multi-pin committee flip height.
pub const PHASE32_BLS_COMMITTEE_FLIP_ENV: &str = "BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT";

pub const PHASE32_CAP_ACTIVATION_ENV: &str = PHASE31_CAP_ACTIVATION_ENV;
pub const PHASE32_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE31_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
pub const PHASE32_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE31_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
pub const PHASE32_RESOLVE_WINDOW_BLOCKS: u64 = PHASE31_RESOLVE_WINDOW_BLOCKS;
pub const PHASE32_MAX_SETTLE_RECEIPTS: usize = PHASE31_MAX_SETTLE_RECEIPTS;
pub const PHASE32_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE31_SERVE_CHALLENGE_WINDOW_BLOCKS;
pub const PHASE32_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE31_SETTLE_RECEIPT_TTL_BLOCKS;
pub const PHASE32_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE31_EXPIRED_APPEAL_RETENTION_BLOCKS;
pub const PHASE32_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE31_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
pub const PHASE32_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = PHASE31_RESOLVED_APPEAL_RETENTION_BLOCKS;
pub const PHASE32_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE31_MAX_APPEAL_EVIDENCE_BYTES;
pub const PHASE32_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE31_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE32_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE31_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE32_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE31_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
pub const PHASE32_RESOLVE_THRESHOLD_NUM: u64 = PHASE31_RESOLVE_THRESHOLD_NUM;
pub const PHASE32_RESOLVE_THRESHOLD_DEN: u64 = PHASE31_RESOLVE_THRESHOLD_DEN;
pub const PHASE32_PIN_SET_DOMAIN: &[u8] = PHASE31_PIN_SET_DOMAIN;
pub const PHASE32_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE31_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE32_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE31_LEGACY_PIN_SET_COMMITMENT;

pub const PHASE32_BLS_PUBLIC_KEY_G1_BYTES: usize = PHASE31_BLS_PUBLIC_KEY_G1_BYTES;
pub const PHASE32_BLS_SIGNATURE_G2_BYTES: usize = PHASE31_BLS_SIGNATURE_G2_BYTES;
pub const PHASE32_BLS_AGGREGATE_BALLOT_DOMAIN: &[u8] = PHASE31_BLS_AGGREGATE_BALLOT_DOMAIN;
pub const PHASE32_BLS_AGGREGATE_WIRED: bool = PHASE31_BLS_AGGREGATE_WIRED;
pub const PHASE32_BLS_DUAL_KEY_REGISTRY_FILE: &str = PHASE31_BLS_DUAL_KEY_REGISTRY_FILE;
pub const PHASE32_BLS_DUAL_KEY_REGISTRY_MAGIC: &[u8; 4] = PHASE31_BLS_DUAL_KEY_REGISTRY_MAGIC;
pub const PHASE32_BLS_DUAL_KEY_REGISTRY_VERSION: u32 = PHASE31_BLS_DUAL_KEY_REGISTRY_VERSION;
pub const PHASE32_BLS_POP_REQUIRED: bool = PHASE31_BLS_POP_REQUIRED;
pub const PHASE32_MAX_PIN_SET_LEN: usize = PHASE31_MAX_PIN_SET_LEN;
pub const PHASE32_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX: u32 =
    PHASE31_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX;
pub const PHASE32_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED: bool =
    PHASE31_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED;
pub const PHASE32_BLS_AGGREGATE_SELF_CONTAINED_WIRED: bool =
    PHASE31_BLS_AGGREGATE_SELF_CONTAINED_WIRED;

/// Consensus admit / execute of tx **33** remains **on** (Phase 31).
pub const PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED: bool = true;

/// Compile-time default: until operators set the flip height, live multi-pin cast stays Ed25519 tx **32**.
/// RPC should prefer a **dynamic** `blsLiveTx32StillEd25519` (= `!flip_active_at_tip`).
pub const PHASE32_BLS_LIVE_TX32_STILL_ED25519: bool = true;

pub type Phase32BlsError = Phase31BlsError;

/// True when flip is configured and `height >= flip_from`.
pub fn phase32_bls_committee_flip_active(height: u64, flip_from: Option<u64>) -> bool {
    match flip_from {
        Some(h) if height >= h => true,
        _ => false,
    }
}

/// When flip **not** active: same as Phase 31 (`pinned_len > 1` ⇒ require Ed25519 **32**).
/// When flip **active**: return **false** (multi-pin no longer requires Ed25519 **32**).
pub fn phase32_committee_requires_attested(
    pinned_len: usize,
    height: u64,
    flip_from: Option<u64>,
) -> bool {
    if phase32_bls_committee_flip_active(height, flip_from) {
        false
    } else {
        phase31_committee_requires_attested(pinned_len)
    }
}

/// True when flip is active **and** `pinned_len > 1` (multi-pin must use tx **33**).
pub fn phase32_committee_requires_bls_aggregate(
    pinned_len: usize,
    height: u64,
    flip_from: Option<u64>,
) -> bool {
    phase32_bls_committee_flip_active(height, flip_from) && pinned_len > 1
}

/// Admit helper — Ok when Phase 32 consensus wire is on (same as Phase 31).
pub fn phase32_admit_bls_aggregate_resolve_tx() -> Result<(), Phase32BlsError> {
    phase31_admit_bls_aggregate_resolve_tx()
}

pub fn phase32_bls_aggregate_ballot_preimage(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    pin_set_commitment: &[u8; 32],
) -> [u8; 32] {
    phase31_bls_aggregate_ballot_preimage(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        pin_set_commitment,
    )
}

pub fn phase32_encode_signer_bitset(
    pinned: &[AccountId],
    signers: &[AccountId],
) -> Result<Vec<u8>, Phase32BlsError> {
    phase31_encode_signer_bitset(pinned, signers)
}

pub fn phase32_decode_signer_bitset(
    pinned: &[AccountId],
    bitset: &[u8],
) -> Result<Vec<AccountId>, Phase32BlsError> {
    phase31_decode_signer_bitset(pinned, bitset)
}

pub fn phase32_identities_from_self_contained(
    pinned: &[AccountId],
    bitset: &[u8],
    pubkeys: &[BlsPublicKeyG1],
) -> Result<Vec<DualKeyIdentity>, Phase32BlsError> {
    phase31_identities_from_self_contained(pinned, bitset, pubkeys)
}

pub fn phase32_verify_aggregate_resolve_self_contained(
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolveSelfContainedPayload,
) -> Result<(), Phase32BlsError> {
    phase31_verify_aggregate_resolve_self_contained(
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        payload,
    )
}

pub fn phase32_verify_aggregate_resolve_with_registry(
    registry: &crate::blobs_phase27::BlsDualKeyRegistry,
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolvePayload,
) -> Result<(), Phase32BlsError> {
    phase31_verify_aggregate_resolve_with_registry(
        registry,
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        payload,
    )
}

pub fn phase32_resolve_deadline(submit_height: u64) -> u64 {
    phase31_resolve_deadline(submit_height)
}

pub fn phase32_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase31_resolve_expired(tip_height, resolve_deadline)
}

pub fn phase32_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase31_expired_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase32_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase31_resolved_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase32_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase31_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase32_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase31_snapshot_electorate_w(pinned_stakes)
}

pub fn phase32_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase31_pin_set_commitment(pinned)
}

pub fn phase32_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase31_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase32_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase31_resolve_threshold_met(weight, electorate_w)
}

pub fn phase32_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase31_das_succinct_proof_plan();
    v.extend([
        "Phase 32: opt-in flip of live multi-pin committee verify from Ed25519 ResolveSlashAppealAttested (32) → ResolveSlashAppealBlsAggregate (33), gated by BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT.",
        "Phase 32: PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=true; PHASE32_BLS_LIVE_TX32_STILL_ED25519=true as compile-time default until flip; unset flip ⇒ identical to Phase 31.",
        "Phase 32: when flip active and |pinned|>1, reject tx 31/32 and require 33; solo (|pinned|≤1) keeps Ed25519 / unattested policy.",
        "Phase 32: remaining — optional Ed25519 account-side binding, SNARK / 256 MiB.",
        "Phase 32: slash_appeals_registry.bin stays V5; bls_dual_key_registry.bin stays BDKR v1; do not clear.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

pub fn phase32_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase31_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 32: leave BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT unset by default (Fly-safe = Phase 31 Ed25519 32 multi-pin default).",
        "Phase 32: only set identical flip height on all validators after BLS dual-key + tx 33 operational readiness.",
        "Phase 32: do not clear slash_appeals_registry.bin — stay V5; dual-keys remain host-local BDKR v1.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_active_inactive() {
        assert!(!phase32_bls_committee_flip_active(0, None));
        assert!(!phase32_bls_committee_flip_active(99, Some(100)));
        assert!(phase32_bls_committee_flip_active(100, Some(100)));
        assert!(phase32_bls_committee_flip_active(101, Some(100)));
    }

    #[test]
    fn solo_vs_multi_pin_before_and_after_flip() {
        // Unset flip: Phase 31 policy — multi-pin requires attested 32; not BLS 33.
        assert!(!phase32_committee_requires_attested(0, 50, None));
        assert!(!phase32_committee_requires_attested(1, 50, None));
        assert!(phase32_committee_requires_attested(2, 50, None));
        assert!(!phase32_committee_requires_bls_aggregate(2, 50, None));
        assert!(!phase32_committee_requires_bls_aggregate(1, 50, None));

        // Below flip height: same as unset.
        assert!(phase32_committee_requires_attested(3, 99, Some(100)));
        assert!(!phase32_committee_requires_bls_aggregate(3, 99, Some(100)));

        // Flip active: multi-pin requires BLS 33; attested no longer required.
        assert!(!phase32_committee_requires_attested(3, 100, Some(100)));
        assert!(phase32_committee_requires_bls_aggregate(3, 100, Some(100)));
        // Solo always keeps Ed25519 path even when flip active.
        assert!(!phase32_committee_requires_attested(1, 100, Some(100)));
        assert!(!phase32_committee_requires_bls_aggregate(1, 100, Some(100)));
        assert!(!phase32_committee_requires_bls_aggregate(0, 100, Some(100)));
    }

    #[test]
    fn consensus_wired_and_defaults() {
        assert!(PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED);
        assert!(PHASE32_BLS_LIVE_TX32_STILL_ED25519);
        assert_eq!(
            PHASE32_BLS_COMMITTEE_FLIP_ENV,
            "BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT"
        );
        assert!(phase32_admit_bls_aggregate_resolve_tx().is_ok());
        let plan = phase32_das_succinct_proof_plan();
        assert!(plan.iter().any(|s| s.contains("Phase 32")));
        assert!(plan.iter().any(|s| s.contains("BLS_COMMITTEE_FLIP_HEIGHT")));
        let checklist = phase32_cap_activation_checklist(None);
        assert!(checklist
            .iter()
            .any(|s| s.contains("BLS_COMMITTEE_FLIP_HEIGHT")));
    }
}
