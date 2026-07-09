//! HotStuff-style BFT consensus engine.
//!
//! Phases: Propose → Vote → Commit (when 2f+1 votes).

use std::collections::HashMap;

use tracing::{debug, info};

use boing_primitives::{
    dummy_vrf_output, leader_from_ecvrf_proofs, leader_from_vrf, verify_ecvrf_output, AccountId,
    Block, Hash, VrfOutput,
};

/// How the round leader is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LeaderElection {
    /// `validators[round % n]` (default).
    #[default]
    RoundRobin,
    /// Prefer [`leader_from_ecvrf_proofs`] when a full set of verified per-validator ECVRF proofs
    /// for the round is present; otherwise fall back to shared BLAKE3 [`dummy_vrf_output`] so all
    /// nodes stay aligned while proofs are still gossiping.
    Vrf,
}

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
    /// Leader selection policy.
    leader_election: LeaderElection,
    /// Validators that voted in the most recently committed round (for liveness accounting).
    last_committed_voters: Vec<AccountId>,
    /// Verified ECVRF outputs keyed by round → validator (VRF leader mode).
    vrf_proofs: HashMap<u64, HashMap<AccountId, VrfOutput>>,
}

impl ConsensusEngine {
    pub fn new(validators: Vec<AccountId>) -> Self {
        assert!(!validators.is_empty(), "Consensus requires at least 1 validator");
        Self {
            validators,
            round: 0,
            pending_block: None,
            votes: HashMap::new(),
            leader_election: LeaderElection::RoundRobin,
            last_committed_voters: Vec::new(),
            vrf_proofs: HashMap::new(),
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

    pub fn leader_election(&self) -> LeaderElection {
        self.leader_election
    }

    pub fn set_leader_election(&mut self, mode: LeaderElection) {
        self.leader_election = mode;
        if mode != LeaderElection::Vrf {
            self.vrf_proofs.clear();
        }
    }

    /// Replace the validator set (e.g. stake-derived epoch refresh).
    ///
    /// Clears any pending proposal/votes so a mid-round set change cannot mix voters.
    /// Caller must ensure `new_validators` is non-empty and that the local node identity
    /// (if any) remains a member when continuing to produce/vote.
    pub fn set_validators(&mut self, new_validators: Vec<AccountId>) {
        assert!(
            !new_validators.is_empty(),
            "Consensus requires at least 1 validator"
        );
        self.validators = new_validators;
        self.pending_block = None;
        self.votes.clear();
        self.last_committed_voters.clear();
        self.vrf_proofs.clear();
    }

    /// Insert a verified ECVRF proof for VRF leader election. Returns `true` if stored.
    pub fn insert_vrf_proof(&mut self, validator: AccountId, round: u64, vrf: VrfOutput) -> bool {
        if self.leader_election != LeaderElection::Vrf {
            return false;
        }
        if !self.validators.contains(&validator) {
            return false;
        }
        if !verify_ecvrf_output(&validator, round, &vrf) {
            return false;
        }
        self.vrf_proofs
            .entry(round)
            .or_default()
            .insert(validator, vrf);
        // Keep current round and a small lookahead/history window.
        let keep_from = self.round.saturating_sub(1);
        self.vrf_proofs.retain(|r, _| *r >= keep_from);
        true
    }

    /// Whether a full set of ECVRF proofs for `round` is available (all validators).
    pub fn has_full_vrf_proofs(&self, round: u64) -> bool {
        let Some(map) = self.vrf_proofs.get(&round) else {
            return false;
        };
        self.validators.iter().all(|v| map.contains_key(v))
    }

    /// Max faulty replicas (f). HotStuff tolerates f failures with n = 3f+1.
    fn f(&self) -> usize {
        (self.validators.len().saturating_sub(1)) / 3
    }

    /// Quorum size (2f+1).
    fn quorum(&self) -> usize {
        2 * self.f() + 1
    }

    /// Leader for round `r`.
    pub fn leader(&self, round: u64) -> AccountId {
        match self.leader_election {
            LeaderElection::RoundRobin => {
                let n = self.validators.len();
                self.validators[(round as usize) % n]
            }
            LeaderElection::Vrf => {
                if let Some(map) = self.vrf_proofs.get(&round) {
                    let proofs: Vec<(AccountId, VrfOutput)> =
                        map.iter().map(|(id, v)| (*id, v.clone())).collect();
                    if let Some(leader) =
                        leader_from_ecvrf_proofs(&self.validators, round, &proofs)
                    {
                        return leader;
                    }
                }
                let vrf = dummy_vrf_output(round);
                leader_from_vrf(&self.validators, &vrf).expect("non-empty validator set")
            }
        }
    }

    /// Shared proposal checks: height, round-leader proposer, validator membership.
    fn validate_proposal(&self, block: &Block) -> Result<(), ConsensusError> {
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
        Ok(())
    }

    fn enter_voting_phase(&mut self, block: Block) {
        self.pending_block = Some(block.clone());
        self.votes.clear();
        info!("Consensus: round {} propose block {}", self.round, block.hash());
    }

    /// Propose a block. Enters voting phase. Only the round leader may propose.
    pub fn propose(&mut self, block: Block) -> Result<(), ConsensusError> {
        self.validate_proposal(&block)?;
        self.enter_voting_phase(block);
        Ok(())
    }

    /// Accept a proposal from the round leader (follower path).
    ///
    /// Same height / proposer-in-set / round-leader checks as [`Self::propose`], but intended for
    /// non-leader validators that received a block over the network and need to enter the voting
    /// phase without being the proposer.
    pub fn accept_proposal(&mut self, block: Block) -> Result<(), ConsensusError> {
        self.validate_proposal(&block)?;
        // If we already have this pending block, keep existing votes (idempotent re-gossip).
        if let Some(pending) = &self.pending_block {
            if pending.hash() == block.hash() {
                return Ok(());
            }
            return Err(ConsensusError::InvalidBlock(
                "Different pending block already in voting phase".into(),
            ));
        }
        self.enter_voting_phase(block);
        Ok(())
    }

    /// Pending block awaiting quorum (if any).
    pub fn pending_block(&self) -> Option<&Block> {
        self.pending_block.as_ref()
    }

    /// Current consensus round (equals next block height to propose).
    pub fn round(&self) -> u64 {
        self.round
    }

    /// Quorum size (2f+1). Exposed for tests and operator diagnostics.
    pub fn quorum_size(&self) -> usize {
        self.quorum()
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
                boing_telemetry::component_warn(
                    "boing_consensus::engine",
                    "consensus",
                    "equivocation",
                    format!("validator={validator:?} round={}", self.round),
                );
                return Err(ConsensusError::Equivocation { validator, round: self.round });
            }
            return Err(ConsensusError::InvalidBlock("Vote for wrong block hash".into()));
        }

        self.votes.insert(validator, block_hash);
        debug!("Consensus: vote from {:?}, {}/{}", validator, self.votes.len(), self.quorum());

        if self.votes.len() >= self.quorum() {
            let h = block.hash();
            info!("Consensus: committed block {} at round {}", h, self.round);
            self.last_committed_voters = self.votes.keys().copied().collect();
            self.round += 1;
            self.pending_block = None;
            self.votes.clear();
            return Ok(Some(h));
        }
        Ok(None)
    }

