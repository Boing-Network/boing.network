//! Signed HotStuff-style consensus votes for P2P gossip.

use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::hash::{hasher, Hash};
use crate::signature::{Signature, SignatureError};
use crate::types::AccountId;

/// Domain-separated vote message prefix (BLAKE3 input).
pub const CONSENSUS_VOTE_DOMAIN: &[u8] = b"boing.consensus.vote.v1\0";

/// Signable digest: `BLAKE3(domain || round_le || block_hash)`.
pub fn consensus_vote_signable_hash(round: u64, block_hash: &Hash) -> [u8; 32] {
    let mut h = hasher();
    h.update(CONSENSUS_VOTE_DOMAIN);
    h.update(&round.to_le_bytes());
    h.update(block_hash.0.as_slice());
    *h.finalize().as_bytes()
}

/// Signed vote for a proposed block at a consensus round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusVote {
    pub round: u64,
    pub block_hash: Hash,
    /// Voter public key (must be in the validator set).
    pub validator: AccountId,
    pub signature: Signature,
}

impl ConsensusVote {
    /// Sign a vote with the validator's Ed25519 key. `validator` must match the key's public key.
    pub fn sign(round: u64, block_hash: Hash, signing_key: &SigningKey) -> Self {
        let validator = AccountId(signing_key.verifying_key().to_bytes());
        let msg = consensus_vote_signable_hash(round, &block_hash);
        let sig = signing_key.sign(&msg);
        Self {
            round,
            block_hash,
            validator,
            signature: Signature(sig.to_bytes()),
        }
    }

    /// Verify Ed25519 signature; `validator` must be the signing public key.
    pub fn verify(&self) -> Result<(), SignatureError> {
        let pk = VerifyingKey::from_bytes(&self.validator.0)
            .map_err(|_| SignatureError::InvalidPublicKey)?;
        let msg = consensus_vote_signable_hash(self.round, &self.block_hash);
        let ed_sig = Ed25519Signature::from_bytes(&self.signature.0);
        pk.verify_strict(&msg, &ed_sig)
            .map_err(|_| SignatureError::InvalidSignature)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn vote_sign_verify_roundtrip() {
        let key = SigningKey::generate(&mut OsRng);
        let hash = Hash([9u8; 32]);
        let vote = ConsensusVote::sign(3, hash, &key);
        assert!(vote.verify().is_ok());
        assert_eq!(
            vote.validator,
            AccountId(key.verifying_key().to_bytes())
        );
    }

    #[test]
    fn vote_verify_rejects_tampered_hash() {
        let key = SigningKey::generate(&mut OsRng);
        let mut vote = ConsensusVote::sign(1, Hash([1u8; 32]), &key);
        vote.block_hash = Hash([2u8; 32]);
        assert!(vote.verify().is_err());
    }
}
