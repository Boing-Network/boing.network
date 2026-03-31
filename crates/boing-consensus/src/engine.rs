//! HotStuff-style BFT consensus engine.
//!
//! Phases: Propose → Vote → Commit (when 2f+1 votes).

use std::collections::HashMap;

use tracing::{debug, info};

use boing_primitives::{AccountId, Block, Hash};

/// Consensus engine — orchestrates BFT consensus rounds.
pub struct ConsensusEngine {
    /// Validator set (AccountIds). Must have at least 1.
    validators: Vec<AccountId>,
    /// Current round number.
    round: u64,
    /// Pending block awaiting votes.
    pending_block: Option<Block>,
    /// Votes for pending block: validator → block_hash (to detect equivocation).
    votes: HashMap<AccountId, Hash>,
}

impl ConsensusEngine {
    pub fn new(validators: Vec<AccountId>) -> Self {
        assert!(!validators.is_empty(), "Consensus requires at least 1 validator");
        Self {
            validators,
            round: 0,
            pending_block: None,
            votes: HashMap::new(),
        }
    }

    /// Create a single-validator engine for local testing.
    pub fn single_validator(validator: AccountId) -> Self {
        Self::new(vec![validator])
    }

    /// Number of validators.
    pub fn num_validators(&self) -> usize {
        self.validators.len()
    }

    /// Validator set (for block import validation).
    pub fn validators(&self) -> &[AccountId] {
        &self.validators
    }

    /// Max faulty replicas (f). HotStuff tolerates f failures with n = 3f+1.
    fn f(&self) -> usize {
        (self.validators.len().saturating_sub(1)) / 3
    }

    /// Quorum size (2f+1).
    fn quorum(&self) -> usize {
        2 * self.f() + 1
    }

    /// Leader for round r (round-robin).
    pub fn leader(&self, round: u64) -> AccountId {
        let n = self.validators.len();
        self.validators[(round as usize) % n]
    }

    /// Propose a block. Enters voting phase. Only the round leader may propose.
    pub fn propose(&mut self, block: Block) -> Result<(), ConsensusError> {
        if block.header.height != self.round {
            return Err(ConsensusError::InvalidBlock(format!(
                "Block height {} != expected round {}",
                block.header.height, self.round
            )));
        }
        let expected_leader = self.leader(self.round);
        if block.header.proposer != expected_leader {
            return Err(ConsensusError::InvalidBlock(format!(
                "Proposer {:?} is not the round leader {:?}",
                block.header.proposer, expected_leader
            )));
        }
        if !self.validators.contains(&block.header.proposer) {
            return Err(ConsensusError::InvalidBlock("Proposer not in validator set".into()));
        }

        self.pending_block = Some(block.clone());
        self.votes.clear();
        info!("Consensus: round {} propose block {}", self.round, block.hash());
        Ok(())
    }

    /// Submit a vote from a validator. Returns Some(block_hash) when committed.
    /// Detects equivocation: validator voting for different blocks in same round.
    pub fn vote(&mut self, block_hash: Hash, validator: AccountId) -> Result<Option<Hash>, ConsensusError> {
        if !self.validators.contains(&validator) {
            return Err(ConsensusError::InvalidBlock("Voter not in validator set".into()));
        }
        let block = self.pending_block.as_ref().ok_or_else(|| {
            ConsensusError::InvalidBlock("No pending block to vote on".into())
        })?;
        if block.hash() != block_hash {
            if self.votes.contains_key(&validator) {
                return Err(ConsensusError::Equivocation { validator, round: self.round });
            }
            return Err(ConsensusError::InvalidBlock("Vote for wrong block hash".into()));
        }

        self.votes.insert(validator, block_hash);
        debug!("Consensus: vote from {:?}, {}/{}", validator, self.votes.len(), self.quorum());

        if self.votes.len() >= self.quorum() {
            let h = block.hash();
            info!("Consensus: committed block {} at round {}", h, self.round);
            self.round += 1;
            self.pending_block = None;
            self.votes.clear();
            return Ok(Some(h));
        }
        Ok(None)
    }

    /// Sync consensus to a given height (e.g. after importing a block from the network).
    pub fn sync_round(&mut self, height: u64) {
        self.round = height;
        self.pending_block = None;
        self.votes.clear();
    }

    /// Propose and immediately collect votes from all validators (for single-process testing).
    pub fn propose_and_commit(&mut self, block: Block) -> Result<Hash, ConsensusError> {
        self.propose(block.clone())?;
        let block_hash = block.hash();
        for v in &self.validators.clone() {
            if let Ok(Some(h)) = self.vote(block_hash, *v) {
                return Ok(h);
            }
        }
        Err(ConsensusError::InsufficientVotes)
    }
}