    /// Validators that cast a vote in the last round that reached quorum (empty if none yet).
    pub fn last_committed_voters(&self) -> &[AccountId] {
        &self.last_committed_voters
    }

    /// Align consensus `round` with the next block to propose.
    ///
    /// [`Self::propose`] requires `block.header.height == self.round`. After the chain tip is at height
    /// `H`, the next block has height `H + 1`, so pass **`H + 1`** (e.g. `chain.height() + 1` after load
    /// or `block.header.height + 1` right after appending that block).
    pub fn sync_round(&mut self, next_block_height: u64) {
        self.round = next_block_height;
        self.pending_block = None;
        self.votes.clear();
        // Catch-up / sync does not preserve live vote sets — clear so liveness is not inferred.
        self.last_committed_voters.clear();
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

    /// Follower accepts leader proposal, then three votes commit (n=4, quorum=3).
    #[test]
    fn test_accept_proposal_follower_then_quorum() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let v2 = AccountId::from_bytes([2u8; 32]);
        let v3 = AccountId::from_bytes([3u8; 32]);
        let v4 = AccountId::from_bytes([4u8; 32]);
        let validators = vec![v1, v2, v3, v4];
        let mut engine = ConsensusEngine::new(validators);
        let block = mk_block(0, v1, Hash::ZERO);
        engine.accept_proposal(block.clone()).unwrap();
        assert_eq!(engine.pending_block().map(|b| b.hash()), Some(block.hash()));
        let block_hash = block.hash();
        assert!(engine.vote(block_hash, v2).unwrap().is_none());
        assert!(engine.vote(block_hash, v3).unwrap().is_none());
        let committed = engine.vote(block_hash, v4).unwrap();
        assert_eq!(committed, Some(block_hash));
        assert!(engine.pending_block().is_none());
    }

