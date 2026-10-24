//! Phase 26: True BLS aggregate scaffolding (dual-key registration + `blst` plan).
//!
//! Phase 18–25 resolve ballots are **Ed25519 verify-each** (thin attest). This phase
//! starts the dual-key path: each resolver keeps `AccountId` (Ed25519) and may later
//! register a BLS12-381 G1 public key so committee resolves can carry a single G2
//! aggregate verified via `blst`.
//!
//! Honest scope for this PR (smallest coherent slice):
//! - Types + sizes + domain tags for dual-key identity and aggregate signatures.
//! - Aggregate / verify APIs return [`BlsAggregateError::NotWired`] — **no fake verify**.
//! - Do **not** add a `blst` crate dependency yet (Fly/build + linker follow-up).
//! - Do **not** change live **31**/**32** Ed25519 verify-each path.
//! - Registry stays **V5**; **do not clear** `slash_appeals_registry.bin`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - ReservedRealV1 verify still rejects; SNARK / 256 MiB still later.
//!
//! Remaining after this scaffold (document in plan strings):
//! 1. Optional Cargo feature + `blst` (or equivalent) for real aggregate prove/verify.
//! 2. Dual-key persistence (likely registry **V6**) + registration tx / host config.
//! 3. Aggregate submit path (new payload or extended **32**) + bitset of signers.
//! 4. Flip committee verify from verify-each to aggregate when `PHASE26_BLS_AGGREGATE_WIRED`.

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase25::{
    phase25_cap_activation_checklist, phase25_committee_requires_attested,
    phase25_das_succinct_proof_plan, phase25_expired_appeal_prunable,
    phase25_is_legacy_electorate_snapshot, phase25_pin_set_commitment, phase25_resolve_deadline,
    phase25_resolve_expired, phase25_resolve_threshold_met, phase25_resolved_appeal_prunable,
    phase25_snapshot_electorate_w, phase25_terminal_serve_challenge_prunable,
    PHASE25_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE25_CAP_ACTIVATION_ENV,
    PHASE25_EXPIRED_APPEAL_RETENTION_BLOCKS, PHASE25_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE25_LEGACY_ELECTORATE_W_SENTINEL, PHASE25_LEGACY_PIN_SET_COMMITMENT,
    PHASE25_MAX_APPEAL_EVIDENCE_BYTES, PHASE25_MAX_SETTLE_RECEIPTS, PHASE25_PIN_SET_DOMAIN,
    PHASE25_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE25_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE25_RESOLVE_THRESHOLD_DEN, PHASE25_RESOLVE_THRESHOLD_NUM, PHASE25_RESOLVE_WINDOW_BLOCKS,
    PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS, PHASE25_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE25_SETTLE_RECEIPT_TTL_BLOCKS, PHASE25_SUBMIT_SLASH_APPEAL_TX_INDEX,
    PHASE25_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::hash::hasher;
use crate::types::AccountId;
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE26_CAP_ACTIVATION_ENV: &str = PHASE25_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE26_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE25_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

pub const PHASE26_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE25_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
pub const PHASE26_RESOLVE_WINDOW_BLOCKS: u64 = PHASE25_RESOLVE_WINDOW_BLOCKS;
pub const PHASE26_MAX_SETTLE_RECEIPTS: usize = PHASE25_MAX_SETTLE_RECEIPTS;
pub const PHASE26_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE25_SERVE_CHALLENGE_WINDOW_BLOCKS;
pub const PHASE26_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE25_SETTLE_RECEIPT_TTL_BLOCKS;
pub const PHASE26_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE25_EXPIRED_APPEAL_RETENTION_BLOCKS;
pub const PHASE26_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE25_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
pub const PHASE26_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS;
pub const PHASE26_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE25_MAX_APPEAL_EVIDENCE_BYTES;
pub const PHASE26_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE25_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE26_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE25_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE26_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE25_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
pub const PHASE26_RESOLVE_THRESHOLD_NUM: u64 = PHASE25_RESOLVE_THRESHOLD_NUM;
pub const PHASE26_RESOLVE_THRESHOLD_DEN: u64 = PHASE25_RESOLVE_THRESHOLD_DEN;
pub const PHASE26_PIN_SET_DOMAIN: &[u8] = PHASE25_PIN_SET_DOMAIN;
pub const PHASE26_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE25_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE26_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE25_LEGACY_PIN_SET_COMMITMENT;

/// Compressed BLS12-381 G1 public key size (bytes).
pub const PHASE26_BLS_PUBLIC_KEY_G1_BYTES: usize = 48;

/// Compressed BLS12-381 G2 signature / aggregate size (bytes).
pub const PHASE26_BLS_SIGNATURE_G2_BYTES: usize = 96;

/// Domain tag for future BLS aggregate ballot messages (not yet verified).
pub const PHASE26_BLS_AGGREGATE_BALLOT_DOMAIN: &[u8] = b"BOING_SLASH_APPEAL_BLS_AGG_V1";

/// Flip to `true` only when a real `blst` (or equivalent) verify path is wired.
///
/// This scaffold keeps the flag **false** so operators never confuse types with live crypto.
pub const PHASE26_BLS_AGGREGATE_WIRED: bool = false;

/// Errors from Phase 26 BLS scaffold APIs.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BlsAggregateError {
    #[error("BLS aggregate crypto not wired (PHASE26_BLS_AGGREGATE_WIRED=false); Ed25519 verify-each on tx 32 remains live")]
    NotWired,
    #[error("invalid BLS public key length: expected {expected}, got {got}")]
    InvalidPublicKeyLength { expected: usize, got: usize },
    #[error("invalid BLS signature length: expected {expected}, got {got}")]
    InvalidSignatureLength { expected: usize, got: usize },
    #[error("empty signer set")]
    EmptySignerSet,
    #[error("duplicate AccountId in dual-key registration set")]
    DuplicateAccount,
}

