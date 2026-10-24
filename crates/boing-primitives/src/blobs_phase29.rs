//! Phase 29: BLS aggregate resolve payload + pin-set signer bitset.
//!
//! Builds on Phase 27–28 (`blst` wire, dual-key registry, PoP). Smallest coherent slice:
//! - Reserved payload index **33** for future `ResolveSlashAppealBlsAggregate`.
//! - Signer bitset over Submit-frozen pin-set order (LSB = `pinned[0]`).
//! - Library encode/decode + verify of an aggregate resolve ballot against dual-keys.
//! - Dry-run RPC path (node) — **consensus admit of tx 33 is NOT wired**
//!   (dual-keys remain host-local / not in state-root).
//! - Do **not** change live ResolveSlashAppealAttested (**32**) Ed25519 verify-each.
//! - Do **not** clear `slash_appeals_registry.bin` (stay V5); BDKR v1 unchanged.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - ReservedRealV1 verify still rejects; SNARK / 256 MiB still later.
//!
//! Remaining after this slice:
//! 1. Consensus-admit tx **33** (consensus-visible dual-keys or self-contained pubkey list).
//! 2. Flip committee cast from Ed25519 verify-each → BLS aggregate when operators opt in.
//! 3. Optional Ed25519 account-side binding on a consensus tx.

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase26::{
    phase26_bls_aggregate_ballot_preimage, BlsAggregateError, BlsSignatureG2, DualKeyIdentity,
};
use crate::blobs_phase27::{phase27_verify_bls_aggregate, BlsDualKeyRegistry, Phase27BlsError};
use crate::blobs_phase28::{
    phase28_cap_activation_checklist, phase28_committee_requires_attested,
    phase28_das_succinct_proof_plan, phase28_expired_appeal_prunable,
    phase28_is_legacy_electorate_snapshot, phase28_pin_set_commitment, phase28_resolve_deadline,
    phase28_resolve_expired, phase28_resolve_threshold_met, phase28_resolved_appeal_prunable,
    phase28_snapshot_electorate_w, phase28_terminal_serve_challenge_prunable,
    phase28_validate_dual_key_set, PHASE28_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
    PHASE28_BLS_AGGREGATE_BALLOT_DOMAIN, PHASE28_BLS_AGGREGATE_WIRED,
    PHASE28_BLS_DUAL_KEY_REGISTRY_FILE, PHASE28_BLS_DUAL_KEY_REGISTRY_MAGIC,
    PHASE28_BLS_DUAL_KEY_REGISTRY_VERSION, PHASE28_BLS_POP_REQUIRED,
    PHASE28_BLS_PUBLIC_KEY_G1_BYTES, PHASE28_BLS_SIGNATURE_G2_BYTES, PHASE28_CAP_ACTIVATION_ENV,
    PHASE28_EXPIRED_APPEAL_RETENTION_BLOCKS, PHASE28_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE28_LEGACY_ELECTORATE_W_SENTINEL, PHASE28_LEGACY_PIN_SET_COMMITMENT,
    PHASE28_MAX_APPEAL_EVIDENCE_BYTES, PHASE28_MAX_SETTLE_RECEIPTS, PHASE28_PIN_SET_DOMAIN,
    PHASE28_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE28_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE28_RESOLVE_THRESHOLD_DEN, PHASE28_RESOLVE_THRESHOLD_NUM, PHASE28_RESOLVE_WINDOW_BLOCKS,
    PHASE28_RESOLVED_APPEAL_RETENTION_BLOCKS, PHASE28_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE28_SETTLE_RECEIPT_TTL_BLOCKS, PHASE28_SUBMIT_SLASH_APPEAL_TX_INDEX,
    PHASE28_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::types::AccountId;
use serde::{Deserialize, Serialize};

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE29_CAP_ACTIVATION_ENV: &str = PHASE28_CAP_ACTIVATION_ENV;

pub const PHASE29_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE28_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
pub const PHASE29_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE28_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
pub const PHASE29_RESOLVE_WINDOW_BLOCKS: u64 = PHASE28_RESOLVE_WINDOW_BLOCKS;
pub const PHASE29_MAX_SETTLE_RECEIPTS: usize = PHASE28_MAX_SETTLE_RECEIPTS;
pub const PHASE29_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE28_SERVE_CHALLENGE_WINDOW_BLOCKS;
pub const PHASE29_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE28_SETTLE_RECEIPT_TTL_BLOCKS;
pub const PHASE29_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE28_EXPIRED_APPEAL_RETENTION_BLOCKS;
pub const PHASE29_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE28_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
pub const PHASE29_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = PHASE28_RESOLVED_APPEAL_RETENTION_BLOCKS;
pub const PHASE29_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE28_MAX_APPEAL_EVIDENCE_BYTES;
pub const PHASE29_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE28_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE29_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE28_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE29_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE28_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
pub const PHASE29_RESOLVE_THRESHOLD_NUM: u64 = PHASE28_RESOLVE_THRESHOLD_NUM;
pub const PHASE29_RESOLVE_THRESHOLD_DEN: u64 = PHASE28_RESOLVE_THRESHOLD_DEN;
pub const PHASE29_PIN_SET_DOMAIN: &[u8] = PHASE28_PIN_SET_DOMAIN;
pub const PHASE29_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE28_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE29_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE28_LEGACY_PIN_SET_COMMITMENT;

pub const PHASE29_BLS_PUBLIC_KEY_G1_BYTES: usize = PHASE28_BLS_PUBLIC_KEY_G1_BYTES;
pub const PHASE29_BLS_SIGNATURE_G2_BYTES: usize = PHASE28_BLS_SIGNATURE_G2_BYTES;
pub const PHASE29_BLS_AGGREGATE_BALLOT_DOMAIN: &[u8] = PHASE28_BLS_AGGREGATE_BALLOT_DOMAIN;
pub const PHASE29_BLS_AGGREGATE_WIRED: bool = PHASE28_BLS_AGGREGATE_WIRED;
pub const PHASE29_BLS_DUAL_KEY_REGISTRY_FILE: &str = PHASE28_BLS_DUAL_KEY_REGISTRY_FILE;
pub const PHASE29_BLS_DUAL_KEY_REGISTRY_MAGIC: &[u8; 4] = PHASE28_BLS_DUAL_KEY_REGISTRY_MAGIC;
pub const PHASE29_BLS_DUAL_KEY_REGISTRY_VERSION: u32 = PHASE28_BLS_DUAL_KEY_REGISTRY_VERSION;
pub const PHASE29_BLS_POP_REQUIRED: bool = PHASE28_BLS_POP_REQUIRED;

/// Reserved bincode / payload index for future `ResolveSlashAppealBlsAggregate`.
///
/// **Not** admitted by consensus execution in this phase — dual-keys are still host-local.
pub const PHASE29_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX: u32 = 33;

/// Soft cap on pin-set size for bitset helpers (matches stake-validator top-N default headroom).
pub const PHASE29_MAX_PIN_SET_LEN: usize = 64;

/// Library + RPC aggregate-resolve helpers are available.
/// Consensus admit of tx **33** stays **false** until dual-keys are consensus-visible.
pub const PHASE29_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED: bool = true;
pub const PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED: bool = false;

/// Errors for Phase 29 aggregate-resolve helpers.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Phase29BlsError {
    #[error(transparent)]
    Inner(#[from] Phase27BlsError),
    #[error(transparent)]
    Aggregate(#[from] BlsAggregateError),
    #[error("pin set empty")]
    EmptyPinSet,
    #[error("pin set too large: {got} > {max}")]
    PinSetTooLarge { got: usize, max: usize },
    #[error("signer bitset empty")]
    EmptyBitset,
    #[error("signer bitset longer than pin set")]
    BitsetLongerThanPinSet,
    #[error("signer bitset has bits set beyond pin set length")]
    BitOutOfRange,
    #[error("account not in pin set")]
    AccountNotInPinSet,
    #[error("duplicate signer index in bitset decode")]
    DuplicateSigner,
    #[error("BLS aggregate resolve consensus admit not wired (PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=false); use ResolveSlashAppealAttested (32)")]
    ConsensusSubmitNotWired,
}

/// Aggregate resolve ballot payload (library / future tx **33** body).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlsAggregateResolvePayload {
    pub appeal_id: u64,
    pub approved: bool,
    /// Bitset over Submit-frozen pin-set order (LSB of byte 0 = `pinned[0]`).
    pub signer_bitset: Vec<u8>,
    pub aggregate_signature: BlsSignatureG2,
}

/// Encode a pin-set-relative signer bitset from AccountIds (must be subset of `pinned`).
pub fn phase29_encode_signer_bitset(
    pinned: &[AccountId],
    signers: &[AccountId],
) -> Result<Vec<u8>, Phase29BlsError> {
    if pinned.is_empty() {
        return Err(Phase29BlsError::EmptyPinSet);
    }
    if pinned.len() > PHASE29_MAX_PIN_SET_LEN {
        return Err(Phase29BlsError::PinSetTooLarge {
            got: pinned.len(),
            max: PHASE29_MAX_PIN_SET_LEN,
        });
    }
    if signers.is_empty() {
        return Err(Phase29BlsError::EmptyBitset);
    }
    let nbytes = pinned.len().div_ceil(8);
    let mut bits = vec![0u8; nbytes];
    let mut seen = std::collections::BTreeSet::new();
    for s in signers {
        let idx = pinned
            .iter()
            .position(|p| p == s)
            .ok_or(Phase29BlsError::AccountNotInPinSet)?;
        if !seen.insert(idx) {
            return Err(Phase29BlsError::DuplicateSigner);
        }
        bits[idx / 8] |= 1u8 << (idx % 8);
    }
    Ok(bits)
}

/// Decode bitset → pin-set AccountIds (order = ascending pin index).
pub fn phase29_decode_signer_bitset(
    pinned: &[AccountId],
    bitset: &[u8],
) -> Result<Vec<AccountId>, Phase29BlsError> {
    if pinned.is_empty() {
        return Err(Phase29BlsError::EmptyPinSet);
    }
    if pinned.len() > PHASE29_MAX_PIN_SET_LEN {
        return Err(Phase29BlsError::PinSetTooLarge {
            got: pinned.len(),
            max: PHASE29_MAX_PIN_SET_LEN,
        });
    }
    let need = pinned.len().div_ceil(8);
    if bitset.len() > need {
        return Err(Phase29BlsError::BitsetLongerThanPinSet);
    }
    let mut out = Vec::new();
    for (byte_i, byte) in bitset.iter().enumerate() {
        for bit in 0..8 {
            if byte & (1u8 << bit) == 0 {
                continue;
            }
            let idx = byte_i * 8 + bit;
            if idx >= pinned.len() {
                return Err(Phase29BlsError::BitOutOfRange);
            }
            out.push(pinned[idx]);
        }
    }
    if out.is_empty() {
        return Err(Phase29BlsError::EmptyBitset);
    }
    Ok(out)
}

/// Domain-separated aggregate ballot message (same as Phase 26/27).
pub fn phase29_bls_aggregate_ballot_preimage(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    pin_set_commitment: &[u8; 32],
) -> [u8; 32] {
    phase26_bls_aggregate_ballot_preimage(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        pin_set_commitment,
    )
}

/// Verify an aggregate resolve payload against a pin set + dual-key identities.
pub fn phase29_verify_aggregate_resolve(
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolvePayload,
    identities: &[DualKeyIdentity],
) -> Result<(), Phase29BlsError> {
    let signers = phase29_decode_signer_bitset(pinned, &payload.signer_bitset)?;
    // Identities must match decoded signers 1:1 in the same order.
    if identities.len() != signers.len() {
        return Err(BlsAggregateError::EmptySignerSet.into());
    }
    for (id, acct) in identities.iter().zip(signers.iter()) {
        if &id.account_id != acct {
            return Err(Phase29BlsError::AccountNotInPinSet);
        }
    }
    phase28_validate_dual_key_set(identities)?;
    let msg = phase29_bls_aggregate_ballot_preimage(
        chain_id,
        payload.appeal_id,
        slash_id,
        payload.approved,
        pin_set_commitment,
    );
    phase27_verify_bls_aggregate(&msg, &payload.aggregate_signature, identities)?;
    Ok(())
}

/// Resolve identities from host dual-key registry for a bitset over `pinned`.
pub fn phase29_resolve_identities_for_bitset(
    registry: &BlsDualKeyRegistry,
    pinned: &[AccountId],
    bitset: &[u8],
) -> Result<Vec<DualKeyIdentity>, Phase29BlsError> {
    let signers = phase29_decode_signer_bitset(pinned, bitset)?;
    Ok(registry.resolve_identities(&signers)?)
}

/// Verify payload using host-local dual-key registry.
pub fn phase29_verify_aggregate_resolve_with_registry(
    registry: &BlsDualKeyRegistry,
    chain_id: u64,
    slash_id: u64,
    pin_set_commitment: &[u8; 32],
    pinned: &[AccountId],
    payload: &BlsAggregateResolvePayload,
) -> Result<(), Phase29BlsError> {
    let identities =
        phase29_resolve_identities_for_bitset(registry, pinned, &payload.signer_bitset)?;
    phase29_verify_aggregate_resolve(
        chain_id,
        slash_id,
        pin_set_commitment,
        pinned,
        payload,
        &identities,
    )
}

/// Consensus admit helper — always errors until dual-keys are consensus-visible.
pub fn phase29_admit_bls_aggregate_resolve_tx() -> Result<(), Phase29BlsError> {
    if PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED {
        Ok(())
    } else {
        Err(Phase29BlsError::ConsensusSubmitNotWired)
    }
}

pub fn phase29_resolve_deadline(submit_height: u64) -> u64 {
    phase28_resolve_deadline(submit_height)
}

pub fn phase29_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase28_resolve_expired(tip_height, resolve_deadline)
}

