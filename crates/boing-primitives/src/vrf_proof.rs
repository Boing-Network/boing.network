//! Gossiped per-validator ECVRF proofs for VRF leader election.

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use crate::randomness::{ecvrf_prove, verify_ecvrf_output, VrfOutput};
use crate::types::AccountId;

/// Round-scoped ECVRF proof from one validator (gossiped on `boing/vrf-proofs`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VrfProofGossip {
    pub round: u64,
    /// Must equal the Ed25519 public key used for ECVRF verify.
    pub validator: AccountId,
    pub vrf: VrfOutput,
}

impl VrfProofGossip {
    /// Prove RFC 9381 ECVRF for `round` with the validator signing seed.
    pub fn prove(round: u64, signing_key: &SigningKey) -> Result<Self, String> {
        let validator = AccountId(signing_key.verifying_key().to_bytes());
        let vrf = ecvrf_prove(signing_key.as_bytes(), round)?;
        Ok(Self {
            round,
            validator,
            vrf,
        })
    }

    /// Verify the ECVRF proof binds to [`Self::validator`] for [`Self::round`].
    pub fn verify(&self) -> bool {
        verify_ecvrf_output(&self.validator, self.round, &self.vrf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prove_verify_roundtrip() {
        let key = SigningKey::from_bytes(&[11u8; 32]);
        let g = VrfProofGossip::prove(5, &key).unwrap();
        assert!(g.verify());
        assert_eq!(g.validator, AccountId(key.verifying_key().to_bytes()));
        let mut bad = g.clone();
        bad.round = 6;
        assert!(!bad.verify());
    }
}