    #[test]
    fn test_accept_proposal_rejects_non_leader() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let v2 = AccountId::from_bytes([2u8; 32]);
        let validators = vec![v1, v2];
        let mut engine = ConsensusEngine::new(validators);
        let block = mk_block(0, v2, Hash::ZERO); // v1 is leader for round 0
        let result = engine.accept_proposal(block);
        assert!(matches!(result, Err(ConsensusError::InvalidBlock(_))));
    }

    #[test]
    fn test_accept_proposal_idempotent_same_block() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let mut engine = ConsensusEngine::single_validator(v1);
        let block = mk_block(0, v1, Hash::ZERO);
        engine.accept_proposal(block.clone()).unwrap();
        engine.accept_proposal(block).unwrap(); // same hash — ok
    }

    #[test]
    fn test_set_validators_clears_pending() {
        let v1 = AccountId::from_bytes([1u8; 32]);
        let v2 = AccountId::from_bytes([2u8; 32]);
        let v3 = AccountId::from_bytes([3u8; 32]);
        let mut engine = ConsensusEngine::new(vec![v1, v2]);
        let block = mk_block(0, v1, Hash::ZERO);
        engine.propose(block.clone()).unwrap();
        assert!(engine.pending_block().is_some());
        engine.set_validators(vec![v1, v2, v3]);
        assert!(engine.pending_block().is_none());
        assert_eq!(engine.num_validators(), 3);
        assert_eq!(engine.leader(0), v1);
    }

    #[test]
    fn test_vrf_leader_uses_dummy_vrf_index() {
        let validators: Vec<_> = (1u8..=4)
            .map(|i| AccountId::from_bytes([i; 32]))
            .collect();
        let mut engine = ConsensusEngine::new(validators.clone());
        engine.set_leader_election(LeaderElection::Vrf);
        let vrf1 = boing_primitives::dummy_vrf_output(1);
        let expected1 = boing_primitives::leader_from_vrf(&validators, &vrf1).unwrap();
        assert_eq!(engine.leader(1), expected1);
        let vrf0 = boing_primitives::dummy_vrf_output(0);
        let expected0 = boing_primitives::leader_from_vrf(&validators, &vrf0).unwrap();
        assert_eq!(engine.leader(0), expected0);
        assert!(boing_primitives::verify_vrf_output(1, &vrf1, None));
    }

    #[test]
    fn test_vrf_leader_prefers_ecvrf_when_full_set() {
        use ed25519_dalek::SigningKey;

        let keys: Vec<SigningKey> = (1u8..=3)
            .map(|i| SigningKey::from_bytes(&[i; 32]))
            .collect();
        let validators: Vec<AccountId> = keys
            .iter()
            .map(|k| AccountId(k.verifying_key().to_bytes()))
            .collect();
        let mut engine = ConsensusEngine::new(validators.clone());
        engine.set_leader_election(LeaderElection::Vrf);

        let round = 7u64;
        let stub = boing_primitives::dummy_vrf_output(round);
        let stub_leader = boing_primitives::leader_from_vrf(&validators, &stub).unwrap();
        assert_eq!(engine.leader(round), stub_leader);

        let mut proofs = Vec::new();
        for k in &keys {
            let g = boing_primitives::VrfProofGossip::prove(round, k).unwrap();
            assert!(engine.insert_vrf_proof(g.validator, g.round, g.vrf.clone()));
            proofs.push((g.validator, g.vrf));
        }
        assert!(engine.has_full_vrf_proofs(round));
        let expected =
            boing_primitives::leader_from_ecvrf_proofs(&validators, round, &proofs).unwrap();
        assert_eq!(engine.leader(round), expected);
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