pub fn phase29_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase28_expired_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase29_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase28_resolved_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase29_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase28_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase29_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase28_snapshot_electorate_w(pinned_stakes)
}

pub fn phase29_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase28_pin_set_commitment(pinned)
}

pub fn phase29_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase28_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase29_committee_requires_attested(pinned_len: usize) -> bool {
    phase28_committee_requires_attested(pinned_len)
}

pub fn phase29_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase28_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 29 aggregate submit scaffold).
pub fn phase29_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase28_das_succinct_proof_plan();
    v.extend([
        "Phase 29: BLS aggregate resolve payload + pin-set signer bitset (library + dry-run RPC); reserved tx index 33.",
        "Phase 29: PHASE29_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED=true; PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=false — dual-keys still host-local.",
        "Phase 29: live ResolveSlashAppealAttested (32) stays Ed25519 verify-each — do not flip committee cast yet.",
        "Phase 29: remaining — consensus-admit tx 33 (consensus-visible dual-keys or self-contained pubkeys), flip cast when operators opt in, SNARK / 256 MiB.",
        "Phase 29: slash_appeals_registry.bin stays V5; bls_dual_key_registry.bin stays BDKR v1; do not clear.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 28 + Phase 29 ops).
pub fn phase29_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase28_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 29: do not clear slash_appeals_registry.bin — stay V5; dual-keys remain host-local BDKR v1.",
        "Phase 29: do not admit consensus tx 33 yet — use library/RPC dry-run; live committee stays on tx 32.",
        "Phase 29: signer bitset is pin-set relative (LSB = pinned[0]); verify via boing_verifyBlsAggregateResolve.",
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
    fn bitset_roundtrip() {
        let pinned = [
            AccountId([1u8; 32]),
            AccountId([2u8; 32]),
            AccountId([3u8; 32]),
        ];
        let bits = phase29_encode_signer_bitset(&pinned, &[pinned[2], pinned[0]]).unwrap();
        let decoded = phase29_decode_signer_bitset(&pinned, &bits).unwrap();
        // Ascending pin index order.
        assert_eq!(decoded, vec![pinned[0], pinned[2]]);
        assert!(phase29_encode_signer_bitset(&pinned, &[AccountId([9u8; 32])]).is_err());
        assert!(phase29_decode_signer_bitset(&pinned, &[]).is_err());
    }

    #[test]
    fn verify_aggregate_resolve_roundtrip() {
        let ikm1 = [21u8; 32];
        let ikm2 = [22u8; 32];
        let pk1 = phase27_bls_pubkey_from_ikm(&ikm1).unwrap();
        let pk2 = phase27_bls_pubkey_from_ikm(&ikm2).unwrap();
        let a1 = AccountId([0x11; 32]);
        let a2 = AccountId([0x22; 32]);
        let pinned = [a1, a2];
        let pin_commit = phase29_pin_set_commitment(&pinned);
        let mut reg = BlsDualKeyRegistry::new();
        reg.register(a1, pk1).unwrap();
        reg.register(a2, pk2).unwrap();
        let msg = phase29_bls_aggregate_ballot_preimage(6913, 9, 7, true, &pin_commit);
        let s1 = phase27_bls_sign(&ikm1, &msg).unwrap();
        let s2 = phase27_bls_sign(&ikm2, &msg).unwrap();
        let agg = phase27_aggregate_bls_signatures(&[s1, s2]).unwrap();
        let bits = phase29_encode_signer_bitset(&pinned, &[a1, a2]).unwrap();
        let payload = BlsAggregateResolvePayload {
            appeal_id: 9,
            approved: true,
            signer_bitset: bits,
            aggregate_signature: agg,
        };
        assert!(phase29_verify_aggregate_resolve_with_registry(
            &reg, 6913, 7, &pin_commit, &pinned, &payload
        )
        .is_ok());
        assert_eq!(
            phase29_admit_bls_aggregate_resolve_tx(),
            Err(Phase29BlsError::ConsensusSubmitNotWired)
        );
        assert_eq!(PHASE29_RESOLVE_SLASH_APPEAL_BLS_AGGREGATE_TX_INDEX, 33);
        assert!(PHASE29_BLS_AGGREGATE_SUBMIT_LIBRARY_WIRED);
        assert!(!PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED);
    }

    #[test]
    fn plan_mentions_bitset_and_tx32() {
        let plan = phase29_das_succinct_proof_plan();
        assert!(plan.iter().any(|s| s.contains("bitset") || s.contains("33")));
        assert!(plan.iter().any(|s| s.contains("verify-each")));
        assert!(plan
            .iter()
            .any(|s| s.contains("CONSENSUS_WIRED=false") || s.contains("host-local")));
    }
}
