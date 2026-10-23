//! Phase 22: batch ballot re-verify RPC + pin-bound ballot domain V2.
//!
//! New attested casts of **ResolveSlashAppealAttested (32)** sign over domain
//! `BOING_SLASH_APPEAL_RESOLVE_BALLOT_V2`, which binds the Submit-frozen
//! `pin_set_commitment`. Historical V1 ballots remain valid via dual-verify
//! (V2 then V1). Read-only RPC `boing_reverifySlashAppealBallots` batch-checks
//! stored ballots + pin-set + registry commitment — no state mutation.
//!
//! Honest scope:
//! - Reuse payload indices **30** / **31** / **32** — no new tx.
//! - Registry stays **V5** (no schema change); **do not clear** `slash_appeals_registry.bin`.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Phase 19 multi-pin → require **32**; Phase 20 frozen W; Phase 21 resolve expiry — unchanged.
//! - True BLS, SNARK/IPA/KZG, live 256 MiB — still later (24+); Phase 23 is Expired prune.

use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey};

use crate::blobs_phase18::{
    sign_slash_appeal_resolve_ballot, slash_appeal_resolve_ballot_preimage,
    verify_slash_appeal_resolve_ballot, SlashAppealBallotSignature,
    SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN,
};
use crate::blobs_phase21::{
    phase21_cap_activation_checklist, phase21_committee_requires_attested,
    phase21_das_succinct_proof_plan, phase21_is_legacy_electorate_snapshot,
    phase21_pin_set_commitment, phase21_resolve_deadline, phase21_resolve_expired,
    phase21_resolve_threshold_met, phase21_snapshot_electorate_w,
    PHASE21_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE21_CAP_ACTIVATION_ENV,
    PHASE21_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE21_LEGACY_ELECTORATE_W_SENTINEL,
    PHASE21_LEGACY_PIN_SET_COMMITMENT, PHASE21_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE21_MAX_SETTLE_RECEIPTS, PHASE21_PIN_SET_DOMAIN,
    PHASE21_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, PHASE21_RESOLVE_SLASH_APPEAL_TX_INDEX,
    PHASE21_RESOLVE_THRESHOLD_DEN, PHASE21_RESOLVE_THRESHOLD_NUM, PHASE21_RESOLVE_WINDOW_BLOCKS,
    PHASE21_SERVE_CHALLENGE_WINDOW_BLOCKS, PHASE21_SETTLE_RECEIPT_TTL_BLOCKS,
    PHASE21_SUBMIT_SLASH_APPEAL_TX_INDEX,
};
use crate::hash::hasher;
use crate::signature::SignatureError;
use crate::types::AccountId;

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE22_CAP_ACTIVATION_ENV: &str = PHASE21_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE22_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE21_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE22_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE21_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Resolve window after Submit (Phase 21 alias).
pub const PHASE22_RESOLVE_WINDOW_BLOCKS: u64 = PHASE21_RESOLVE_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE22_MAX_SETTLE_RECEIPTS: usize = PHASE21_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE22_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE21_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE22_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE21_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE22_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE21_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE22_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE21_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`] (unattested).
pub const PHASE22_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE21_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppealAttested`].
pub const PHASE22_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 =
    PHASE21_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX;

/// Numerator of the fixed stake-weighted resolve quorum (⌈2W/3⌉).
pub const PHASE22_RESOLVE_THRESHOLD_NUM: u64 = PHASE21_RESOLVE_THRESHOLD_NUM;

/// Denominator of the fixed stake-weighted resolve quorum.
pub const PHASE22_RESOLVE_THRESHOLD_DEN: u64 = PHASE21_RESOLVE_THRESHOLD_DEN;

/// Domain tag for pin-set commitment at Submit (Phase 20/21 alias).
pub const PHASE22_PIN_SET_DOMAIN: &[u8] = PHASE21_PIN_SET_DOMAIN;

/// Sentinel: missing snapshot on migrated pre-V4 Pending appeals (live-W fallback).
pub const PHASE22_LEGACY_ELECTORATE_W_SENTINEL: u128 = PHASE21_LEGACY_ELECTORATE_W_SENTINEL;

/// Zero pin commitment paired with legacy W sentinel ⇒ legacy row.
pub const PHASE22_LEGACY_PIN_SET_COMMITMENT: [u8; 32] = PHASE21_LEGACY_PIN_SET_COMMITMENT;

