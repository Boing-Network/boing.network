//! Phase 18: per-vote Ed25519 resolve attestation for slash appeals.
//!
//! New tx **ResolveSlashAppealAttested (32)** carries a domain-separated ballot over
//! `appeal_id | slash_id | approved | voter | chain_id`. Ballots accumulate on the
//! Phase 17 stake-weighted vote record and are verified individually (thin attest —
//! **not** BLS). ResolveSlashAppeal (**31**) stays for historical / unattested casts.
//!
//! Honest scope:
//! - Append-only payload index **32**; do not change variant 31 fields.
//! - Registry V3 (magic + u32 + bincode); V2/legacy load without required clear.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.
//! - Submit/Resolve/Attested (30/31/32) remain always admitted.
//! - True BLS aggregate of resolve ballots is still later (19+).

use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey};
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use crate::blobs_phase17::{
    phase17_cap_activation_checklist, phase17_das_succinct_proof_plan,
    phase17_resolve_threshold_met, PHASE17_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
    PHASE17_CAP_ACTIVATION_ENV, PHASE17_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
    PHASE17_MAX_APPEAL_EVIDENCE_BYTES, PHASE17_MAX_SETTLE_RECEIPTS,
    PHASE17_RESOLVE_SLASH_APPEAL_TX_INDEX, PHASE17_RESOLVE_THRESHOLD_DEN,
    PHASE17_RESOLVE_THRESHOLD_NUM, PHASE17_SERVE_CHALLENGE_WINDOW_BLOCKS,
    PHASE17_SETTLE_RECEIPT_TTL_BLOCKS, PHASE17_SUBMIT_SLASH_APPEAL_TX_INDEX,
};
use crate::hash::hasher;
use crate::signature::SignatureError;
use crate::types::AccountId;

/// 64-byte Ed25519 ballot signature (serde as a 64-tuple → raw bytes in bincode).
///
/// Distinct from [`crate::Signature`] (which uses length-prefixed `serialize_bytes`) so the
/// payload wire layout stays a plain `[u8; 64]` discriminant field.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SlashAppealBallotSignature(pub [u8; 64]);

impl SlashAppealBallotSignature {
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

impl fmt::Debug for SlashAppealBallotSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SlashAppealBallotSignature({}...)",
            hex::encode(&self.0[..8])
        )
    }
}

impl Serialize for SlashAppealBallotSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(64)?;
        for b in &self.0 {
            tup.serialize_element(b)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for SlashAppealBallotSignature {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = SlashAppealBallotSignature;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("64 ballot signature bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut bytes = [0u8; 64];
                for (i, slot) in bytes.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(65, &self));
                }
                Ok(SlashAppealBallotSignature(bytes))
            }
        }
        deserializer.deserialize_tuple(64, V)
    }
}

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE18_CAP_ACTIVATION_ENV: &str = PHASE17_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE18_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE17_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe consensus slash records.
pub const PHASE18_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE17_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE18_MAX_SETTLE_RECEIPTS: usize = PHASE17_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE18_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE17_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Settle-receipt TTL alias.
pub const PHASE18_SETTLE_RECEIPT_TTL_BLOCKS: u64 = PHASE17_SETTLE_RECEIPT_TTL_BLOCKS;

/// Max evidence bytes on [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE18_MAX_APPEAL_EVIDENCE_BYTES: usize = PHASE17_MAX_APPEAL_EVIDENCE_BYTES;

