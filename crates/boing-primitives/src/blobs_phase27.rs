//! Phase 27: Wire `blst` (`min_pk`) + host-local dual-key registry + aggregate verify.
//!
//! Builds on Phase 26 scaffolding. Smallest coherent slice:
//! - Real BLS12-381 sign / aggregate / `fast_aggregate_verify` via `blst`.
//! - Host-local [`BlsDualKeyRegistry`] (separate file — **not** slash appeals V5 / not state-root).
//! - Library + RPC verify path; **live ResolveSlashAppealAttested (32) stays Ed25519 verify-each**.
//! - Do **not** clear `slash_appeals_registry.bin`; no V6 slash schema.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - ReservedRealV1 verify still rejects; SNARK / 256 MiB still later.
//!
//! Remaining after this slice:
//! 1. Proof-of-possession for dual-key registration.
//! 2. Aggregate submit path (new payload or extended **32**) + signer bitset.
//! 3. Flip committee cast from Ed25519 verify-each → BLS aggregate when operators opt in.

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase26::{
    phase26_bls_aggregate_ballot_preimage, phase26_cap_activation_checklist,
    phase26_committee_requires_attested, phase26_das_succinct_proof_plan,
    phase26_expired_appeal_prunable, phase26_is_legacy_electorate_snapshot,
    phase26_pin_set_commitment, phase26_resolve_deadline, phase26_resolve_expired,
    phase26_resolve_threshold_met, phase26_resolved_appeal_prunable,
    phase26_snapshot_electorate_w, phase26_terminal_serve_challenge_prunable,
    phase26_validate_dual_key_set, BlsAggregateError, BlsPublicKeyG1, BlsSignatureG2,
    DualKeyIdentity, PHASE26_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE26_BLS_AGGREGATE_BALLOT_DOMAIN,
    PHASE26_BLS_PUBLIC_KEY_G1_BYTES, PHASE26_BLS_SIGNATURE_G2_BYTES, PHASE26_CAP_ACTIVATION_ENV,
    PHASE26_EXPIRED_APPEAL_RETENTION_BLOCKS, PHASE26_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE26_LEGACY_ELECTORATE_W_SENTINEL, PHASE26_LEGACY_PIN_SET_COMMITMENT,
    PHASE26_MAX_APPEAL_EVIDENCE_BYTES, PHASE26_MAX_SETTLE_RECEIPTS, PHASE26_PIN_SET_DOMAIN,
    PHASE26_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE26_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE26_RESOLVE_THRESHOLD_DEN, PHASE26_RESOLVE_THRESHOLD_NUM, PHASE26_RESOLVE_WINDOW_BLOCKS,
    PHASE26_RESOLVED_APPEAL_RETENTION_BLOCKS, PHASE26_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE26_SETTLE_RECEIPT_TTL_BLOCKS, PHASE26_SUBMIT_SLASH_APPEAL_TX_INDEX,
    PHASE26_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::types::AccountId;
use blst::min_pk::{AggregateSignature, PublicKey, SecretKey, Signature};
use blst::BLST_ERROR;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE27_CAP_ACTIVATION_ENV: &str = PHASE26_CAP_ACTIVATION_ENV;

pub const PHASE27_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE26_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
pub const PHASE27_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE26_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
pub const PHASE27_RESOLVE_WINDOW_BLOCKS: u64 = PHASE26_RESOLVE_WINDOW_BLOCKS;
pub const PHASE27_MAX_SETTLE_RECEIPTS: usize = PHASE26_MAX_SETTLE_RECEIPTS;
pub const PHASE27_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE26_SERVE_CHALLENGE_WINDOW_BLOCKS;
pub const PHASE27_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE26_SETTLE_RECEIPT_TTL_BLOCKS;
pub const PHASE27_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE26_EXPIRED_APPEAL_RETENTION_BLOCKS;
pub const PHASE27_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE26_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
pub const PHASE27_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = PHASE26_RESOLVED_APPEAL_RETENTION_BLOCKS;
pub const PHASE27_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE26_MAX_APPEAL_EVIDENCE_BYTES;
pub const PHASE27_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE26_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE27_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE26_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE27_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE26_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
pub const PHASE27_RESOLVE_THRESHOLD_NUM: u64 = PHASE26_RESOLVE_THRESHOLD_NUM;
pub const PHASE27_RESOLVE_THRESHOLD_DEN: u64 = PHASE26_RESOLVE_THRESHOLD_DEN;
pub const PHASE27_PIN_SET_DOMAIN: &[u8] = PHASE26_PIN_SET_DOMAIN;
pub const PHASE27_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE26_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE27_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE26_LEGACY_PIN_SET_COMMITMENT;

pub const PHASE27_BLS_PUBLIC_KEY_G1_BYTES: usize = PHASE26_BLS_PUBLIC_KEY_G1_BYTES;
pub const PHASE27_BLS_SIGNATURE_G2_BYTES: usize = PHASE26_BLS_SIGNATURE_G2_BYTES;
pub const PHASE27_BLS_AGGREGATE_BALLOT_DOMAIN: &[u8] = PHASE26_BLS_AGGREGATE_BALLOT_DOMAIN;

/// CipherSuite DST for Boing BLS aggregate ballots (`blst` `min_pk` path).
pub const PHASE27_BLS_SIG_DST: &[u8] = b"BOING_BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

/// Host-local dual-key registry filename (under the node data directory).
pub const PHASE27_BLS_DUAL_KEY_REGISTRY_FILE: &str = "bls_dual_key_registry.bin";

/// On-disk magic for [`PHASE27_BLS_DUAL_KEY_REGISTRY_FILE`].
pub const PHASE27_BLS_DUAL_KEY_REGISTRY_MAGIC: &[u8; 4] = b"BDKR";

/// On-disk payload version for the dual-key registry.
pub const PHASE27_BLS_DUAL_KEY_REGISTRY_VERSION: u32 = 1;

/// Library + RPC aggregate verify is wired. Live tx **32** remains Ed25519 verify-each.
pub const PHASE27_BLS_AGGREGATE_WIRED: bool = true;

/// Extended errors for the wired `blst` path (includes Phase 26 variants).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Phase27BlsError {
    #[error(transparent)]
    Aggregate(#[from] BlsAggregateError),
    #[error("invalid BLS secret key")]
    InvalidSecretKey,
    #[error("invalid BLS public key (curve / subgroup)")]
    InvalidPublicKey,
    #[error("invalid BLS signature (curve / subgroup)")]
    InvalidSignature,
    #[error("BLS aggregate failed")]
    AggregateFailed,
    #[error("BLS aggregate verify failed")]
    VerifyFailed,
    #[error("account not registered in dual-key registry")]
    AccountNotRegistered,
    #[error("duplicate AccountId in dual-key registration set")]
    DuplicateAccount,
}

impl From<BLST_ERROR> for Phase27BlsError {
    fn from(err: BLST_ERROR) -> Self {
        match err {
            BLST_ERROR::BLST_SUCCESS => unreachable!("BLST_SUCCESS is not an error"),
            BLST_ERROR::BLST_BAD_ENCODING
            | BLST_ERROR::BLST_POINT_NOT_ON_CURVE
            | BLST_ERROR::BLST_POINT_NOT_IN_GROUP => Phase27BlsError::InvalidPublicKey,
            BLST_ERROR::BLST_AGGR_TYPE_MISMATCH | BLST_ERROR::BLST_PK_IS_INFINITY => {
                Phase27BlsError::AggregateFailed
            }
            BLST_ERROR::BLST_VERIFY_FAIL => Phase27BlsError::VerifyFailed,
            _ => Phase27BlsError::VerifyFailed,
        }
    }
}

/// Domain-separated message hash for a BLS aggregate ballot (same as Phase 26).
pub fn phase27_bls_aggregate_ballot_preimage(
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

/// Validate dual-key set (unique AccountIds + lengths). Curve validation is separate.
pub fn phase27_validate_dual_key_set(
    identities: &[DualKeyIdentity],
) -> Result<(), BlsAggregateError> {
    phase26_validate_dual_key_set(identities)
}

/// Parse + subgroup-check a compressed G1 public key.
pub fn phase27_parse_bls_pubkey(bytes: &[u8]) -> Result<BlsPublicKeyG1, Phase27BlsError> {
    let key = BlsPublicKeyG1::try_from_slice(bytes)?;
    let pk = PublicKey::from_bytes(key.as_bytes()).map_err(Phase27BlsError::from)?;
    if pk.validate().is_err() {
        return Err(Phase27BlsError::InvalidPublicKey);
    }
    Ok(key)
}

/// Derive a compressed G1 pubkey from a 32-byte IKM via `SecretKey::key_gen`.
pub fn phase27_bls_pubkey_from_ikm(ikm: &[u8; 32]) -> Result<BlsPublicKeyG1, Phase27BlsError> {
    let sk = SecretKey::key_gen(ikm, &[]).map_err(|_| Phase27BlsError::InvalidSecretKey)?;
    let pk = sk.sk_to_pk();
    Ok(BlsPublicKeyG1::from_bytes(pk.to_bytes()))
}

/// Sign a 32-byte message (typically a ballot preimage) under `ikm`.
pub fn phase27_bls_sign(
    ikm: &[u8; 32],
    message: &[u8; 32],
) -> Result<BlsSignatureG2, Phase27BlsError> {
    let sk = SecretKey::key_gen(ikm, &[]).map_err(|_| Phase27BlsError::InvalidSecretKey)?;
    let sig = sk.sign(message, PHASE27_BLS_SIG_DST, &[]);
    Ok(BlsSignatureG2::from_bytes(sig.to_bytes()))
}

/// Aggregate individual BLS G2 signatures via `blst`.
pub fn phase27_aggregate_bls_signatures(
    signatures: &[BlsSignatureG2],
) -> Result<BlsSignatureG2, Phase27BlsError> {
    if signatures.is_empty() {
        return Err(BlsAggregateError::EmptySignerSet.into());
    }
    let parsed: Result<Vec<Signature>, Phase27BlsError> = signatures
        .iter()
        .map(|s| {
            Signature::from_bytes(s.as_bytes()).map_err(|e| {
                if matches!(
                    e,
                    BLST_ERROR::BLST_BAD_ENCODING
                        | BLST_ERROR::BLST_POINT_NOT_ON_CURVE
                        | BLST_ERROR::BLST_POINT_NOT_IN_GROUP
                ) {
                    Phase27BlsError::InvalidSignature
                } else {
                    Phase27BlsError::from(e)
                }
            })
        })
        .collect();
    let parsed = parsed?;
    let refs: Vec<&Signature> = parsed.iter().collect();
    let agg = AggregateSignature::aggregate(&refs, true)
        .map_err(|_| Phase27BlsError::AggregateFailed)?;
    Ok(BlsSignatureG2::from_bytes(agg.to_signature().to_bytes()))
}

/// Verify a BLS aggregate against a dual-key set (`fast_aggregate_verify`).
pub fn phase27_verify_bls_aggregate(
    message: &[u8; 32],
    aggregate: &BlsSignatureG2,
    identities: &[DualKeyIdentity],
) -> Result<(), Phase27BlsError> {
    phase27_validate_dual_key_set(identities)?;
    let sig = Signature::from_bytes(aggregate.as_bytes()).map_err(|_| {
        Phase27BlsError::InvalidSignature
    })?;
    let pks: Result<Vec<PublicKey>, Phase27BlsError> = identities
        .iter()
        .map(|id| {
            let pk = PublicKey::from_bytes(id.bls_pubkey.as_bytes()).map_err(Phase27BlsError::from)?;
            if pk.validate().is_err() {
                return Err(Phase27BlsError::InvalidPublicKey);
            }
            Ok(pk)
        })
        .collect();
    let pks = pks?;
    let pk_refs: Vec<&PublicKey> = pks.iter().collect();
    let err = sig.fast_aggregate_verify(true, message, PHASE27_BLS_SIG_DST, &pk_refs);
    if err == BLST_ERROR::BLST_SUCCESS {
        Ok(())
    } else {
        Err(Phase27BlsError::VerifyFailed)
    }
}

/// Host-local dual-key registry (not consensus state; not slash appeals V5).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlsDualKeyRegistry {
    /// AccountId bytes → compressed BLS G1 pubkey.
    entries: BTreeMap<[u8; 32], BlsPublicKeyG1>,
}

impl BlsDualKeyRegistry {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, account: &AccountId) -> Option<&BlsPublicKeyG1> {
        self.entries.get(&account.0)
    }

    pub fn list(&self) -> Vec<DualKeyIdentity> {
        self.entries
            .iter()
            .map(|(acct, pk)| DualKeyIdentity {
                account_id: AccountId(*acct),
                bls_pubkey: *pk,
            })
            .collect()
    }

    /// Register or replace a dual-key binding after curve validation.
    pub fn register(
        &mut self,
        account: AccountId,
        bls_pubkey: BlsPublicKeyG1,
    ) -> Result<(), Phase27BlsError> {
        let _ = phase27_parse_bls_pubkey(bls_pubkey.as_bytes())?;
        self.entries.insert(account.0, bls_pubkey);
        Ok(())
    }

    /// Remove a binding (no-op if absent).
    pub fn unregister(&mut self, account: &AccountId) -> bool {
        self.entries.remove(&account.0).is_some()
    }

    /// Resolve AccountIds to identities; errors if any are missing.
    pub fn resolve_identities(
        &self,
        accounts: &[AccountId],
    ) -> Result<Vec<DualKeyIdentity>, Phase27BlsError> {
        let mut out = Vec::with_capacity(accounts.len());
        let mut seen = std::collections::BTreeSet::new();
        for acct in accounts {
            if !seen.insert(acct.0) {
                return Err(Phase27BlsError::DuplicateAccount);
            }
            let pk = self
                .get(acct)
                .ok_or(Phase27BlsError::AccountNotRegistered)?;
            out.push(DualKeyIdentity {
                account_id: *acct,
                bls_pubkey: *pk,
            });
        }
        if out.is_empty() {
            return Err(BlsAggregateError::EmptySignerSet.into());
        }
        Ok(out)
    }

    /// Verify aggregate using registered keys for `accounts` (order = signer set).
    pub fn verify_aggregate_for_accounts(
        &self,
        message: &[u8; 32],
        aggregate: &BlsSignatureG2,
        accounts: &[AccountId],
    ) -> Result<(), Phase27BlsError> {
        let identities = self.resolve_identities(accounts)?;
        phase27_verify_bls_aggregate(message, aggregate, &identities)
    }
}

