//! Phase 28: BLS dual-key proof-of-possession (PoP).
//!
//! Builds on Phase 27 (`blst` wire + host-local dual-key registry). Smallest coherent slice:
//! - Chain-bound BLS PoP message + sign/verify helpers (`min_pk`).
//! - Registration requires a valid PoP (BLS secret key binds to `AccountId` + pubkey + chain).
//! - Do **not** change live ResolveSlashAppealAttested (**32**) Ed25519 verify-each.
//! - Do **not** clear `slash_appeals_registry.bin` (stay V5); dual-key file stays BDKR v1
//!   (PoP verified at register time; signature not persisted).
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - ReservedRealV1 verify still rejects; SNARK / 256 MiB still later.
//!
//! Remaining after this slice:
//! 1. Aggregate submit path (new payload or extended **32**) + signer bitset.
//! 2. Flip committee cast from Ed25519 verify-each → BLS aggregate when operators opt in.
//! 3. Optional Ed25519 account-side binding (owner consents to BLS key) on a consensus tx.

use crate::blobs_phase10::BlobSampleServeChallengeStatus;
use crate::blobs_phase26::{BlsPublicKeyG1, BlsSignatureG2, DualKeyIdentity};
use crate::blobs_phase27::{
    phase27_bls_sign, phase27_cap_activation_checklist, phase27_committee_requires_attested,
    phase27_das_succinct_proof_plan, phase27_expired_appeal_prunable,
    phase27_is_legacy_electorate_snapshot, phase27_parse_bls_pubkey, phase27_pin_set_commitment,
    phase27_resolve_deadline, phase27_resolve_expired, phase27_resolve_threshold_met,
    phase27_resolved_appeal_prunable, phase27_snapshot_electorate_w,
    phase27_terminal_serve_challenge_prunable, phase27_validate_dual_key_set, BlsDualKeyRegistry,
    Phase27BlsError, PHASE27_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE27_BLS_AGGREGATE_BALLOT_DOMAIN,
    PHASE27_BLS_AGGREGATE_WIRED, PHASE27_BLS_DUAL_KEY_REGISTRY_FILE,
    PHASE27_BLS_DUAL_KEY_REGISTRY_MAGIC, PHASE27_BLS_DUAL_KEY_REGISTRY_VERSION,
    PHASE27_BLS_PUBLIC_KEY_G1_BYTES, PHASE27_BLS_SIGNATURE_G2_BYTES, PHASE27_CAP_ACTIVATION_ENV,
    PHASE27_EXPIRED_APPEAL_RETENTION_BLOCKS, PHASE27_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE27_LEGACY_ELECTORATE_W_SENTINEL, PHASE27_LEGACY_PIN_SET_COMMITMENT,
    PHASE27_MAX_APPEAL_EVIDENCE_BYTES, PHASE27_MAX_SETTLE_RECEIPTS, PHASE27_PIN_SET_DOMAIN,
    PHASE27_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE27_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE27_RESOLVE_THRESHOLD_DEN, PHASE27_RESOLVE_THRESHOLD_NUM, PHASE27_RESOLVE_WINDOW_BLOCKS,
    PHASE27_RESOLVED_APPEAL_RETENTION_BLOCKS, PHASE27_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE27_SETTLE_RECEIPT_TTL_BLOCKS, PHASE27_SUBMIT_SLASH_APPEAL_TX_INDEX,
    PHASE27_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
};
use crate::hash::hasher;
use crate::types::AccountId;
use blst::min_pk::{PublicKey, Signature};
use blst::BLST_ERROR;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE28_CAP_ACTIVATION_ENV: &str = PHASE27_CAP_ACTIVATION_ENV;