/// Bincode / payload index for [`crate::TransactionPayload::SubmitSlashAppeal`].
pub const PHASE18_SUBMIT_SLASH_APPEAL_TX_INDEX: u32 = PHASE17_SUBMIT_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppeal`] (unattested).
pub const PHASE18_RESOLVE_SLASH_APPEAL_TX_INDEX: u32 = PHASE17_RESOLVE_SLASH_APPEAL_TX_INDEX;

/// Bincode / payload index for [`crate::TransactionPayload::ResolveSlashAppealAttested`].
pub const PHASE18_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX: u32 = 32;

/// Numerator of the fixed stake-weighted resolve quorum (⌈2W/3⌉).
pub const PHASE18_RESOLVE_THRESHOLD_NUM: u64 = PHASE17_RESOLVE_THRESHOLD_NUM;

/// Denominator of the fixed stake-weighted resolve quorum.
pub const PHASE18_RESOLVE_THRESHOLD_DEN: u64 = PHASE17_RESOLVE_THRESHOLD_DEN;

/// Domain tag for slash-appeal resolve ballots (Phase 18).
pub const SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN: &[u8] = b"BOING_SLASH_APPEAL_RESOLVE_BALLOT_V1";

/// True when `weight` meets ⌈2W/3⌉ (alias of Phase 17 integer compare).
pub fn phase18_resolve_threshold_met(weight: u128, electorate_w: u128) -> bool {
    phase17_resolve_threshold_met(weight, electorate_w)
}

/// BLAKE3(`DOMAIN || chain_id_le8 || appeal_id_le8 || slash_id_le8 || approved_u8 || voter`).
pub fn slash_appeal_resolve_ballot_preimage(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    voter: &AccountId,
) -> [u8; 32] {
    let mut h = hasher();
    h.update(SLASH_APPEAL_RESOLVE_BALLOT_DOMAIN);
    h.update(&chain_id.to_le_bytes());
    h.update(&appeal_id.to_le_bytes());
    h.update(&slash_id.to_le_bytes());
    h.update(&[u8::from(approved)]);
    h.update(&voter.0);
    *h.finalize().as_bytes()
}

/// Sign a resolve ballot with the voter's Ed25519 key. Public key must equal `voter`.
pub fn sign_slash_appeal_resolve_ballot(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    signing_key: &SigningKey,
) -> SlashAppealBallotSignature {
    let voter = AccountId(signing_key.verifying_key().to_bytes());
    let msg = slash_appeal_resolve_ballot_preimage(chain_id, appeal_id, slash_id, approved, &voter);
    SlashAppealBallotSignature(signing_key.sign(&msg).to_bytes())
}

/// Verify a resolve ballot against `voter` (AccountId = Ed25519 verifying key).
pub fn verify_slash_appeal_resolve_ballot(
    chain_id: u64,
    appeal_id: u64,
    slash_id: u64,
    approved: bool,
    voter: &AccountId,
    ballot_signature: &SlashAppealBallotSignature,
) -> Result<(), SignatureError> {
    let pk = VerifyingKey::from_bytes(&voter.0).map_err(|_| SignatureError::InvalidPublicKey)?;
    let msg = slash_appeal_resolve_ballot_preimage(chain_id, appeal_id, slash_id, approved, voter);
    let ed_sig = Ed25519Signature::from_bytes(&ballot_signature.0);
    pk.verify_strict(&msg, &ed_sig)
        .map_err(|_| SignatureError::InvalidSignature)?;
    Ok(())
}

/// Operator-facing plan strings (Phase 18 Ed25519 attested resolve + registry V3).
pub fn phase18_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase17_das_succinct_proof_plan();
    v.extend([
        "Phase 18: ResolveSlashAppealAttested (32) carries a domain-separated Ed25519 ballot over appeal_id|slash_id|approved|voter|chain_id.",
        "Phase 18: ballots accumulate on the stake-weighted vote record and are verified individually (thin attest — not BLS aggregate).",
        "Phase 18: ResolveSlashAppeal (31) remains for historical replay and unattested solo casts (ballot_signature=None).",
        "Phase 18: prefer 32 for multi-validator committee nets; stake-weighted ⌈2W/3⌉ finalize math unchanged from Phase 17.",
        "Phase 18: slash_appeals_registry.bin is V3 (magic+u32+bincode); V2/legacy Phase 14–16 loads migrate without required clear.",
        "Phase 18: true BLS aggregate of resolve ballots is still later — do not invent BLS crates or fake-BLS naming.",
        "ReservedRealV1 verify still rejects; do not flip is_implemented without a real IPA/KZG/SNARK verifier.",
    ]);
    v
}

/// Extended activation checklist (Phase 17 + Phase 18 ops). Do not rewrite live admission height.
pub fn phase18_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase17_cap_activation_checklist(activation_height);
    v.extend([
        "Phase 18: prefer ResolveSlashAppealAttested (32) for committee nets; keep 31 for solo/compat and historical blocks.",
        "Phase 18: do not clear slash_appeals_registry.bin on upgrade — node loads V3 or migrates V2 / Phase 14–16 legacy in memory and rewrites V3 on next save.",
        "Phase 18: mid-tally V2 votes survive with ballot_signature=None; new attested casts attach sigs; threshold math unchanged.",
        "Phase 18: BLS aggregate resolve is still later — this phase is Ed25519 verify-each only.",
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
        assert_eq!(PHASE18_SUBMIT_SLASH_APPEAL_TX_INDEX, 30);
        assert_eq!(PHASE18_RESOLVE_SLASH_APPEAL_TX_INDEX, 31);
        assert_eq!(PHASE18_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX, 32);
        assert_eq!(PHASE18_MAX_APPEAL_EVIDENCE_BYTES, 4096);
        assert_eq!(PHASE18_RESOLVE_THRESHOLD_NUM, 2);
        assert_eq!(PHASE18_RESOLVE_THRESHOLD_DEN, 3);
        assert_eq!(
            PHASE18_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            PHASE17_ACTIVATED_MAX_MEDIA_BLOB_BYTES
        );
    }

    #[test]
    fn ballot_sign_verify_roundtrip() {
        let key = SigningKey::generate(&mut OsRng);
        let voter = AccountId(key.verifying_key().to_bytes());
        let sig = sign_slash_appeal_resolve_ballot(6913, 1, 7, true, &key);
        assert!(verify_slash_appeal_resolve_ballot(6913, 1, 7, true, &voter, &sig).is_ok());
        let bytes = bincode::serialize(&sig).unwrap();
        assert_eq!(bytes.len(), 64);
        let back: SlashAppealBallotSignature = bincode::deserialize(&bytes).unwrap();
        assert_eq!(back, sig);
    }

    #[test]
    fn ballot_rejects_bad_sig_and_wrong_bindings() {
        let key = SigningKey::generate(&mut OsRng);
        let voter = AccountId(key.verifying_key().to_bytes());
        let sig = sign_slash_appeal_resolve_ballot(6913, 1, 7, true, &key);
        assert!(verify_slash_appeal_resolve_ballot(6913, 1, 7, false, &voter, &sig).is_err());
        assert!(verify_slash_appeal_resolve_ballot(6913, 1, 8, true, &voter, &sig).is_err());
        assert!(verify_slash_appeal_resolve_ballot(1, 1, 7, true, &voter, &sig).is_err());
        let other = AccountId([9u8; 32]);
        assert!(verify_slash_appeal_resolve_ballot(6913, 1, 7, true, &other, &sig).is_err());
        let mut bad = sig;
        bad.0[0] ^= 0xff;
        assert!(verify_slash_appeal_resolve_ballot(6913, 1, 7, true, &voter, &bad).is_err());
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase18_cap_activation_checklist(None).len() >= 28);
        let plan = phase18_das_succinct_proof_plan();
        assert!(plan.len() >= 28);
        assert!(plan
            .iter()
            .any(|s| s.contains("ReservedRealV1 verify still rejects")));
        assert!(plan.iter().any(|s| s.contains("ResolveSlashAppealAttested") || s.contains("(32)")));
        assert!(plan.iter().any(|s| s.contains("not BLS") || s.contains("still later")));
        assert!(plan.iter().any(|s| s.contains("V3") || s.contains("slash_appeals_registry")));
    }
}
