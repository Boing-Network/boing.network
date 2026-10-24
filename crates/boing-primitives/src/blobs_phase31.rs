//! Phase 31: consensus-admit / execute BLS aggregate resolve (tx **33**).
//!
//! Builds on Phase 30 (self-contained G1 pubkeys + enum wire). Smallest coherent safe slice:
//! - `PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=true` — mempool admits and VM executes tx **33**.
//! - Execution verifies the self-contained aggregate against the appeal's pin-set commitment,
//!   then stake-weighted-casts each bitset signer (no Ed25519 per voter).
//! - **Do not flip** committee default: multi-pin nets still **must** use ResolveSlashAppealAttested
//!   (**32**) Ed25519 verify-each; tx **33** is an **optional** alternate path operators may submit.
//! - Do **not** clear `slash_appeals_registry.bin` (stay V5); BDKR v1 unchanged.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - ReservedRealV1 verify still rejects; SNARK / 256 MiB still later.
//!
//! Remaining after this slice:
//! 1. Flip committee cast from Ed25519 verify-each → BLS aggregate when operators opt in.
//! 2. Optional Ed25519 account-side binding on a consensus tx.
//! 3. Real IPA/KZG/SNARK for ReservedRealV1; coordinated 256 MiB activation.

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase26::{BlsPublicKeyG1, DualKeyIdentity};
use crate::blobs_phase29::BlsAggregateResolvePayload;
use crate::blobs_phase30::{
    phase30_admit_bls_aggregate_resolve_tx, phase30_bls_aggregate_ballot_preimage,
    phase30_cap_activation_checklist, phase30_committee_requires_attested,
    phase30_das_succinct_proof_plan, phase30_decode_signer_bitset, phase30_encode_signer_bitset,
    phase30_expired_appeal_prunable, phase30_identities_from_self_contained,
    phase30_is_legacy_electorate_snapshot, phase30_pin_set_commitment, phase30_resolve_deadline,
    phase30_resolve_expired, phase30_resolve_threshold_met, phase30_resolved_appeal_prunable,
    phase30_snapshot_electorate_w, phase30_terminal_serve_challenge_prunable,
    phase30_verify_aggregate_resolve_self_contained, phase30_verify_aggregate_resolve_with_registry,
    BlsAggregateResolveSelfContainedPayload, Phase30BlsError, PHASE30_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
    PHASE30_BLS_AGGREGATE_BALLOT_DOMAIN, PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED,
    PHASE30_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED, PHASE30_BLS_AGGREGATE_WIRED,
    PHASE30_BLS_DUAL_KEY_REGISTRY_FILE, PHASE30_BLS_DUAL_KEY_REGISTRY_MAGIC,
    PHASE30_BLS_DUAL_KEY_REGISTRY_VERSION, PHASE30_BLS_POP_REQUIRED,
    PHASE30_BLS_PUBLIC_KEY_G1_BYTES, PHASE30_BLS_SIGNATURE_G2_BYTES, PHASE30_CAP_ACTIVATION_ENV,
    PHASE30_EXPIRED_APPEAL_RETENTION_BLOCKS, PHASE30_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE30_LEGACY_ELECTORATE_W_SENTINEL, PHASE30_LEGACY_PIN_SET_COMMITMENT,
    PHASE30_MAX_APPEAL_EVIDENCE_BYTES, PHASE30_MAX_PIN_SET_LEN, PHASE30_MAX_SETTLE_RECEIPTS,
    PHASE30_PIN_SET_DOMAIN, PHASE30_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX,
    PHASE30_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX, PHASE30_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE30_RESOLVE_THRESHOLD_DEN, PHASE30_RESOLVE_THRESHOLD_NUM, PHASE30_RESOLVE_WINDOW_BLOCKS,
    PHASE30_RESOLVED_APPEAL_RETENTION_BLOCKS, PHASE30_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE30_SETTLE_RECEIPT_TTL_BLOCKS, PHASE30_SUBMIT_SLASH_APPEAL_TX_INDEX,
    PHASE30_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::types::AccountId;

pub const PHASE31_CAP_ACTIVATION_ENV: &str = PHASE30_CAP_ACTIVATION_ENV;
pub const PHASE31_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE30_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
pub const PHASE31_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE30_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
pub const PHASE31_RESOLVE_WINDOW_BLOCKS: u64 = PHASE30_RESOLVE_WINDOW_BLOCKS;
pub const PHASE31_MAX_SETTLE_RECEIPTS: usize = PHASE30_MAX_SETTLE_RECEIPTS;
pub const PHASE31_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE30_SERVE_CHALLENGE_WINDOW_BLOCKS;
pub const PHASE31_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE30_SETTLE_RECEIPT_TTL_BLOCKS;
pub const PHASE31_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE30_EXPIRED_APPEAL_RETENTION_BLOCKS;
pub const PHASE31_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE30_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
pub const PHASE31_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = PHASE30_RESOLVED_APPEAL_RETENTION_BLOCKS;
pub const PHASE31_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE30_MAX_APPEAL_EVIDENCE_BYTES;
pub const PHASE31_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE30_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE31_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE30_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE31_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE30_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
pub const PHASE31_RESOLVE_THRESHOLD_NUM: u64 = PHASE30_RESOLVE_THRESHOLD_NUM;
pub const PHASE31_RESOLVE_THRESHOLD_DEN: u64 = PHASE30_RESOLVE_THRESHOLD_DEN;
pub const PHASE31_PIN_SET_DOMAIN: &[u8] = PHASE30_PIN_SET_DOMAIN;
pub const PHASE31_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE30_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE31_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE30_LEGACY_PIN_SET_COMMITMENT;

pub const PHASE31_BLS_PUBLIC_KEY_G1_BYTES: usize = PHASE30_BLS_PUBLIC_KEY_G1_BYTES;
pub const PHASE31_BLS_SIGNATURE_G2_BYTES: usize = PHASE30_BLS_SIGNATURE_G2_BYTES;
pub const PHASE31_BLS_AGGREGATE_BALLOT_DOMAIN: &[u8] = PHASE30_BLS_AGGREGATE_BALLOT_DOMAIN;
pub const PHASE31_BLS_AGGREGATE_WIRED: bool = PHASE30_BLS_AGGREGATE_WIRED;
pub const PHASE31_BLS_DUAL_KEY_REGISTRY_FILE: &str = PHASE30_BLS_DUAL_KEY_REGISTRY_FILE;
pub const PHASE31_BLS_DUAL_KEY_REGISTRY_MAGIC: &[u8; 4] = PHASE30_BLS_DUAL_KEY_REGISTRY_MAGIC;
pub const PHASE31_BLS_DUAL_KEY_REGISTRY_VERSION: u32 = PHASE30_BLS_DUAL_KEY_REGISTRY_VERSION;
pub const PHASE31_BLS_POP_REQUIRED: bool = PHASE30_BLS_POP_REQUIRED;
pub const PHASE31_MAX_PIN_SET_LEN: usize = PHASE30_MAX_PIN_SET_LEN;
pub const PHASE31_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX: u32 =
    PHASE30_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX;
pub const PHASE31_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED: bool =
    PHASE30_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED;
pub const PHASE31_BLS_AGGREGATE_SELF_CONTAINED_WIRED: bool =
    PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED;

/// Consensus admit / execute of tx **33** is **on** (optional path; committee default stays **32**).
pub const PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED: bool = true;

/// Live committee cast for multi-pin nets remains Ed25519 tx **32** (not flipped this phase).
pub const PHASE31_BLS_LIVE_TX32_STILL_ED25519: bool = true;

pub type Phase31BlsError = Phase30BlsError;

/// Admit helper — Ok when Phase 31 consensus wire is on.
pub fn phase31_admit_bls_aggregate_resolve_tx() -> Result<(), Phase31BlsError> {
    if PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED {
        Ok(())
    } else {
        // Keep Phase 30 error shape if ever toggled off.
        let _ = phase30_admit_bls_aggregate_resolve_tx();
        Err(Phase30BlsError::ConsensusSubmitNotWired)
    }
}

pub fn phase31_bls_aggregate_ballot_preimage(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    pin_set_commitment: &[u8; 32],
) -> [u8; 32] {
    phase30_bls_aggregate_ballot_preimage(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        pin_set_commitment,
    )
}

pub fn phase31_encode_signer_bitset(
    pinned: &[AccountId],
    signers: &[AccountId],
) -> Result<Vec<u8>, Phase31BlsError> {
    phase30_encode_signer_bitset(pinned, signers)
}

pub fn phase31_decode_signer_bitset(
    pinned: &[AccountId],
    bitset: &[u8],
) -> Result<Vec<AccountId>, Phase31BlsError> {
    phase30_decode_signer_bitset(pinned, bitset)
}

pub fn phase31_identities_from_self_contained(
    pinned: &[AccountId],
    bitset: &[u8],
    pubkeys: &[BlsPublicKeyG1],
) -> Result<Vec<DualKeyIdentity>, Phase31BlsError> {
    phase30_identities_from_self_contained(pinned, bitset, pubkeys)
}

pub fn phase31_verify_aggregate_resolve_self_contained(
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolveSelfContainedPayload,
) -> Result<(), Phase31BlsError> {
    phase30_verify_aggregate_resolve_self_contained(
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        payload,
    )
}

pub fn phase31_verify_aggregate_resolve_with_registry(
    registry: &crate::blobs_phase27::BlsDualKeyRegistry,
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolvePayload,
) -> Result<(), Phase31BlsError> {
    phase30_verify_aggregate_resolve_with_registry(
        registry,
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        payload,
    )
}

pub fn phase31_resolve_deadline(submit_height: u64) -> u64 {
    phase30_resolve_deadline(submit_height)
}

pub fn phase31_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase30_resolve_expired(tip_height, resolve_deadline)
}