pub const PHASE28_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE27_ACTIVATED_MAX_MEDIA_BLOB_BYTES;
pub const PHASE28_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE27_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;
pub const PHASE28_RESOLVE_WINDOW_BLOCKS: u64 = PHASE27_RESOLVE_WINDOW_BLOCKS;
pub const PHASE28_MAX_SETTLE_RECEIPTS: usize = PHASE27_MAX_SETTLE_RECEIPTS;
pub const PHASE28_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE27_SERVE_CHALLENGE_WINDOW_BLOCKS;
pub const PHASE28_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE27_SETTLE_RECEIPT_TTL_BLOCKS;
pub const PHASE28_EXPIRED_APPEAL_RETENTION_BLOCKS: u64 = PHASE27_EXPIRED_APPEAL_RETENTION_BLOCKS;
pub const PHASE28_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS: u64 =
    PHASE27_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS;
pub const PHASE28_RESOLVED_APPEAL_RETENTION_BLOCKS: u64 = PHASE27_RESOLVED_APPEAL_RETENTION_BLOCKS;
pub const PHASE28_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE27_MAX_APPEAL_EVIDENCE_BYTES;
pub const PHASE28_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE27_SUBMIT_SLASH_APPEAL_TX_INDEX;
pub const PHASE28_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE27_RESOLVE_SLASH_APPEAL_TX_INDEX;
pub const PHASE28_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE27_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;
pub const PHASE28_RESOLVE_THRESHOLD_NUM: u64 = PHASE27_RESOLVE_THRESHOLD_NUM;
pub const PHASE28_RESOLVE_THRESHOLD_DEN: u64 = PHASE27_RESOLVE_THRESHOLD_DEN;
pub const PHASE28_PIN_SET_DOMAIN: &[u8] = PHASE27_PIN_SET_DOMAIN;
pub const PHASE28_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE27_LEGACY_ELECTORATE_W_SENTINEL;
pub const PHASE28_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE27_LEGACY_PIN_SET_COMMITMENT;

pub const PHASE28_BLS_PUBLIC_KEY_G1_BYTES: usize = PHASE27_BLS_PUBLIC_KEY_G1_BYTES;
pub const PHASE28_BLS_SIGNATURE_G2_BYTES: usize = PHASE27_BLS_SIGNATURE_G2_BYTES;
pub const PHASE28_BLS_AGGREGATE_BALLOT_DOMAIN: &[u8] = PHASE27_BLS_AGGREGATE_BALLOT_DOMAIN;
pub const PHASE28_BLS_AGGREGATE_WIRED: bool = PHASE27_BLS_AGGREGATE_WIRED;
pub const PHASE28_BLS_DUAL_KEY_REGISTRY_FILE: &str = PHASE27_BLS_DUAL_KEY_REGISTRY_FILE;
pub const PHASE28_BLS_DUAL_KEY_REGISTRY_MAGIC: &[u8; 4] = PHASE27_BLS_DUAL_KEY_REGISTRY_MAGIC;
pub const PHASE28_BLS_DUAL_KEY_REGISTRY_VERSION: u32 = PHASE27_BLS_DUAL_KEY_REGISTRY_VERSION;

/// BLAKE3 domain for the PoP preimage (not the blst DST).
pub const PHASE28_BLS_POP_DOMAIN: &[u8] = b"BOING_BLS_DUAL_KEY_POP_V1";

/// CipherSuite DST for dual-key PoP signatures (distinct from ballot DST).
pub const PHASE28_BLS_POP_SIG_DST: &[u8] = b"BOING_BLS_POP_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

/// Registration requires a verified BLS PoP.
pub const PHASE28_BLS_POP_REQUIRED: bool = true;

/// Errors for Phase 28 PoP (includes Phase 27 variants via transparent).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Phase28BlsError {
    #[error(transparent)]
    Inner(#[from] Phase27BlsError),
    #[error("BLS dual-key proof-of-possession required")]
    PopRequired,
    #[error("invalid BLS proof-of-possession")]
    InvalidPop,
    #[error("chain_id required for BLS dual-key proof-of-possession")]
    ChainIdRequired,
}

/// Domain-separated PoP preimage: BLAKE3(domain || chain_id_le8 || account_id || bls_pubkey).
pub fn phase28_bls_pop_preimage(
    chain_id: u64,
    account: &AccountId,
    bls_pubkey: &BlsPublicKeyG1,
) -> [u8; 32] {
    let mut h = hasher();
    h.update(PHASE28_BLS_POP_DOMAIN);
    h.update(&chain_id.to_le_bytes());
    h.update(&account.0);
    h.update(bls_pubkey.as_bytes());
    *h.finalize().as_bytes()
}