/// Historical ballot domain (Phase 18 V1) — still dual-verified.
pub const SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V1: &[u8] = SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN;

/// Phase 22 ballot domain: binds Submit-frozen `pin_set_commitment`.
pub const SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V2: &[u8] = b"BOING_SLASH_APPEAL_RESOLVE_BALLOT_V2";

/// Which ballot domain verified successfully under dual-verify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashAppealBallotDomain {
    V1,
    V2,
}

impl SlashAppealBallotDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
        }
    }
}

/// `resolve_deadline` from Submit tip height (Phase 21 alias).
pub fn phase22_resolve_deadline(submit_height: u64) -> u64 {
    phase21_resolve_deadline(submit_height)
}

/// True when tip is strictly past the resolve deadline (Phase 21 alias).
pub fn phase22_resolve_expired(tip_height: u64, resolve_deadline: u64) -> bool {
    phase21_resolve_expired(tip_height, resolve_deadline)
}

/// Snapshot electorate W (Phase 20/21 alias).
pub fn phase22_snapshot_electorate_w(pinned_stakes: &[u128]) -> u128 {
    phase21_snapshot_electorate_w(pinned_stakes)
}

/// Pin-set commitment (Phase 20/21 alias).
pub fn phase22_pin_set_commitment(pinned: &[AccountId]) -> [u8; 32] {
    phase21_pin_set_commitment(pinned)
}

/// Legacy electorate snapshot detection (Phase 20/21 alias).
pub fn phase22_is_legacy_electorate_snapshot(
    electorate_w_snapshot: u128,
    pin_set_commitment: &[u8; 32],
) -> bool {
    phase21_is_legacy_electorate_snapshot(electorate_w_snapshot, pin_set_commitment)
}

/// True when a multi-pin committee must use attested resolve (**32**) (Phase 19 alias).
pub fn phase22_committee_requires_attested(pinned_len: usize) -> bool {
    phase21_committee_requires_attested(pinned_len)
}

/// True when `weight` meets ⌈2W/3⌉ (alias).
pub fn phase22_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase21_resolve_threshold_met(weight, electorate_w)
}

/// True when the pin commitment is the legacy all-zero sentinel (no meaningful pin to bind).
pub fn phase22_is_legacy_pin_commitment(pin_set_commitment: &[u8; 32]) -> bool {
    *pin_set_commitment == PHASE22_LEGACY_PIN_SET_COMMITMENT
}

/// BLAKE3 V1 preimage (alias of Phase 18).
pub fn slash_appeal_resolve_ballot_preimage_v1(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    voter: &AccountId,
) -> [u8; 32] {
    slash_appeal_resolve_ballot_preimage(chain_id, appeal_id, slash_id, approved, voter)
}

/// BLAKE3(`DOMAIN_V2 || chain_id_le8 || appeal_id_le8 || slash_id_le8 || approved_u8 || voter || pin_set_commitment`).
pub fn slash_appeal_resolve_ballot_preimage_v2(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    voter: &AccountId,
    pin_set_commitment: &[u8; 32],
) -> [u8; 32] {
    let mut h = hasher();
    h.update(SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V2);
    h.update(&chain_id.to_le_bytes());
    h.update(&appeal_id.to_le_bytes());
    h.update(&slash_id.to_le_bytes());
    h.update(&[u8::from(approved)]);
    h.update(&voter.0);
    h.update(pin_set_commitment);
    *h.finalize().as_bytes()
}

/// Sign a V2 resolve ballot (binds `pin_set_commitment`). Public key must equal voter.
pub fn sign_slash_appeal_resolve_ballot_v2(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    signing_key: &SigningKey,
    pin_set_commitment: &[u8; 32],
) -> SlashAppealBallotSignature {
    let voter = AccountId(signing_key.verifying_key().to_bytes());
    let msg = slash_appeal_resolve_ballot_preimage_v2(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        &voter,
        pin_set_commitment,
    );
    SlashAppealBallotSignature(signing_key.sign(&msg).to_bytes())
}