/// Compressed BLS12-381 G1 public key (scaffold type; not curve-validated until wired).
///
/// Serde as a fixed tuple (same pattern as [`crate::SlashAppealBallotSignature`]) because
/// default serde does not implement `[u8; 48]`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BlsPublicKeyG1(pub [u8; PHASE26_BLS_PUBLIC_KEY_G1_BYTES]);

impl BlsPublicKeyG1 {
    pub fn from_bytes(bytes: [u8; PHASE26_BLS_PUBLIC_KEY_G1_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; PHASE26_BLS_PUBLIC_KEY_G1_BYTES] {
        &self.0
    }

    /// Length-check constructor from a slice (no curve validation until wired).
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, BlsAggregateError> {
        if bytes.len() != PHASE26_BLS_PUBLIC_KEY_G1_BYTES {
            return Err(BlsAggregateError::InvalidPublicKeyLength {
                expected: PHASE26_BLS_PUBLIC_KEY_G1_BYTES,
                got: bytes.len(),
            });
        }
        let mut arr = [0u8; PHASE26_BLS_PUBLIC_KEY_G1_BYTES];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }
}

impl fmt::Debug for BlsPublicKeyG1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlsPublicKeyG1({}…)", hex::encode(&self.0[..8]))
    }
}

impl Serialize for BlsPublicKeyG1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(PHASE26_BLS_PUBLIC_KEY_G1_BYTES)?;
        for b in &self.0 {
            tup.serialize_element(b)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for BlsPublicKeyG1 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = BlsPublicKeyG1;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{} BLS G1 public key bytes", PHASE26_BLS_PUBLIC_KEY_G1_BYTES)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut bytes = [0u8; PHASE26_BLS_PUBLIC_KEY_G1_BYTES];
                for (i, slot) in bytes.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(
                        PHASE26_BLS_PUBLIC_KEY_G1_BYTES + 1,
                        &self,
                    ));
                }
                Ok(BlsPublicKeyG1(bytes))
            }
        }
        deserializer.deserialize_tuple(PHASE26_BLS_PUBLIC_KEY_G1_BYTES, V)
    }
}

/// Compressed BLS12-381 G2 signature / aggregate (scaffold type).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BlsSignatureG2(pub [u8; PHASE26_BLS_SIGNATURE_G2_BYTES]);

impl BlsSignatureG2 {
    pub fn from_bytes(bytes: [u8; PHASE26_BLS_SIGNATURE_G2_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; PHASE26_BLS_SIGNATURE_G2_BYTES] {
        &self.0
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, BlsAggregateError> {
        if bytes.len() != PHASE26_BLS_SIGNATURE_G2_BYTES {
            return Err(BlsAggregateError::InvalidSignatureLength {
                expected: PHASE26_BLS_SIGNATURE_G2_BYTES,
                got: bytes.len(),
            });
        }
        let mut arr = [0u8; PHASE26_BLS_SIGNATURE_G2_BYTES];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }
}

impl fmt::Debug for BlsSignatureG2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlsSignatureG2({}…)", hex::encode(&self.0[..8]))
    }
}

