//! Phase 30: self-contained BLS pubkey list on aggregate-resolve payload.
//!
//! Builds on Phase 29 (bitset + dry-run). Smallest coherent slice toward consensus admit:
//! - Aggregate-resolve payload carries ordered G1 pubkeys matching the signer bitset
//!   (ascending pin index) so verify needs **no** host-local dual-key registry.
//! - Wire `TransactionPayload::ResolveSlashAppealBlsAggregate` at index **33**.
//! - Library + dry-run RPC verify self-contained path (`PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED=true`).
//! - **Consensus admit of tx 33 stays false** (`PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=false`)
//!   — mempool/VM reject with a clear error until operators opt in.
//! - Do **not** change live ResolveSlashAppealAttested (**32**) Ed25519 verify-each.
//! - Do **not** clear `slash_appeals_registry.bin` (stay V5); BDKR v1 unchanged.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - ReservedRealV1 verify still rejects; SNARK / 256 MiB still later.
//!
//! Remaining after this slice:
//! 1. Consensus-admit / execute tx **33** (self-contained verify → stake-weighted finalize).
//! 2. Flip committee cast from Ed25519 verify-each → BLS aggregate when operators opt in.
//! 3. Optional Ed25519 account-side binding on a consensus tx.

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase26::{
    phase26_validate_dual_key_set, BlsAggregateError, BlsPublicKeyG1, BlsSignatureG2,
    DualKeyIdentity,
};
use crate::blobs_phase29::{
    phase29_bls_aggregate_ballot_preimage, phase29_cap_activation_checklist,
    phase29_committee_requires_attested, phase29_das_succinct_proof_plan,
    phase29_decode_signer_bitset, phase29_encode_signer_bitset, phase29_expired_appeal_prunable,
    phase29_is_legacy_electorate_snapshot, phase29_pin_set_commitment, phase29_resolve_deadline,
    phase29_resolve_expired, phase29_resolve_threshold_met, phase29_resolved_appeal_prunable,
    phase29_snapshot_electorate_w, phase29_terminal_serve_challenge_prunable,
    phase29_verify_aggregate_resolve, phase29_verify_aggregate_resolve_with_registry,
    BlsAggregateResolvePayload, Phase29BlsError, PHASE29_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
    PHASE29_BLS_AGGREGATE_BALLOT_DOMAIN, PHASE29_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED,
    PHASE29_BLS_AGGREGATE_WIRED, PHASE29_BLS_DUAL_KEY_REGISTRY_FILE,
    PHASE29_BLS_DUAL_KEY_REGISTRY_MAGIC, PHASE29_BLS_DUAL_KEY_REGISTRY_VERSION,
    PHASE29_BLS_POP_REQUIRED, PHASE29_BLS_PUBLIC_KEY_G1_BYTES, PHASE29_BLS_SIGNATURE_G2_BYTES,
    PHASE29_CAP_ACTIVATION_ENV, PHASE29_EXPIRED_APPEAL_RETENTION_BLOCKS,
    PHASE29_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE29_LEGACY_ELECTORATE_W_SENTINEL,
    PHASE29_LEGACY_PIN_SET_COMMITMENT, PHASE29_MAX_APPEAL_EVIDENCE_BYTES, PHASE29_MAX_PIN_SET_LEN,
    PHASE29_MAX_SETTLE_RECEIPTS, PHASE29_PIN_SET_DOMAIN,
    PHASE29_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE29_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX,
    PHASE29_RESOLVE_SLASH_APPEAL_TX_INDEX, PHASE29_RESOLVE_THRESHOLD_DEN,
    PHASE29_RESOLVE_THRESHOLD_NUM, PHASE29_RESOLVE_WINDOW_BLOCKS,
    PHASE29_RESOLVED_APPEAL_RETENTION_BLOCKS, PHASE29_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE29_SETTLE_RECEIPT_TTL_BLOCKS, PHASE29_SUBMIT_SLASH_APPEAL_TX_INDEX,
    PHASE29_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::types::AccountId;
use serde::{Deserialize, Serialize};

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE30_CAP_ACTIVATION_ENV: &str = PHASE29_CAP_ACTIVATION_ENV;

pub const PHASE30_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE29_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
pub const PHASE30_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE29_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
pub const PHASE30_RESOLVE_WINDOW_BLOCKS: u64 = PHASE29_RESOLVE_WINDOW_BLOCKS;
pub const PHASE30_MAX_SETTLE_RECEIPTS: usize = PHASE29_MAX_SETTLE_RECEIPTS;
pub const PHASE30_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE29_SERVE_CHALLENGE_WINDOW_BLOCKS;
pub const PHASE30_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE29_SETTLE_RECEIPT_TTL_BLOCKS;
pub const PHASE30_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE29_EXPIRED_APPEAL_RETENTION_BLOCKS;
pub const PHASE30_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE29_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
pub const PHASE30_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = PHASE29_RESOLVED_APPEAL_RETENTION_BLOCKS;
pub const PHASE30_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE29_MAX_APPEAL_EVIDENCE_BYTES;
pub const PHASE30_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE29_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE30_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE29_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE30_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE29_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
pub const PHASE30_RESOLVE_THRESHOLD_NUM: u64 = PHASE29_RESOLVE_THRESHOLD_NUM;
pub const PHASE30_RESOLVE_THRESHOLD_DEN: u64 = PHASE29_RESOLVE_THRESHOLD_DEN;
pub const PHASE30_PIN_SET_DOMAIN: &[u8] = PHASE29_PIN_SET_DOMAIN;
pub const PHASE30_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE29_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE30_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE29_LEGACY_PIN_SET_COMMITMENT;

pub const PHASE30_BLS_PUBLIC_KEY_G1_BYTES: usize = PHASE29_BLS_PUBLIC_KEY_G1_BYTES;
pub const PHASE30_BLS_SIGNATURE_G2_BYTES: usize = PHASE29_BLS_SIGNATURE_G2_BYTES;
pub const PHASE30_BLS_AGGREGATE_BALLOT_DOMAIN: &[u8] = PHASE29_BLS_AGGREGATE_BALLOT_DOMAIN;
pub const PHASE30_BLS_AGGREGATE_WIRED: bool = PHASE29_BLS_AGGREGATE_WIRED;
pub const PHASE30_BLS_DUAL_KEY_REGISTRY_FILE: &str = PHASE29_BLS_DUAL_KEY_REGISTRY_FILE;
pub const PHASE30_BLS_DUAL_KEY_REGISTRY_MAGIC: &[u8; 4] = PHASE29_BLS_DUAL_KEY_REGISTRY_MAGIC;
pub const PHASE30_BLS_DUAL_KEY_REGISTRY_VERSION: u32 = PHASE29_BLS_DUAL_KEY_REGISTRY_VERSION;
pub const PHASE30_BLS_POP_REQUIRED: bool = PHASE29_BLS_POP_REQUIRED;
pub const PHASE30_MAX_PIN_SET_LEN: usize = PHASE29_MAX_PIN_SET_LEN;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppealBlsAggregate`].
pub const PHASE30_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX: u32 =
    PHASE29_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX;

pub const PHASE30_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED: bool =
    PHASE29_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED;

/// Self-contained pubkey list verify path is available (no host registry required).
pub const PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED: bool = true;

/// Consensus admit / execute of tx **33** stays **false** until operators opt in.
pub const PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED: bool = false;

/// Errors for Phase 30 self-contained aggregate-resolve helpers.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Phase30BlsError {
    #[error(transparent)]
    Inner(#[from] Phase29BlsError),
    #[error(transparent)]
    Aggregate(#[from] BlsAggregateError),
    #[error("signer pubkey count {got} != bitset signer count {expected}")]
    PubkeyCountMismatch { got: usize, expected: usize },
    #[error("BLS aggregate resolve consensus admit not wired (PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=false); use ResolveSlashAppealAttested (32)")]
    ConsensusSubmitNotWired,
}

/// Aggregate resolve ballot with self-contained G1 pubkeys (library / tx **33** body).
///
/// `signer_bls_pubkeys` is ordered to match `phase29_decode_signer_bitset` (ascending pin index).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlsAggregateResolveSelfContainedPayload {
    pub appeal_id: u64,
    pub approved: bool,
    /// Bitset over Submit-frozen pin-set order (LSB of byte 0 = `pinned[0]`).
    pub signer_bitset: Vec<u8>,
    pub aggregate_signature: BlsSignatureG2,
    /// Compressed G1 pubkeys, 1:1 with decoded bitset signers (ascending pin index).
    pub signer_bls_pubkeys: Vec<BlsPublicKeyG1>,
}

impl BlsAggregateResolveSelfContainedPayload {
    /// Strip to Phase 29 registry-oriented payload (no pubkeys).
    pub fn without_pubkeys(&self) -> BlsAggregateResolvePayload {
        BlsAggregateResolvePayload {
            appeal_id: self.appeal_id,
            approved: self.approved,
            signer_bitset: self.signer_bitset.clone(),
            aggregate_signature: self.aggregate_signature,
        }
    }
}

/// Build dual-key identities from pin-set + bitset + self-contained pubkeys.
pub fn phase30_identities_from_self_contained(
    pinned: &[AccountId],
    bitset: &[u8],
    pubkeys: &[BlsPublicKeyG1],
) -> Result<Vec<DualKeyIdentity>, Phase30BlsError> {
    let signers = phase29_decode_signer_bitset(pinned, bitset)?;
    if pubkeys.len() != signers.len() {
        return Err(Phase30BlsError::PubkeyCountMismatch {
            got: pubkeys.len(),
            expected: signers.len(),
        });
    }
    let identities: Vec<DualKeyIdentity> = signers
        .into_iter()
        .zip(pubkeys.iter().copied())
        .map(|(account_id, bls_pubkey)| DualKeyIdentity {
            account_id,
            bls_pubkey,
        })
        .collect();
    phase26_validate_dual_key_set(&identities)?;
    Ok(identities)
}

/// Verify self-contained aggregate resolve (no host dual-key registry).
pub fn phase30_verify_aggregate_resolve_self_contained(
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolveSelfContainedPayload,
) -> Result<(), Phase30BlsError> {
    let identities = phase30_identities_from_self_contained(
        pinned,
        &payload.signer_bitset,
        &payload.signer_bls_pubkeys,
    )?;
    let inner = payload.without_pubkeys();
    phase29_verify_aggregate_resolve(
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        &inner,
        &identities,
    )?;
    Ok(())
}

/// Domain-separated aggregate ballot message (same as Phase 26–29).
pub fn phase30_bls_aggregate_ballot_preimage(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    pin_set_commitment: &[u8; 32],
) -> [u8; 32] {
    phase29_bls_aggregate_ballot_preimage(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        pin_set_commitment,
    )
}

pub fn phase30_encode_signer_bitset(
    pinned: &[AccountId],
    signers: &[AccountId],
) -> Result<Vec<u8>, Phase30BlsError> {
    Ok(phase29_encode_signer_bitset(pinned, signers)?)
}

pub fn phase30_decode_signer_bitset(
    pinned: &[AccountId],
    bitset: &[u8],
) -> Result<Vec<AccountId>, Phase30BlsError> {
    Ok(phase29_decode_signer_bitset(pinned, bitset)?)
}

pub fn phase30_verify_aggregate_resolve_with_registry(
    registry: &crate::blobs_phase27::BlsDualKeyRegistry,
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolvePayload,
) -> Result<(), Phase30BlsError> {
    Ok(phase29_verify_aggregate_resolve_with_registry(
        registry,
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        payload,
    )?)
}

/// Consensus admit helper — always errors until operators opt in.
pub fn phase30_admit_bls_aggregate_resolve_tx() -> Result<(), Phase30BlsError> {
    if PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED {
        Ok(())
    } else {
        Err(Phase30BlsError::ConsensusSubmitNotWired)
    }
}

pub fn phase30_resolve_deadline(submit_height: u64) -> u64 {
    phase29_resolve_deadline(submit_height)
}

pub fn phase30_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase29_resolve_expired(tip_height, resolve_deadline)
}

pub fn phase30_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase29_expired_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase30_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase29_resolved_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase30_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase29_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase30_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase29_snapshot_electorate_w(pinned_stakes)
}

pub fn phase30_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase29_pin_set_commitment(pinned)
}

pub fn phase30_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase29_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase30_committee_requires_attested(pinned_len: usize) -> bool {
    phase29_committee_requires_attested(pinned_len)
}

pub fn phase30_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase29_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 30 self-contained pubkeys).
pub fn phase30_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase29_das_succinct_proof_plan();
    v.extend([
        "Phase 30: self-contained BLS G1 pubkey list on aggregate-resolve payload (library + dry-run); TransactionPayload::ResolveSlashAppealBlsAggregate (33) wired but not consensus-admitted.",
        "Phase 30: PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED=true; PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=false — verify without host dual-key registry; do not execute tx 33 yet.",
        "Phase 30: live ResolveSlashAppealAttested (32) stays Ed25519 verify-each — do not flip committee cast yet.",
        "Phase 30: remaining — consensus-admit/execute tx 33, flip cast when operators opt in, optional Ed25519 account-side binding, SNARK / 256 MiB.",
        "Phase 30: slash_appeals_registry.bin stays V5; bls_dual_key_registry.bin stays BDKR v1; do not clear.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 29 + Phase 30 ops).
pub fn phase30_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase29_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 30: self-contained signer_bls_pubkeys enable registry-free dry-run; do not admit consensus tx 33 yet.",
        "Phase 30: do not clear slash_appeals_registry.bin — stay V5; dual-keys remain host-local BDKR v1.",
        "Phase 30: live committee stays on ResolveSlashAppealAttested (32) Ed25519 verify-each.",
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
    fn self_contained_verify_roundtrip() {
        let ikm1 = [31u8; 32];
        let ikm2 = [32u8; 32];
        let pk1 = phase27_bls_pubkey_from_ikm(&ikm1).unwrap();
        let pk2 = phase27_bls_pubkey_from_ikm(&ikm2).unwrap();
        let a1 = AccountId([0x31; 32]);
        let a2 = AccountId([0x32; 32]);
        let pinned = [a1, a2];
        let pin_commit = phase30_pin_set_commitment(&pinned);
        let msg = phase30_bls_aggregate_ballot_preimage(6913, 11, 8, true, &pin_commit);
        let s1 = phase27_bls_sign(&ikm1, &msg).unwrap();
        let s2 = phase27_bls_sign(&ikm2, &msg).unwrap();
        let agg = phase27_aggregate_bls_signatures(&[s1, s2]).unwrap();
        let bits = phase30_encode_signer_bitset(&pinned, &[a1, a2]).unwrap();
        let payload = BlsAggregateResolveSelfContainedPayload {
            appeal_id: 11,
            approved: true,
            signer_bitset: bits,
            aggregate_signature: agg,
            signer_bls_pubkeys: vec![pk1, pk2],
        };
        assert!(phase30_verify_aggregate_resolve_self_contained(
            6913, 8, &pin_commit, &pinned, &payload
        )
        .is_ok());
        // Wrong pubkey count.
        let mut bad = payload.clone();
        bad.signer_bls_pubkeys.pop();
        assert!(matches!(
            phase30_verify_aggregate_resolve_self_contained(
                6913, 8, &pin_commit, &pinned, &bad
            ),
            Err(Phase30BlsError::PubkeyCountMismatch { .. })
        ));
        assert_eq!(
            phase30_admit_bls_aggregate_resolve_tx(),
            Err(Phase30BlsError::ConsensusSubmitNotWired)
        );
        assert_eq!(PHASE30_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX, 33);
        assert!(PHASE30_BLS_AGGREGATE_SELF_CONTAINED_WIRED);
        assert!(!PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED);
    }

    #[test]
    fn plan_mentions_self_contained_and_tx32() {
        let plan = phase30_das_succinct_proof_plan();
        assert!(plan
            .iter()
            .any(|s| s.contains("self-contained") || s.contains("pubkey")));
        assert!(plan.iter().any(|s| s.contains("verify-each")));
        assert!(plan
            .iter()
            .any(|s| s.contains("CONSENSUS_WIRED=false") || s.contains("not consensus")));
    }
}
