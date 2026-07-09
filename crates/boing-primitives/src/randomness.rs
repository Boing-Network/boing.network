//! Verifiable randomness for leader selection (VRF / ECVRF).
//!
//! Production path: **RFC 9381** `ECVRF-EDWARDS25519-SHA512-TAI` via `vrf-rfc9381`, using the same
//! 32-byte Ed25519 seeds / public keys as [`AccountId`] / `ed25519-dalek`.
//!
//! A deterministic BLAKE3 stub ([`dummy_vrf_output`]) remains for tests and for VRF leader mode when
//! a full set of per-validator ECVRF proofs is not yet available.

use serde::{Deserialize, Serialize};
use vrf_rfc9381::{Proof, Prover, Verifier};
use vrf_rfc9381::ec::edwards25519::tai::{
    EdVrfEdwards25519TaiPublicKey, EdVrfEdwards25519TaiSecretKey,
};

use crate::{AccountId, Hash};

/// Domain separator for Boing VRF / leader election messages (`alpha`).
const VRF_DOMAIN: &[u8] = b"boing.vrf.leader.v1\0";

/// RFC 9381 Ed25519 ECVRF `pi_string` length (Gamma || c || s).
pub const ECVRF_PROOF_LEN: usize = 80;

/// VDF output — verifiable delay function result for fair ordering.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VdfOutput {
    pub input: Hash,
    pub output: [u8; 32],
    pub proof: Vec<u8>,
}

/// VRF output — verifiable random function for leader election.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VrfOutput {
    pub proof: Vec<u8>,
    pub output: [u8; 32],
}

/// Select leader from validator set using VRF output.
/// Given validators and a VRF output, deterministically returns the elected leader.
pub fn leader_from_vrf(validators: &[AccountId], vrf_output: &VrfOutput) -> Option<AccountId> {
    if validators.is_empty() {
        return None;
    }
    let mut idx = 0u64;
    for (i, b) in vrf_output.output.iter().take(8).enumerate() {
        idx |= (*b as u64) << (i * 8);
    }
    Some(validators[(idx as usize) % validators.len()])
}

/// Among verified per-validator ECVRF outputs, pick the validator with the lexicographically
/// smallest 32-byte output (ties broken by earlier index in `validators`).
pub fn leader_from_ecvrf_proofs(
    validators: &[AccountId],
    round: u64,
    proofs: &[(AccountId, VrfOutput)],
) -> Option<AccountId> {
    if validators.is_empty() {
        return None;
    }
    let mut best: Option<(AccountId, [u8; 32])> = None;
    for v in validators {
        let Some((_, vrf)) = proofs.iter().find(|(id, _)| id == v) else {
            return None;
        };
        if !verify_ecvrf_output(v, round, vrf) {
            return None;
        }
        match &best {
            None => best = Some((*v, vrf.output)),
            Some((_, best_out)) if vrf.output.as_slice() < best_out.as_slice() => {
                best = Some((*v, vrf.output));
            }
            _ => {}
        }
    }
    best.map(|(id, _)| id)
}

/// Round seed hashed into the VRF message: `BLAKE3(domain || round_le)`.
pub fn vrf_round_seed(round: u64) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(VRF_DOMAIN);
    hasher.update(&round.to_le_bytes());
    *hasher.finalize().as_bytes()
}

/// Alpha string for ECVRF prove/verify: domain || round little-endian (same domain as the stub).
pub fn vrf_round_alpha(round: u64) -> Vec<u8> {
    let mut alpha = Vec::with_capacity(VRF_DOMAIN.len() + 8);
    alpha.extend_from_slice(VRF_DOMAIN);
    alpha.extend_from_slice(&round.to_le_bytes());
    alpha
}

fn output_from_proof_hash(hash512: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&hash512[..32]);
    out
}

/// Prove RFC 9381 ECVRF-EDWARDS25519-SHA512-TAI for `round` with a 32-byte Ed25519 secret seed.
pub fn ecvrf_prove(secret_seed: &[u8; 32], round: u64) -> Result<VrfOutput, String> {
    let prover = EdVrfEdwards25519TaiSecretKey::from_slice(secret_seed)
        .map_err(|e| format!("ecvrf prover: {e}"))?;
    let alpha = vrf_round_alpha(round);
    let proof = prover
        .prove(&alpha)
        .map_err(|e| format!("ecvrf prove: {e}"))?;
    let pi = proof.encode_to_pi();
    let hash = proof
        .proof_to_hash(vrf_rfc9381::Ciphersuite::ECVRF_EDWARDS25519_SHA512_TAI)
        .map_err(|e| format!("ecvrf proof_to_hash: {e}"))?;
    Ok(VrfOutput {
        proof: pi,
        output: output_from_proof_hash(hash.as_slice()),
    })
}