impl Serialize for BlsSignatureG2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(PHASE26_BLS_SIGNATURE_G2_BYTES)?;
        for b in &self.0 {
            tup.serialize_element(b)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for BlsSignatureG2 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = BlsSignatureG2;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{} BLS G2 signature bytes", PHASE26_BLS_SIGNATURE_G2_BYTES)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut bytes = [0u8; PHASE26_BLS_SIGNATURE_G2_BYTES];
                for (i, slot) in bytes.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(
                        PHASE26_BLS_SIGNATURE_G2_BYTES + 1,
                        &self,
                    ));
                }
                Ok(BlsSignatureG2(bytes))
            }
        }
        deserializer.deserialize_tuple(PHASE26_BLS_SIGNATURE_G2_BYTES, V)
    }
}

/// Dual-key identity: consensus `AccountId` (Ed25519) + optional BLS G1 pubkey.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DualKeyIdentity {
    pub account_id: AccountId,
    pub bls_pubkey: BlsPublicKeyG1,
}

/// Domain-separated message hash for a future BLS aggregate ballot.
///
/// Same logical fields as Ed25519 V2 ballots (chain, appeal, slash, approved, pin commitment)
/// under [`PHASE26_BLS_AGGREGATE_BALLOT_DOMAIN`]. Not signed/verified until wired.
pub fn phase26_bls_aggregate_ballot_preimage(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    pin_set_commitment: &[u8; 32],
) -> [u8; 32] {
    let mut h = hasher();
    h.update(PHASE26_BLS_AGGREGATE_BALLOT_DOMAIN);
    h.update(&chain_id.to_le_bytes());
    h.update(&appeal_id.to_le_bytes());
    h.update(&slash_id.to_le_bytes());
    h.update(&[u8::from(approved)]);
    h.update(pin_set_commitment);
    *h.finalize().as_bytes()
}

/// Validate a dual-key registration set (unique AccountIds + key lengths).
///
/// Does **not** curve-validate pubkeys and does **not** persist — scaffold only.
pub fn phase26_validate_dual_key_set(
    identities: &[DualKeyIdentity],
) -> Result<(), BlsAggregateError> {
    if identities.is_empty() {
        return Err(BlsAggregateError::EmptySignerSet);
    }
    let mut seen = std::collections::BTreeSet::new();
    for id in identities {
        if !seen.insert(id.account_id.0) {
            return Err(BlsAggregateError::DuplicateAccount);
        }
        // Length already enforced by type; re-check for API symmetry with slice helpers.
        if id.bls_pubkey.0.len() != PHASE26_BLS_PUBLIC_KEY_G1_BYTES {
            return Err(BlsAggregateError::InvalidPublicKeyLength {
                expected: PHASE26_BLS_PUBLIC_KEY_G1_BYTES,
                got: id.bls_pubkey.0.len(),
            });
        }
    }
    Ok(())
}

/// Aggregate individual BLS signatures — **not wired** in this scaffold.
pub fn phase26_aggregate_bls_signatures(
    _signatures: &[BlsSignatureG2],
) -> Result<BlsSignatureG2, BlsAggregateError> {
    Err(BlsAggregateError::NotWired)
}

/// Verify a BLS aggregate against a dual-key set — **not wired** in this scaffold.
pub fn phase26_verify_bls_aggregate(
    _message: &[u8; 32],
    _aggregate: &BlsSignatureG2,
    _identities: &[DualKeyIdentity],
) -> Result<(), BlsAggregateError> {
    Err(BlsAggregateError::NotWired)
}

pub fn phase26_resolve_deadline(submit_height: u64) -> u64 {
    phase25_resolve_deadline(submit_height)
}

pub fn phase26_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase25_resolve_expired(tip_height, resolve_deadline)
}

pub fn phase26_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase25_expired_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase26_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase25_resolved_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase26_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase25_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase26_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase25_snapshot_electorate_w(pinned_stakes)
}

pub fn phase26_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase25_pin_set_commitment(pinned)
}