/// Sign a dual-key PoP with IKM-derived BLS secret key.
pub fn phase28_bls_sign_pop(
    ikm: &[u8; 32],
    chain_id: u64,
    account: &AccountId,
    bls_pubkey: &BlsPublicKeyG1,
) -> Result<BlsSignatureG2, Phase28BlsError> {
    let msg = phase28_bls_pop_preimage(chain_id, account, bls_pubkey);
    // Sign with PoP DST (not ballot DST).
    let sk = blst::min_pk::SecretKey::key_gen(ikm, &[])
        .map_err(|_| Phase27BlsError::InvalidSecretKey)?;
    let sig = sk.sign(&msg, PHASE28_BLS_POP_SIG_DST, &[]);
    Ok(BlsSignatureG2::from_bytes(sig.to_bytes()))
}

/// Verify a dual-key PoP: BLS pubkey signs the chain-bound account+pubkey preimage.
pub fn phase28_bls_verify_pop(
    chain_id: u64,
    account: &AccountId,
    bls_pubkey: &BlsPublicKeyG1,
    pop: &BlsSignatureG2,
) -> Result<(), Phase28BlsError> {
    let _ = phase27_parse_bls_pubkey(bls_pubkey.as_bytes())?;
    let msg = phase28_bls_pop_preimage(chain_id, account, bls_pubkey);
    let pk = PublicKey::from_bytes(bls_pubkey.as_bytes()).map_err(Phase27BlsError::from)?;
    if pk.validate().is_err() {
        return Err(Phase27BlsError::InvalidPublicKey.into());
    }
    let sig = Signature::from_bytes(pop.as_bytes()).map_err(|_| Phase27BlsError::InvalidSignature)?;
    let err = sig.verify(true, &msg, PHASE28_BLS_POP_SIG_DST, &[], &pk, true);
    if err == BLST_ERROR::BLST_SUCCESS {
        Ok(())
    } else {
        Err(Phase28BlsError::InvalidPop)
    }
}

/// Register after verifying PoP. Existing BDKR v1 on-disk format unchanged.
pub fn phase28_register_with_pop(
    registry: &mut BlsDualKeyRegistry,
    chain_id: u64,
    account: AccountId,
    bls_pubkey: BlsPublicKeyG1,
    pop: &BlsSignatureG2,
) -> Result<(), Phase28BlsError> {
    phase28_bls_verify_pop(chain_id, &account, &bls_pubkey, pop)?;
    registry.register(account, bls_pubkey)?;
    Ok(())
}

pub fn phase28_validate_dual_key_set(
    identities: &[DualKeyIdentity],
) -> Result<(), crate::blobs_phase26::BlsAggregateError> {
    phase27_validate_dual_key_set(identities)
}

pub fn phase28_resolve_deadline(submit_height: u64) -> u64 {
    phase27_resolve_deadline(submit_height)
}

pub fn phase28_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase27_resolve_expired(tip_height, resolve_deadline)
}

pub fn phase28_expired_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase27_expired_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase28_resolved_appeal_prunable(tip_height: u64, resolve_deadline: u64) -> bool {
    phase27_resolved_appeal_prunable(tip_height, resolve_deadline)
}

pub fn phase28_terminal_serve_challenge_prunable(
    tip_height: u64,
    deadline_height: u64,
    status: BlobSampleServeChallengeStatus,
    bond_settled: bool,
) -> bool {
    phase27_terminal_serve_challenge_prunable(tip_height, deadline_height, status, bond_settled)
}

pub fn phase28_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase27_snapshot_electorate_w(pinned_stakes)
}

pub fn phase28_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase27_pin_set_commitment(pinned)
}

pub fn phase28_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase27_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

pub fn phase28_committee_requires_attested(pinned_len: usize) -> bool {
    phase27_committee_requires_attested(pinned_len)
}

pub fn phase28_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase27_resolve_threshold_met(weight, electorate_w)
}