/// Verify an ECVRF [`VrfOutput`] for `round` against an Ed25519 public key ([`AccountId`]).
pub fn verify_ecvrf_output(public_key: &AccountId, round: u64, vrf: &VrfOutput) -> bool {
    if vrf.proof.len() != ECVRF_PROOF_LEN {
        return false;
    }
    let Ok(verifier) = EdVrfEdwards25519TaiPublicKey::from_slice(&public_key.0) else {
        return false;
    };
    let Ok(proof) = <EdVrfEdwards25519TaiSecretKey as Prover<_>>::Proof::decode_pi(&vrf.proof) else {
        return false;
    };
    let alpha = vrf_round_alpha(round);
    let Ok(hash) = verifier.verify(&alpha, proof) else {
        return false;
    };
    vrf.output == output_from_proof_hash(hash.as_slice())
}

/// Produce a deterministic VRF-shaped output for a round (dev/testnet stub).
///
/// Output = `BLAKE3(domain || "out" || round_seed)`; proof = `BLAKE3(domain || "prf" || round_seed)`
/// (32 bytes). Prefer [`ecvrf_prove`] / [`verify_ecvrf_output`] for production validator keys.
pub fn dummy_vrf_output(round: u64) -> VrfOutput {
    let seed = vrf_round_seed(round);
    let mut out_h = blake3::Hasher::new();
    out_h.update(VRF_DOMAIN);
    out_h.update(b"out");
    out_h.update(&seed);
    let output = *out_h.finalize().as_bytes();

    let mut prf_h = blake3::Hasher::new();
    prf_h.update(VRF_DOMAIN);
    prf_h.update(b"prf");
    prf_h.update(&seed);
    let proof = prf_h.finalize().as_bytes().to_vec();

    VrfOutput { proof, output }
}

/// Verify a stub [`VrfOutput`] from [`dummy_vrf_output`], or an ECVRF proof when `public_key` is set.
///
/// - 32-byte proof → BLAKE3 stub check (ignores `public_key`).
/// - 80-byte proof → RFC 9381 ECVRF; requires `public_key` ([`AccountId`] = Ed25519 pk).
pub fn verify_vrf_output(round: u64, vrf: &VrfOutput, public_key: Option<&AccountId>) -> bool {
    match vrf.proof.len() {
        32 => {
            let expected = dummy_vrf_output(round);
            expected.output == vrf.output && expected.proof == vrf.proof
        }
        ECVRF_PROOF_LEN => match public_key {
            Some(pk) => verify_ecvrf_output(pk, round, vrf),
            None => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    #[test]
    fn dummy_vrf_is_deterministic_and_verifiable() {
        let a = dummy_vrf_output(7);
        let b = dummy_vrf_output(7);
        assert_eq!(a, b);
        assert!(verify_vrf_output(7, &a, None));
        assert!(!verify_vrf_output(8, &a, None));
    }

    #[test]
    fn leader_from_vrf_stable() {
        let v0 = AccountId([1u8; 32]);
        let v1 = AccountId([2u8; 32]);
        let vrf = dummy_vrf_output(1);
        let a = leader_from_vrf(&[v0, v1], &vrf).unwrap();
        let b = leader_from_vrf(&[v0, v1], &vrf).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn ecvrf_prove_verify_roundtrip() {
        let sk = SigningKey::from_bytes(&[9u8; 32]);
        let pk = AccountId(sk.verifying_key().to_bytes());
        let vrf = ecvrf_prove(sk.as_bytes(), 42).unwrap();
        assert_eq!(vrf.proof.len(), ECVRF_PROOF_LEN);
        assert!(verify_ecvrf_output(&pk, 42, &vrf));
        assert!(verify_vrf_output(42, &vrf, Some(&pk)));
        assert!(!verify_ecvrf_output(&pk, 43, &vrf));
        let other = AccountId(SigningKey::from_bytes(&[8u8; 32]).verifying_key().to_bytes());
        assert!(!verify_ecvrf_output(&other, 42, &vrf));
    }

    #[test]
    fn leader_from_ecvrf_proofs_picks_min_output() {
        let k0 = SigningKey::from_bytes(&[1u8; 32]);
        let k1 = SigningKey::from_bytes(&[2u8; 32]);
        let v0 = AccountId(k0.verifying_key().to_bytes());
        let v1 = AccountId(k1.verifying_key().to_bytes());
        let round = 3u64;
        let p0 = ecvrf_prove(k0.as_bytes(), round).unwrap();
        let p1 = ecvrf_prove(k1.as_bytes(), round).unwrap();
        let proofs = vec![(v0, p0.clone()), (v1, p1.clone())];
        let leader = leader_from_ecvrf_proofs(&[v0, v1], round, &proofs).unwrap();
        let expected = if p0.output <= p1.output { v0 } else { v1 };
        assert_eq!(leader, expected);
    }
}