pub fn phase26_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase25_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase26_committee_requires_attested(pinned_len: usize) -> bool {
    phase25_committee_requires_attested(pinned_len)
}

pub fn phase26_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase25_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 26 BLS dual-key scaffold).
pub fn phase26_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase25_das_succinct_proof_plan();
    v.extend([
        "Phase 26: scaffold dual-key BLS aggregate types (BLS12-381 G1 pubkey 48 B + G2 sig 96 B) and NotWired aggregate/verify APIs — do not fake crypto.",
        "Phase 26: live ResolveSlashAppealAttested (32) stays Ed25519 verify-each; PHASE26_BLS_AGGREGATE_WIRED=false until blst is wired.",
        "Phase 26: remaining — optional blst feature, dual-key persistence (likely V6), registration tx/host config, aggregate submit path, flip verify when wired.",
        "Phase 26: registry stays V5 (untouched); do not clear; txs 30/31/32 unchanged; Phase 19–25 policies unchanged.",
        "Phase 26: SNARK/IPA/KZG and live 256 MiB remain later — keep Fly-safe 16 MiB.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 25 + Phase 26 ops).
pub fn phase26_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase25_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 26: do not clear slash_appeals_registry.bin — stay V5; dual-key persistence is a later V6 follow-up.",
        "Phase 26: do not enable PHASE26_BLS_AGGREGATE_WIRED or depend on blst until a coordinated crypto PR lands.",
        "Phase 26: committee nets continue Ed25519 verify-each on tx 32; aggregate path is scaffold-only.",
        "Phase 26: true SNARK / 256 MiB still later — this phase only scaffolds dual-key BLS types + NotWired APIs.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_sizes_and_wired_flag() {
        assert_eq!(PHASE26_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE26_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE26_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE26_BLS_PUBLIC_KEY_G1_BYTES, 48);
        assert_eq!(PHASE26_BLS_SIGNATURE_G2_BYTES, 96);
        assert!(!PHASE26_BLS_AGGREGATE_WIRED);
        assert_eq!(
            PHASE26_RESOLVED_APPEAL_RETENTION_BLOCKS,
            PHASE25_RESOLVED_APPEAL_RETENTION_BLOCKS
        );
    }

    #[test]
    fn dual_key_validate_and_not_wired() {
        let a = DualKeyIdentity {
            account_id: AccountId([1u8; 32]),
            bls_pubkey: BlsPublicKeyG1::from_bytes([2u8; 48]),
        };
        let b = DualKeyIdentity {
            account_id: AccountId([3u8; 32]),
            bls_pubkey: BlsPublicKeyG1::from_bytes([4u8; 48]),
        };
        assert!(phase26_validate_dual_key_set(&[a.clone(), b]).is_ok());
        assert_eq!(
            phase26_validate_dual_key_set(&[a.clone(), a.clone()]),
            Err(BlsAggregateError::DuplicateAccount)
        );
        assert_eq!(
            phase26_validate_dual_key_set(&[]),
            Err(BlsAggregateError::EmptySignerSet)
        );
        assert_eq!(
            phase26_aggregate_bls_signatures(&[]),
            Err(BlsAggregateError::NotWired)
        );
        let msg = phase26_bls_aggregate_ballot_preimage(6913, 1, 7, true, &[9u8; 32]);
        assert_eq!(
            phase26_verify_bls_aggregate(&msg, &BlsSignatureG2::from_bytes([0u8; 96]), &[a]),
            Err(BlsAggregateError::NotWired)
        );
        assert!(BlsPublicKeyG1::try_from_slice(&[0u8; 47]).is_err());
        assert!(BlsSignatureG2::try_from_slice(&[0u8; 95]).is_err());
    }

    #[test]
    fn plan_mentions_scaffold_and_remaining() {
        let plan = phase26_das_succinct_proof_plan();
        assert!(plan.iter().any(|s| s.contains("NotWired") || s.contains("scaffold")));
        assert!(plan.iter().any(|s| s.contains("blst") || s.contains("V6")));
        assert!(plan.iter().any(|s| s.contains("verify-each")));
        let checklist = phase26_cap_activation_checklist(Some(1200));
        assert!(checklist
            .iter()
            .any(|s| s.contains("slash_appeals_registry.bin") && s.contains("V5")));
    }
}
