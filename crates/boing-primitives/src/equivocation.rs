//! Equivocation evidence: two conflicting signed votes from the same validator/round.

use serde::{Deserialize, Serialize};

use crate::consensus_vote::ConsensusVote;
use crate::signature::SignatureError;
use crate::types::AccountId;

/// Two valid signatures from one validator for different blocks in the same round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivocationEvidence {
    pub vote_a: ConsensusVote,
    pub vote_b: ConsensusVote,
}

impl EquivocationEvidence {
    /// Build evidence when both votes share validator+round and disagree on `block_hash`.
    pub fn try_from_votes(a: ConsensusVote, b: ConsensusVote) -> Result<Self, EquivocationError> {
        if a.validator != b.validator {
            return Err(EquivocationError::ValidatorMismatch);
        }
        if a.round != b.round {
            return Err(EquivocationError::RoundMismatch);
        }
        if a.block_hash == b.block_hash {
            return Err(EquivocationError::SameBlock);
        }
        a.verify().map_err(EquivocationError::BadSignature)?;
        b.verify().map_err(EquivocationError::BadSignature)?;
        Ok(Self {
            vote_a: a,
            vote_b: b,
        })
    }

    pub fn validator(&self) -> AccountId {
        self.vote_a.validator
    }

    pub fn round(&self) -> u64 {
        self.vote_a.round
    }

    /// Re-verify both signatures and conflict constraints (for gossip edge / peers).
    pub fn verify(&self) -> Result<(), EquivocationError> {
        Self::try_from_votes(self.vote_a.clone(), self.vote_b.clone()).map(|_| ())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EquivocationError {
    #[error("votes from different validators")]
    ValidatorMismatch,
    #[error("votes from different rounds")]
    RoundMismatch,
    #[error("votes target the same block")]
    SameBlock,
    #[error("bad vote signature: {0}")]
    BadSignature(SignatureError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::Hash;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn evidence_from_conflicting_votes() {
        let key = SigningKey::generate(&mut OsRng);
        let a = ConsensusVote::sign(5, Hash([1u8; 32]), &key);
        let b = ConsensusVote::sign(5, Hash([2u8; 32]), &key);
        let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
        assert!(ev.verify().is_ok());
        assert_eq!(ev.round(), 5);
    }

    #[test]
    fn rejects_same_block() {
        let key = SigningKey::generate(&mut OsRng);
        let h = Hash([3u8; 32]);
        let a = ConsensusVote::sign(1, h, &key);
        let b = ConsensusVote::sign(1, h, &key);
        assert!(matches!(
            EquivocationEvidence::try_from_votes(a, b).unwrap_err(),
            EquivocationError::SameBlock
        ));
    }
}