pub fn phase31_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase30_expired_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase31_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase30_resolved_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase31_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase30_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase31_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase30_snapshot_electorate_w(pinned_stakes)
}

pub fn phase31_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase30_pin_set_commitment(pinned)
}

pub fn phase31_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase30_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase31_committee_requires_attested(pinned_len: usize) -> bool {
    // Still means "must use attested Ed25519 32" — not flipped to require BLS 33.
    phase30_committee_requires_attested(pinned_len)
}

pub fn phase31_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase30_resolve_threshold_met(weight, electorate_w)
}

pub fn phase31_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase30_das_succinct_proof_plan();
    v.extend([
        "Phase 31: consensus-admit / execute ResolveSlashAppealBlsAggregate (33) with self-contained pubkey verify → stake-weighted cast for each bitset signer.",
        "Phase 31: PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=true; live multi-pin committee default stays ResolveSlashAppealAttested (32) Ed25519 verify-each (PHASE31_BLS_LIVE_TX32_STILL_ED25519=true).",
        "Phase 31: remaining — flip committee default to BLS aggregate when operators opt in, optional Ed25519 account-side binding, SNARK / 256 MiB.",
        "Phase 31: slash_appeals_registry.bin stays V5; bls_dual_key_registry.bin stays BDKR v1; do not clear.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

pub fn phase31_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase30_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 31: tx 33 is consensus-admitted; keep multi-pin default on Ed25519 32 until a later opt-in flip.",
        "Phase 31: do not clear slash_appeals_registry.bin — stay V5; dual-keys remain host-local BDKR v1.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs_phase27::{
        phase27_aggregate_bls_signatures, phase27_bls_pubkey_from_ikm, phase27_bls_sign,
    };

    #[test]
    fn consensus_wired_and_verify_still_works() {
        assert!(PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED);
        assert!(PHASE31_BLS_LIVE_TX32_STILL_ED25519);
        assert!(phase31_admit_bls_aggregate_resolve_tx().is_ok());
        let ikm1 = [41u8; 32];
        let ikm2 = [42u8; 32];
        let pk1 = phase27_bls_pubkey_from_ikm(&ikm1).unwrap();
        let pk2 = phase27_bls_pubkey_from_ikm(&ikm2).unwrap();
        let a1 = AccountId([0x41; 32]);
        let a2 = AccountId([0x42; 32]);
        let pinned = [a1, a2];
        let pin_commit = phase31_pin_set_commitment(&pinned);
        let msg = phase31_bls_aggregate_ballot_preimage(6913, 12, 9, true, &pin_commit);
        let s1 = phase27_bls_sign(&ikm1, &msg).unwrap();
        let s2 = phase27_bls_sign(&ikm2, &msg).unwrap();
        let agg = phase27_aggregate_bls_signatures(&[s1, s2]).unwrap();
        let bits = phase31_encode_signer_bitset(&pinned, &[a1, a2]).unwrap();
        let payload = BlsAggregateResolveSelfContainedPayload {
            appeal_id: 12,
            approved: true,
            signer_bitset: bits,
            aggregate_signature: agg,
            signer_bls_pubkeys: vec![pk1, pk2],
        };
        assert!(phase31_verify_aggregate_resolve_self_contained(
            6913, 9, &pin_commit, &pinned, &payload
        )
        .is_ok());
        let plan = phase31_das_succinct_proof_plan();
        assert!(plan.iter().any(|s| s.contains("Phase 31")));
        assert!(plan.iter().any(|s| s.contains("32") && s.contains("Ed25519")));
    }
}