/// Verify a V2 resolve ballot against `voter` + `pin_set_commitment`.
pub fn verify_slash_appeal_resolve_ballot_v2(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    voter: &AccountId,
    pin_set_commitment: &[u8; 32],
    ballot_signature: &SlashAppealBallotSignature,
) -> Result<(), SignatureError> {
    let pk = VerifyingKey::from_bytes(&voter.0).map_err(|_| SignatureError::InvalidPublicKey)?;
    let msg = slash_appeal_resolve_ballot_preimage_v2(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        voter,
        pin_set_commitment,
    );
    let ed_sig = Ed25519Signature::from_bytes(&ballot_signature.0);
    pk.verify_strict(&msg, &ed_sig)
        .map_err(|_| SignatureError::InvalidSignature)?;
    Ok(())
}

/// Sign a V1 resolve ballot (historical / mid-upgrade). Alias of Phase 18.
pub fn sign_slash_appeal_resolve_ballot_v1(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    signing_key: &SigningKey,
) -> SlashAppealBallotSignature {
    sign_slash_appeal_resolve_ballot(chain_id, appeal_id, slash_id, approved, signing_key)
}

/// Verify a V1 resolve ballot. Alias of Phase 18.
pub fn verify_slash_appeal_resolve_ballot_v1(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    voter: &AccountId,
    ballot_signature: &SlashAppealBallotSignature,
) -> Result<(), SignatureError> {
    verify_slash_appeal_resolve_ballot(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        voter,
        ballot_signature,
    )
}

/// Dual-verify a stored / cast ballot.
///
/// - Legacy pin sentinel (all-zero): **V1 only** (no meaningful pin to bind).
/// - Otherwise: try **V2** first (preferred for new casts), then **V1** (historical).
/// - Rejects only if neither domain verifies.
pub fn verify_slash_appeal_resolve_ballot_dual(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    voter: &AccountId,
    pin_set_commitment: &[u8; 32],
    ballot_signature: &SlashAppealBallotSignature,
) -> Result<SlashAppealBallotDomain, SignatureError> {
    if phase22_is_legacy_pin_commitment(pin_set_commitment) {
        verify_slash_appeal_resolve_ballot_v1(
            chain_id,
            appeal_id,
            slash_id,
            approved,
            voter,
            ballot_signature,
        )?;
        return Ok(SlashAppealBallotDomain::V1);
    }
    if verify_slash_appeal_resolve_ballot_v2(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        voter,
        pin_set_commitment,
        ballot_signature,
    )
    .is_ok()
    {
        return Ok(SlashAppealBallotDomain::V2);
    }
    verify_slash_appeal_resolve_ballot_v1(
        chain_id,
        appeal_id,
        slash_id,
        approved,
        voter,
        ballot_signature,
    )?;
    Ok(SlashAppealBallotDomain::V1)
}