impl Default for ConsensusEngine {
    fn default() -> Self {
        let default_validator = AccountId([1u8; 32]);
        Self::single_validator(default_validator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{Block, BlockHeader};

    fn mk_block(height: u64, proposer: AccountId, parent: Hash) -> Block {
        Block {
            header: BlockHeader {
                parent_hash: parent,
                height,
                timestamp: 0,
                proposer,
                tx_root: Hash::ZERO,
                receipts_root: Hash::ZERO,
                state_root: Hash::ZERO,
            },
            transactions: vec![],
        }
    }

    #[test]
    fn test_propose_and_commit_single_validator() {
        let v = AccountId::from_bytes([1u8; 32]);
        let mut engine = ConsensusEngine::single_validator(v);
        let block = mk_block(0, v, Hash::ZERO);
        let h = engine.propose_and_commit(block.clone()).unwrap();
        assert_eq!(h, block.hash());
    }

    #[test]
    fn test_equivocation_detected() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let v2 = AccountId::from_bytes([2u8; 32]);
        let v3 = AccountId::from_bytes([3u8; 32]);
        let v4 = AccountId::from_bytes([4u8; 32]);
        let validators = vec![v1, v2, v3, v4];
        let mut engine = ConsensusEngine::new(validators);
        let block_a = mk_block(0, v1, Hash::ZERO);
        let block_b = mk_block(0, v1, Hash([1u8; 32])); // different parent -> different hash
        engine.propose(block_a.clone()).unwrap();
        let hash_a = block_a.hash();
        engine.vote(hash_a, v1).unwrap();
        let result = engine.vote(block_b.hash(), v1); // v1 votes for different block
        assert!(matches!(result, Err(ConsensusError::Equivocation { .. })));
    }

    #[test]
    fn test_leader_rotation() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let v2 = AccountId::from_bytes([2u8; 32]);
        let v3 = AccountId::from_bytes([3u8; 32]);
        let validators = vec![v1, v2, v3];
        let engine = ConsensusEngine::new(validators);
        assert_eq!(engine.leader(0), v1);
        assert_eq!(engine.leader(1), v2);
        assert_eq!(engine.leader(2), v3);
        assert_eq!(engine.leader(3), v1);
    }

    #[test]
    fn test_only_leader_can_propose() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let v2 = AccountId::from_bytes([2u8; 32]);
        let validators = vec![v1, v2];
        let mut engine = ConsensusEngine::new(validators);
        let block = mk_block(0, v2, Hash::ZERO); // v2 proposes but v1 is leader for round 0
        let result = engine.propose(block);
        assert!(matches!(result, Err(ConsensusError::InvalidBlock(_))));
    }

    #[test]
    fn test_propose_then_vote_commits() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let v2 = AccountId::from_bytes([2u8; 32]);
        let v3 = AccountId::from_bytes([3u8; 32]);
        let v4 = AccountId::from_bytes([4u8; 32]);
        let validators = vec![v1, v2, v3, v4]; // n=4, f=1, quorum=3
        let mut engine = ConsensusEngine::new(validators);
        let block = mk_block(0, v1, Hash::ZERO);
        engine.propose(block.clone()).unwrap();
        let block_hash = block.hash();
        assert!(engine.vote(block_hash, v1).unwrap().is_none());
        assert!(engine.vote(block_hash, v2).unwrap().is_none());
        let committed = engine.vote(block_hash, v3).unwrap();
        assert_eq!(committed, Some(block_hash));
    }

    /// Simulate 4 nodes, 1 Byzantine (v4 never votes). 3 honest nodes reach quorum.
    #[test]
    fn test_simulate_4_nodes_1_byzantine() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let v2 = AccountId::from_bytes([2u8; 32]);
        let v3 = AccountId::from_bytes([3u8; 32]);
        let v4 = AccountId::from_bytes([4u8; 32]); // Byzantine: does not vote
        let validators = vec![v1, v2, v3, v4]; // n=4, f=1, quorum=3
        let mut engine = ConsensusEngine::new(validators);

        let block = mk_block(0, v1, Hash::ZERO);
        engine.propose(block.clone()).unwrap();
        let block_hash = block.hash();

        // Honest: v1, v2, v3 vote. Byzantine v4 never votes.
        assert!(engine.vote(block_hash, v1).unwrap().is_none());
        assert!(engine.vote(block_hash, v2).unwrap().is_none());
        let committed = engine.vote(block_hash, v3).unwrap();
        assert_eq!(committed, Some(block_hash), "3 honest nodes should commit despite 1 Byzantine");
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    #[error("Not enough votes")]
    InsufficientVotes,
    #[error("Equivocation: validator {validator:?} voted for different blocks at round {round}")]
    Equivocation { validator: AccountId, round: u64 },
}