/// Operator-facing plan strings (Phase 28 BLS PoP).
pub fn phase28_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase27_das_succinct_proof_plan();
    v.extend([
        "Phase 28: BLS dual-key proof-of-possession — chain-bound PoP required on boing_registerBlsDualKey (PHASE28_BLS_POP_REQUIRED=true).",
        "Phase 28: PoP message = BLAKE3(BOING_BLS_DUAL_KEY_POP_V1 || chain_id || accountId || blsPubkey); signed under BOING_BLS_POP_… DST.",
        "Phase 28: live ResolveSlashAppealAttested (32) stays Ed25519 verify-each — PoP does not flip committee cast.",
        "Phase 28: bls_dual_key_registry.bin stays BDKR v1 (PoP checked at register; signature not persisted); slash appeals stay V5.",
        "Phase 28: remaining — aggregate submit tx / signer bitset, flip verify-each → aggregate, optional Ed25519 account-side binding, SNARK / 256 MiB.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 27 + Phase 28 ops).
pub fn phase28_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase27_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 28: do not clear slash_appeals_registry.bin — stay V5; dual-keys remain in bls_dual_key_registry.bin (BDKR v1).",
        "Phase 28: boing_registerBlsDualKey requires blsPop (96 B) + chain_id; reject without valid PoP.",
        "Phase 28: live tx 32 remains Ed25519 verify-each; aggregate submit / flip cast still later.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs_phase27::phase27_bls_pubkey_from_ikm;

    #[test]
    fn pop_roundtrip_and_binding() {
        let ikm = [11u8; 32];
        let pk = phase27_bls_pubkey_from_ikm(&ikm).unwrap();
        let acct = AccountId([0xCD; 32]);
        let pop = phase28_bls_sign_pop(&ikm, 6913, &acct, &pk).unwrap();
        assert!(phase28_bls_verify_pop(6913, &acct, &pk, &pop).is_ok());
        assert_eq!(
            phase28_bls_verify_pop(6914, &acct, &pk, &pop),
            Err(Phase28BlsError::InvalidPop)
        );
        assert_eq!(
            phase28_bls_verify_pop(6913, &AccountId([0xEE; 32]), &pk, &pop),
            Err(Phase28BlsError::InvalidPop)
        );
        // Ballot DST signature must not satisfy PoP verify.
        let msg = phase28_bls_pop_preimage(6913, &acct, &pk);
        let ballot_sig = phase27_bls_sign(&ikm, &msg).unwrap();
        assert_eq!(
            phase28_bls_verify_pop(6913, &acct, &pk, &ballot_sig),
            Err(Phase28BlsError::InvalidPop)
        );
    }

    #[test]
    fn register_with_pop() {
        let ikm = [12u8; 32];
        let pk = phase27_bls_pubkey_from_ikm(&ikm).unwrap();
        let acct = AccountId([0xAA; 32]);
        let pop = phase28_bls_sign_pop(&ikm, 6913, &acct, &pk).unwrap();
        let mut reg = BlsDualKeyRegistry::new();
        phase28_register_with_pop(&mut reg, 6913, acct, pk, &pop).unwrap();
        assert_eq!(reg.len(), 1);
        let bad = phase28_bls_sign_pop(&ikm, 6914, &acct, &pk).unwrap();
        let mut reg2 = BlsDualKeyRegistry::new();
        assert!(phase28_register_with_pop(&mut reg2, 6913, acct, pk, &bad).is_err());
    }

    #[test]
    fn flags_and_plan() {
        assert!(PHASE28_BLS_POP_REQUIRED);
        assert!(PHASE28_BLS_AGGREGATE_WIRED);
        assert_eq!(PHASE28_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        let plan = phase28_das_succinct_proof_plan();
        assert!(plan.iter().any(|s| s.contains("proof-of-possession") || s.contains("PoP")));
        assert!(plan.iter().any(|s| s.contains("verify-each")));
        let checklist = phase28_cap_activation_checklist(Some(1200));
        assert!(checklist.iter().any(|s| s.contains("blsPop")));
    }
}