/// Operator-facing plan strings (Phase 22 batch re-verify + pin-bound V2 ballots).
pub fn phase22_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase21_das_succinct_proof_plan();
    v.extend([
        "Phase 22: boing_reverifySlashAppealBallots is a read-only audit RPC — batch-checks stored Ed25519 resolve ballots, pin-set match, and registry commitment; never mutates book state.",
        "Phase 22: ResolveSlashAppealAttested (32) new casts use ballot domain V2 binding pin_set_commitment; dual-verify accepts historical V1 (mid-upgrade / pre-Phase-22).",
        "Phase 22: registry stays V5 (no schema change, no clear); txs 30/31/32 unchanged; Phase 19 multi-pin→32, Phase 20 frozen W, Phase 21 resolve expiry unchanged.",
        "Phase 22: true BLS aggregate, SNARK/IPA/KZG, and live 256 MiB remain later — keep Fly-safe 16 MiB; do not clear the registry.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 21 + Phase 22 ops). Do not rewrite live admission height.
pub fn phase22_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase21_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 22: do not clear slash_appeals_registry.bin — stay V5 save/load (no V6).",
        "Phase 22: clients must sign ResolveSlashAppealAttested (32) ballots with domain V2 (pin_set_commitment); roll validators together so old nodes do not reject V2-signed 32.",
        "Phase 22: mid-tally V1 ballots remain valid via dual-verify; unattested 31 votes report as unattested on re-verify RPC (not invalid).",
        "Phase 22: true BLS / SNARK / 256 MiB still later — this phase is audit re-verify + pin-bound Ed25519 ballots only.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn indices_cap_and_aliases() {
        assert_eq!(PHASE22_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE22_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE22_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE22_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(PHASE22_RESOLVE_THRESHOLD_NUM, 2);
        assert_eq!(PHASE22_RESOLVE_THRESHOLD_DEN, 3);
        assert_eq!(PHASE22_RESOLVE_WINDOW_BLOCKS, 1000);
        assert_eq!(
            PHASE22_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE21_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
        assert_eq!(
            SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN_V2,
            b"BOING_SLASH_APPEAL_RESOLVE_BALLOT_V2"
        );
    }

    #[test]
    fn v2_sign_verify_roundtrip_and_pin_bind() {
        let key = SigningKey::generate(&mut OsRng);
        let voter = AccountId(key.verifying_key().to_bytes());
        let pin = phase22_pin_set_commitment(&[voter, AccountId([2u8; 32])]);
        let sig = sign_slash_appeal_resolve_ballot_v2(6913, 1, 7, true, &key, &pin);
        assert!(
            verify_slash_appeal_resolve_ballot_v2(6913, 1, 7, true, &voter, &pin, &sig).is_ok()
        );
        let wrong_pin = phase22_pin_set_commitment(&[AccountId([9u8; 32])]);
        assert!(
            verify_slash_appeal_resolve_ballot_v2(6913, 1, 7, true, &voter, &wrong_pin, &sig)
                .is_err()
        );
        // V2 sig is not a valid V1 ballot.
        assert!(
            verify_slash_appeal_resolve_ballot_v1(6913, 1, 7, true, &voter, &sig).is_err()
        );
    }

    #[test]
    fn dual_verify_prefers_v2_accepts_v1() {
        let key = SigningKey::generate(&mut OsRng);
        let voter = AccountId(key.verifying_key().to_bytes());
        let pin = phase22_pin_set_commitment(&[voter]);
        let sig_v2 = sign_slash_appeal_resolve_ballot_v2(6913, 2, 8, true, &key, &pin);
        assert_eq!(
            verify_slash_appeal_resolve_ballot_dual(6913, 2, 8, true, &voter, &pin, &sig_v2)
                .unwrap(),
            SlashAppealBallotDomain::V2
        );
        let sig_v1 = sign_slash_appeal_resolve_ballot_v1(6913, 2, 8, true, &key);
        assert_eq!(
            verify_slash_appeal_resolve_ballot_dual(6913, 2, 8, true, &voter, &pin, &sig_v1)
                .unwrap(),
            SlashAppealBallotDomain::V1
        );
        // Wrong pin + V2-only sig → reject (no V1 fallback for that bytes).
        let wrong = phase22_pin_set_commitment(&[AccountId([3u8; 32])]);
        assert!(
            verify_slash_appeal_resolve_ballot_dual(6913, 2, 8, true, &voter, &wrong, &sig_v2)
                .is_err()
        );
    }

    #[test]
    fn legacy_pin_dual_verify_v1_only() {
        let key = SigningKey::generate(&mut OsRng);
        let voter = AccountId(key.verifying_key().to_bytes());
        let legacy = PHASE22_LEGACY_PIN_SET_COMMITMENT;
        let sig_v1 = sign_slash_appeal_resolve_ballot_v1(6913, 3, 9, false, &key);
        assert_eq!(
            verify_slash_appeal_resolve_ballot_dual(6913, 3, 9, false, &voter, &legacy, &sig_v1)
                .unwrap(),
            SlashAppealBallotDomain::V1
        );
        // V2 over legacy pin would not be preferred path; legacy forces V1-only.
        let sig_v2 = sign_slash_appeal_resolve_ballot_v2(6913, 3, 9, false, &key, &legacy);
        assert!(
            verify_slash_appeal_resolve_ballot_dual(6913, 3, 9, false, &voter, &legacy, &sig_v2)
                .is_err()
        );
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase22_cap_activation_checklist(None).len() >= 50);
        let plan = phase22_das_succinct_proof_plan();
        assert!(plan.len() >= 50);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan
            .iter()
            .any(|s| s.contains("boing_reverifySlashAppealBallots") || s.contains("re-verify")));
        assert!(plan.iter().any(|s| s.contains("V2") && s.contains("pin_set")));
        assert!(plan
            .iter()
            .any(|s| s.contains("V5")
                && (s.contains("no clear")
                    || s.contains("no schema")
                    || s.contains("stays V5")
                    || s.contains("do not clear"))));
        assert!(plan.iter().any(|s| s.contains("still later") || s.contains("BLS")));
        assert!(phase22_committee_requires_attested(3));
        assert!(!phase22_committee_requires_attested(1));
    }
}