/// Encode registry as magic + u32 version + bincode.
pub fn encode_bls_dual_key_registry(reg: &BlsDualKeyRegistry) -> Result<Vec<u8>, String> {
    let payload = bincode::serialize(reg).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(4 + 4 + payload.len());
    out.extend_from_slice(PHASE27_BLS_DUAL_KEY_REGISTRY_MAGIC);
    out.extend_from_slice(&PHASE27_BLS_DUAL_KEY_REGISTRY_VERSION.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Decode registry envelope (V1 only for this phase).
pub fn decode_bls_dual_key_registry(bytes: &[u8]) -> Result<BlsDualKeyRegistry, String> {
    if bytes.len() < 8 {
        return Err("bls_dual_key_registry.bin too short".into());
    }
    if &bytes[..4] != PHASE27_BLS_DUAL_KEY_REGISTRY_MAGIC {
        return Err("bls_dual_key_registry.bin bad magic".into());
    }
    let ver = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if ver != PHASE27_BLS_DUAL_KEY_REGISTRY_VERSION {
        return Err(format!(
            "unsupported bls_dual_key_registry.bin version {ver}"
        ));
    }
    bincode::deserialize(&bytes[8..]).map_err(|e| e.to_string())
}

pub fn phase27_resolve_deadline(submit_height: u64) -> u64 {
    phase26_resolve_deadline(submit_height)
}

pub fn phase27_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase26_resolve_expired(tip_height, resolve_deadline)
}

pub fn phase27_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase26_expired_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase27_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase26_resolved_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase27_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase26_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase27_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase26_snapshot_electorate_w(pinned_stakes)
}

