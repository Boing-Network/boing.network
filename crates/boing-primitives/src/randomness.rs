//! Verifiable randomness for leader selection (VRF / ECVRF-shaped API).
//!
//! See DECENTRALIZATION-STRATEGY.md for design. The current implementation is a
//! **deterministic BLAKE3-based stub** with an ECVRF-shaped proof layout so clients
//! and nodes can share one verify path before a production ECVRF crate is wired.

use serde::{Deserialize, Serialize};

use crate::{AccountId, Hash};

/// Domain separator for Boing VRF / leader election stubs.
const VRF_DOMAIN: &[u8] = b"boing.vrf.leader.v1\0";

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

/// Round seed hashed into the VRF message: `BLAKE3(domain || round_le)`.
pub fn vrf_round_seed(round: u64) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(VRF_DOMAIN);
    hasher.update(&round.to_le_bytes());
    *hasher.finalize().as_bytes()
}

/// Produce a deterministic VRF-shaped output for a round (dev/testnet stub).
///
/// Output = `BLAKE3(domain || "out" || round_seed)`; proof = `BLAKE3(domain || "prf" || round_seed)`
/// (32 bytes). Use [`verify_vrf_output`] to check the binding. Production will replace this with
/// real ECVRF prove/verify over Ed25519 while keeping [`leader_from_vrf`] unchanged.
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

/// Verify a [`VrfOutput`] produced by [`dummy_vrf_output`] for `round`.
///
/// Returns `true` when proof and output match the stub derivation. Always `false` for empty proofs
/// or mismatched lengths (future ECVRF proofs will use a different verifier branch).
pub fn verify_vrf_output(round: u64, vrf: &VrfOutput) -> bool {
    if vrf.proof.len() != 32 {
        return false;
    }
    let expected = dummy_vrf_output(round);
    expected.output == vrf.output && expected.proof == vrf.proof
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_vrf_is_deterministic_and_verifiable() {
        let a = dummy_vrf_output(7);
        let b = dummy_vrf_output(7);
        assert_eq!(a, b);
        assert!(verify_vrf_output(7, &a));
        assert!(!verify_vrf_output(8, &a));
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
}