pub fn phase27_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase26_pin_set_commitment(pinned)
}

pub fn phase27_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase26_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase27_committee_requires_attested(pinned_len: usize) -> bool {
    phase26_committee_requires_attested(pinned_len)
}

pub fn phase27_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase26_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 27 blst wire).
pub fn phase27_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase26_das_succinct_proof_plan();
    v.extend([
        "Phase 27: wire blst (min_pk) — real sign / aggregate / fast_aggregate_verify; PHASE27_BLS_AGGREGATE_WIRED=true for library + RPC.",
        "Phase 27: host-local bls_dual_key_registry.bin (BDKR v1) — not slash V5, not state-root; list/register/verify RPC.",
        "Phase 27: live ResolveSlashAppealAttested (32) stays Ed25519 verify-each — do not flip committee cast until aggregate submit path lands.",
        "Phase 27: remaining — PoP for dual-keys, aggregate submit tx / signer bitset, flip verify-each → aggregate when operators opt in.",
        "Phase 27: slash_appeals_registry.bin stays V5 (untouched); do not clear; txs 30/31/32 unchanged; Phase 19–26 policies unchanged.",
        "Phase 27: SNARK/IPA/KZG and live 256 MiB remain later — keep Fly-safe 16 MiB.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 26 + Phase 27 ops).
pub fn phase27_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase26_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 27: do not clear slash_appeals_registry.bin — stay V5; dual-keys live in separate bls_dual_key_registry.bin.",
        "Phase 27: library/RPC BLS aggregate is wired (PHASE27_BLS_AGGREGATE_WIRED=true); live tx 32 remains Ed25519 verify-each.",
        "Phase 27: register dual-keys via boing_registerBlsDualKey (operator auth when token set); verify via boing_verifyBlsAggregate.",
        "Phase 27: PoP + consensus aggregate submit / flip cast still later — do not treat registry as committee identity yet.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wired_flag_and_indices() {
        assert!(PHASE27_BLS_AGGREGATE_WIRED);
        assert_eq!(PHASE27_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE27_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE27_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE27_BLS_PUBLIC_KEY_G1_BYTES, 48);
        assert_eq!(PHASE27_BLS_SIGNATURE_G2_BYTES, 96);
    }

    #[test]
    fn sign_aggregate_verify_roundtrip() {
        let ikm1 = [1u8; 32];
        let ikm2 = [2u8; 32];
        let pk1 = phase27_bls_pubkey_from_ikm(&ikm1).unwrap();
        let pk2 = phase27_bls_pubkey_from_ikm(&ikm2).unwrap();
        let msg = phase27_bls_aggregate_ballot_preimage(6913, 1, 7, true, &[9u8; 32]);
        let s1 = phase27_bls_sign(&ikm1, &msg).unwrap();
        let s2 = phase27_bls_sign(&ikm2, &msg).unwrap();
        let agg = phase27_aggregate_bls_signatures(&[s1, s2]).unwrap();
        let ids = [
            DualKeyIdentity {
                account_id: AccountId([0x11; 32]),
                bls_pubkey: pk1,
            },
            DualKeyIdentity {
                account_id: AccountId([0x22; 32]),
                bls_pubkey: pk2,
            },
        ];
        assert!(phase27_verify_bls_aggregate(&msg, &agg, &ids).is_ok());
        let bad = phase27_bls_aggregate_ballot_preimage(6913, 1, 7, false, &[9u8; 32]);
        assert_eq!(
            phase27_verify_bls_aggregate(&bad, &agg, &ids),
            Err(Phase27BlsError::VerifyFailed)
        );
    }

    #[test]
    fn registry_persist_and_verify() {
        let ikm = [3u8; 32];
        let pk = phase27_bls_pubkey_from_ikm(&ikm).unwrap();
        let acct = AccountId([0xAB; 32]);
        let mut reg = BlsDualKeyRegistry::new();
        reg.register(acct, pk).unwrap();
        let bytes = encode_bls_dual_key_registry(&reg).unwrap();
        let back = decode_bls_dual_key_registry(&bytes).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back.get(&acct), Some(&pk));

        let msg = phase27_bls_aggregate_ballot_preimage(1, 2, 3, true, &[0u8; 32]);
        let sig = phase27_bls_sign(&ikm, &msg).unwrap();
        assert!(back
            .verify_aggregate_for_accounts(&msg, &sig, &[acct])
            .is_ok());
        assert_eq!(
            back.verify_aggregate_for_accounts(&msg, &sig, &[AccountId([0; 32])]),
            Err(Phase27BlsError::AccountNotRegistered)
        );
    }

    #[test]
    fn reject_invalid_pubkey_length_and_zeros() {
        assert!(phase27_parse_bls_pubkey(&[0u8; 47]).is_err());
        // All-zero compressed G1 is not a valid subgroup point.
        assert!(matches!(
            phase27_parse_bls_pubkey(&[0u8; 48]),
            Err(Phase27BlsError::InvalidPublicKey)
        ));
    }

    #[test]
    fn plan_mentions_blst_and_tx32_safe() {
        let plan = phase27_das_succinct_proof_plan();
        assert!(plan.iter().any(|s| s.contains("blst") && s.contains("WIRED")));
        assert!(plan.iter().any(|s| s.contains("bls_dual_key_registry")));
        assert!(plan.iter().any(|s| s.contains("verify-each")));
        let checklist = phase27_cap_activation_checklist(Some(1200));
        assert!(checklist
            .iter()
            .any(|s| s.contains("slash_appeals_registry.bin") && s.contains("V5")));
    }
}
