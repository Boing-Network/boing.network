//! Boing node — wires consensus, execution, state, and P2P together.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use boing_consensus::ConsensusEngine;
use boing_execution::{BlockExecutor, TransactionScheduler, Vm};
use boing_governance::{SlashReason, SlashRegistry, SlashingError};
use boing_p2p::{P2pEvent, P2pNode};
use boing_primitives::{
    Account, AccountId, AccountState, Block, ConsensusVote, EquivocationEvidence, ExecutionReceipt,
    Hash, SignedTransaction, VrfProofGossip,
};
use boing_qa::pool::{PendingQaQueue, PoolError, PoolResolution, QaPoolVote};
use boing_qa::{QaPoolGovernanceConfig, RuleRegistry};
use boing_state::{ChainNativeAggregates, StateStore};
use ed25519_dalek::SigningKey;
use tokio::sync::{broadcast, mpsc};
use tracing::info;

use crate::block_producer::BlockProducer;
use crate::block_validation::{import_block, BlockValidationError};
use crate::chain::ChainState;
use crate::dapp_registry::DappRegistry;
use crate::intent_pool::IntentPool;
use crate::logging;
use crate::mempool::{Mempool, MempoolError};
use crate::persistence::{Persistence, PersistenceError};

/// Wraps ChainState to implement BlockProvider for P2P block requests.
pub struct ChainBlockProvider(pub ChainState);

impl boing_p2p::BlockProvider for ChainBlockProvider {
    fn get_block_by_hash(&self, hash: &Hash) -> Option<Block> {
        self.0.get_block_by_hash(hash)
    }
    fn get_block_by_height(&self, height: u64) -> Option<Block> {
        self.0.get_block_by_height(height)
    }
}

/// Executed proposal awaiting consensus quorum before chain append.
pub struct PendingCommit {
    pub block: Block,
    pub receipts: Vec<ExecutionReceipt>,
    pub new_state: StateStore,
}

/// Full Boing node.
#[allow(dead_code)]
pub struct BoingNode {
    pub chain: ChainState,
    pub consensus: ConsensusEngine,
    pub state: StateStore,
    pub executor: BlockExecutor,
    pub producer: BlockProducer,
    pub vm: Vm,
    pub scheduler: TransactionScheduler,
    pub mempool: Mempool,
    pub p2p: P2pNode,
    pub dapp_registry: DappRegistry,
    pub intent_pool: IntentPool,
    /// Community QA pool for deploys that return Unsure from automation.
    pub qa_pool: PendingQaQueue,
    /// Persistence backend; None for in-memory only (e.g. tests).
    pub persistence: Option<Persistence>,
    /// Execution receipts by transaction id (`tx.id()`), for RPC.
    pub receipts: HashMap<Hash, ExecutionReceipt>,
    /// Chain-wide sums over committed accounts; refreshed after state commits (see [`Self::refresh_native_aggregates`]).
    pub native_aggregates: ChainNativeAggregates,
    /// Optional broadcast of committed tip updates for WebSocket **`newHeads`** subscribers (`/ws`).
    pub head_broadcast: Option<Arc<broadcast::Sender<serde_json::Value>>>,
    /// Optional Ed25519 key for signing consensus votes (multi-validator). Local single-validator
    /// mode can omit this and still self-vote by AccountId.
    pub validator_signing_key: Option<SigningKey>,
    /// Block + post-execution state waiting for quorum.
    pub pending_commit: Option<PendingCommit>,
    /// Votes that arrived before the matching proposal (round, block_hash) → voters.
    pub early_votes: HashMap<(u64, Hash), Vec<AccountId>>,
    /// When set, refresh the consensus validator set from `top_stakers(n)` every `epoch_len` blocks.
    pub stake_validator_set: Option<StakeValidatorSetConfig>,
    /// Rounds already slashed for a validator → slash registry id (dedupe local + gossiped evidence).
    pub slashed_equivocations: HashMap<(AccountId, u64), u64>,
    /// Last accepted signed vote per (round, validator) — used to build gossip evidence on conflict.
    pub observed_votes: HashMap<(u64, AccountId), ConsensusVote>,
    /// Auditable slash / appeal registry (in-memory MVP; not yet persisted).
    pub slash_registry: SlashRegistry,
    /// Consecutive leader-timeout observations per validator (multi-validator liveness).
    pub liveness_miss_streak: HashMap<AccountId, u32>,
    /// When the local node started waiting for the current round's leader (non-leader path).
    pub leader_wait_started: Option<(u64, Instant)>,
}

/// Result of recording a vote and resolving the pool item when possible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QaPoolVoteResult {
    Pending,
    Rejected,
    /// Pool allowed; transaction was inserted into the mempool.
    AllowedAdmitted,
    /// Pool allowed but the tx was already in the mempool (duplicate).
    AllowedAlreadyInMempool,
    /// Pool allowed but mempool rejected insertion (e.g. pending limit).
    AllowedMempoolFailed(String),
}

/// Default QA pool for tests / dev node (open voting, generous caps). Production uses [`QaPoolGovernanceConfig::production_default`] via governance.
pub fn pending_qa_pool_default() -> PendingQaQueue {
    PendingQaQueue::from_governance_config(QaPoolGovernanceConfig::development_default())
}

/// Opt-in stake-derived validator set (epoch refresh from `StateStore::top_stakers`).
#[derive(Clone, Debug)]
pub struct StakeValidatorSetConfig {
    /// How many top stakers form the set (at least 1).
    pub top_n: usize,
    /// Refresh when `height % epoch_len == 0` after a commit (height ≥ epoch_len).
    pub epoch_len: u64,
    /// Minimum active `stake` to enter the set (default [`boing_tokenomics::MIN_VALIDATOR_STAKE`]).
    pub min_stake: u128,
}

impl BoingNode {
    /// Create a node with inert P2P (for tests). Single-validator default.
    pub fn new() -> Self {
        Self::with_validators(vec![AccountId([1u8; 32])], AccountId([1u8; 32]), None)
    }

    /// Create a node with a static validator set and local proposer identity.
    pub fn with_validators(
        validators: Vec<AccountId>,
        local_validator: AccountId,
        signing_key: Option<SigningKey>,
    ) -> Self {
        assert!(!validators.is_empty(), "validator set must be non-empty");
        assert!(
            validators.contains(&local_validator),
            "local validator must be in the validator set"
        );
        if let Some(ref key) = signing_key {
            let derived = AccountId(key.verifying_key().to_bytes());
            assert_eq!(
                derived, local_validator,
                "validator signing key must match local_validator AccountId"
            );
        }

        let genesis = ChainState::genesis(local_validator);
        let chain = ChainState::from_genesis(genesis.clone());
        let mut consensus = ConsensusEngine::new(validators);
        let _ = consensus.propose_and_commit(genesis);

        let mut state = StateStore::new();
        state.insert(Account {
            id: local_validator,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0, ..Default::default() },
        });
        let native_aggregates = state.compute_native_aggregates();

        let qa_registry = RuleRegistry::new();

        Self {
            chain,
            consensus,
            state,
            executor: BlockExecutor::with_qa_registry(qa_registry.clone()),
            producer: BlockProducer::new(local_validator).with_max_txs(100),
            vm: Vm::with_qa_registry(qa_registry.clone()),
            scheduler: TransactionScheduler::new(),
            mempool: Mempool::new().with_qa_registry(qa_registry),
            p2p: P2pNode::default(),
            dapp_registry: DappRegistry::new(),
            intent_pool: IntentPool::new(),
            qa_pool: pending_qa_pool_default(),
            persistence: None,
            receipts: HashMap::new(),
            native_aggregates,
            head_broadcast: None,
            validator_signing_key: signing_key,
            pending_commit: None,
            early_votes: HashMap::new(),
            stake_validator_set: None,
            slashed_equivocations: HashMap::new(),
            observed_votes: HashMap::new(),
            slash_registry: SlashRegistry::new(),
            liveness_miss_streak: HashMap::new(),
            leader_wait_started: None,
        }
    }

    /// Notify WebSocket **`newHeads`** subscribers of the current committed tip (no-op if [`Self::head_broadcast`] is unset).
    pub fn emit_head_subscriber_event(&self) {
        let Some(tx) = &self.head_broadcast else {
            return;
        };
        let height = self.chain.height();
        let hash = self.chain.latest_hash();
        let _ = tx.send(serde_json::json!({
            "type": "newHead",
            "height": height,
            "hash": format!("0x{}", hex::encode(hash.0)),
        }));
    }

    /// Recompute [`ChainNativeAggregates`] from committed `state` (O(account count); call after state commits).
    pub fn refresh_native_aggregates(&mut self) {
        self.native_aggregates = self.state.compute_native_aggregates();
    }

    /// Create a node with optional data directory for persistence.
    /// If data_dir is Some and contains persisted data, loads from disk. Otherwise starts fresh.
    pub fn with_data_dir(
        data_dir: Option<impl AsRef<std::path::Path>>,
    ) -> Result<Self, PersistenceError> {
        Self::with_data_dir_and_validators(
            data_dir,
            vec![AccountId([1u8; 32])],
            AccountId([1u8; 32]),
            None,
        )
    }

    /// Like [`Self::with_data_dir`] with an explicit validator set / local identity.
    pub fn with_data_dir_and_validators(
        data_dir: Option<impl AsRef<std::path::Path>>,
        validators: Vec<AccountId>,
        local_validator: AccountId,
        signing_key: Option<SigningKey>,
    ) -> Result<Self, PersistenceError> {
        let mut node = Self::with_validators(validators, local_validator, signing_key);

        if let Some(ref path) = data_dir {
            let path = path.as_ref();
            let persistence = Persistence::new(path);
            if let Some(genesis) = node.chain.get_block_by_height(0) {
                if persistence.save_genesis_if_missing(&genesis)? {
                    tracing::info!(
                        "Persistence: wrote missing genesis 0.bin so restarts can reload the chain"
                    );
                }
            }

            if persistence.has_persisted_data() {
                if let Some(chain) = persistence.load_chain()? {
                    node.chain = chain;
                }
                if let Some(state) = persistence.load_state()? {
                    node.state = state;
                }
                let height = node.chain.height();
                node.consensus.sync_round(height.saturating_add(1));
                for h in 0..=height {
                    if let Some(list) = persistence.load_receipts_for_height(h)? {
                        for r in list {
                            node.receipts.insert(r.tx_id, r);
                        }
                    }
                }
            }

            node.persistence = Some(persistence);

            let (load_reg, load_pool, load_slash) = {
                let p = node.persistence.as_ref().expect("just set");
                (
                    p.load_qa_registry()?,
                    p.load_qa_pool_config()?,
                    p.load_slash_registry()?,
                )
            };
            if load_reg.is_some() || load_pool.is_some() {
                let reg = load_reg.unwrap_or_else(|| node.mempool.qa_registry().clone());
                let pool = load_pool.unwrap_or_else(QaPoolGovernanceConfig::development_default);
                node.apply_qa_policy_without_persist(reg, pool);
            }
            if let Some(slash_reg) = load_slash {
                node.slash_registry = slash_reg;
                node.rebuild_slashed_equivocations_from_registry();
            }
        }

        node.refresh_native_aggregates();
        Ok(node)
    }

    /// Rebuild `(validator, round) → slash_id` dedupe map from persisted equivocation records.
    fn rebuild_slashed_equivocations_from_registry(&mut self) {
        self.slashed_equivocations.clear();
        for slash in self.slash_registry.list_slashes() {
            if matches!(slash.reason, SlashReason::Equivocation) {
                self.slashed_equivocations
                    .insert((AccountId(slash.validator), slash.block_height), slash.id);
            }
        }
    }

    fn persist_slash_registry(&self) {
        if let Some(ref p) = self.persistence {
            if let Err(e) = p.save_slash_registry(&self.slash_registry) {
                logging::log_persistence_warn("save_slash_registry", &e);
            }
        }
    }

    /// Apply QA registry + pool config without writing disk (used when loading from persistence).
    fn apply_qa_policy_without_persist(
        &mut self,
        registry: RuleRegistry,
        pool_config: QaPoolGovernanceConfig,
    ) {
        self.mempool.set_qa_registry(registry.clone());
        self.executor = BlockExecutor::with_qa_registry(registry.clone());
        self.vm = Vm::with_qa_registry(registry);
        self.qa_pool.set_governance_config(pool_config);
    }

    /// Set QA rules and pool governance together; persists to `qa_registry.json` / `qa_pool_config.json` when `data_dir` is configured.
    pub fn set_qa_policy(&mut self, registry: RuleRegistry, pool_config: QaPoolGovernanceConfig) {
        self.apply_qa_policy_without_persist(registry.clone(), pool_config.clone());
        if let Some(ref p) = self.persistence {
            if let Err(e) = p.save_qa_registry(&registry) {
                logging::log_persistence_warn("save_qa_registry", &e);
            }
            if let Err(e) = p.save_qa_pool_config(&pool_config) {
                logging::log_persistence_warn("save_qa_pool_config", &e);
            }
        }
    }

    /// Create a node with live P2P. Returns the node and a receiver for incoming blocks/txs/votes.
    pub fn with_p2p(
        p2p_listen: &str,
        data_dir: Option<impl AsRef<std::path::Path>>,
        max_connections_per_ip: u32,
    ) -> Result<(Self, mpsc::UnboundedReceiver<P2pEvent>), boing_p2p::P2pError> {
        Self::with_p2p_and_validators(
            p2p_listen,
            data_dir,
            max_connections_per_ip,
            vec![AccountId([1u8; 32])],
            AccountId([1u8; 32]),
            None,
        )
    }

    /// Like [`Self::with_p2p`] with an explicit validator set.
    pub fn with_p2p_and_validators(
        p2p_listen: &str,
        data_dir: Option<impl AsRef<std::path::Path>>,
        max_connections_per_ip: u32,
        validators: Vec<AccountId>,
        local_validator: AccountId,
        signing_key: Option<SigningKey>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<P2pEvent>), boing_p2p::P2pError> {
        let mut node =
            Self::with_data_dir_and_validators(data_dir, validators, local_validator, signing_key)
                .map_err(|e| boing_p2p::P2pError::Network(e.to_string()))?;
        let chain = node.chain.clone();
        let (p2p, event_rx) = P2pNode::new(
            p2p_listen,
            Some(std::sync::Arc::new(ChainBlockProvider(chain))),
            max_connections_per_ip,
        )?;
        node.p2p = p2p;
        Ok((node, event_rx))
    }

    fn persist_block_and_state(
        &self,
        block: &boing_primitives::Block,
        receipts: &[ExecutionReceipt],
    ) {
        if let Some(ref p) = self.persistence {
            if let Err(e) = p.save_block(block) {
                logging::log_persistence_warn("save_block", &e);
            }
            if let Err(e) = p.save_receipts(block.header.height, receipts) {
                logging::log_persistence_warn("save_receipts", &e);
            }
            if let Err(e) = p.save_chain_meta(block.header.height, block.hash()) {
                logging::log_persistence_warn("save_chain_meta", &e);
            }
            if let Err(e) = p.save_state(&self.state) {
                logging::log_persistence_warn("save_state", &e);
            }
        }
    }

    /// Append a quorum-committed block: update tip state, receipts, persistence, and round.
    fn commit_pending(&mut self, pending: PendingCommit) -> Hash {
        let hash = pending.block.hash();
        let height = pending.block.header.height;
        self.state = pending.new_state;
        self.chain
            .append(pending.block.clone())
            .expect("pending block must chain to tip");
        for r in &pending.receipts {
            self.receipts.insert(r.tx_id, r.clone());
        }
        self.persist_block_and_state(&pending.block, &pending.receipts);
        self.pending_commit = None;
        self.early_votes
            .retain(|(r, _), _| *r >= height.saturating_add(1));
        self.refresh_native_aggregates();
        self.emit_head_subscriber_event();
        self.maybe_refresh_stake_validators(height);
        // Successful commit clears leader-timeout accounting for this height's proposer.
        self.liveness_miss_streak
            .insert(pending.block.header.proposer, 0);
        self.leader_wait_started = None;
        info!("Block committed: height={} hash={:?}", height, hash);
        self.maybe_publish_vrf_proof_for_current_round();
        hash
    }

    /// Prove + gossip local ECVRF for the current consensus round when VRF leader mode is on.
    pub fn maybe_publish_vrf_proof_for_current_round(&mut self) {
        if self.consensus.leader_election() != boing_consensus::LeaderElection::Vrf {
            return;
        }
        let Some(ref key) = self.validator_signing_key else {
            return;
        };
        let round = self.consensus.round();
        let Ok(proof) = VrfProofGossip::prove(round, key) else {
            return;
        };
        if !self
            .consensus
            .insert_vrf_proof(proof.validator, proof.round, proof.vrf.clone())
        {
            return;
        }
        let _ = self.p2p.broadcast_vrf_proof(&proof);
    }

    /// Apply a verified gossiped ECVRF proof (P2P edge already verified).
    pub fn on_vrf_proof(&mut self, proof: VrfProofGossip) -> bool {
        if !proof.verify() {
            return false;
        }
        self.consensus
            .insert_vrf_proof(proof.validator, proof.round, proof.vrf)
    }

    /// Refresh consensus validators from top stakers when configured and at an epoch boundary.
    fn maybe_refresh_stake_validators(&mut self, committed_height: u64) {
        let Some(cfg) = self.stake_validator_set.clone() else {
            return;
        };
        if cfg.epoch_len == 0 || cfg.top_n == 0 {
            return;
        }
        if committed_height < cfg.epoch_len || committed_height % cfg.epoch_len != 0 {
            return;
        }
        let mut next: Vec<AccountId> = self
            .state
            .top_stakers(cfg.top_n.saturating_mul(4).max(cfg.top_n))
            .into_iter()
            .filter(|id| {
                self.state
                    .get(id)
                    .map(|a| a.stake >= cfg.min_stake)
                    .unwrap_or(false)
            })
            .take(cfg.top_n)
            .collect();
        if next.is_empty() {
            // Nobody meets min stake — keep current set.
            return;
        }
        // Ensure local identity remains in the set so this node can still vote/produce.
        let local = self.producer.proposer();
        if !next.contains(&local) {
            next.push(local);
        }
        if next.as_slice() == self.consensus.validators() {
            return;
        }
        info!(
            "Stake validator set refresh at height={}: n={}",
            committed_height,
            next.len()
        );
        self.consensus.set_validators(next);
        self.maybe_publish_vrf_proof_for_current_round();
    }

    /// Apply any buffered votes for the current pending proposal; returns commit hash if quorum hit.
    fn drain_early_votes_for_pending(&mut self, block_hash: Hash) -> Option<Hash> {
        let round = self.consensus.round();
        let voters = self
            .early_votes
            .remove(&(round, block_hash))
            .unwrap_or_default();
        for voter in voters {
            match self.consensus.vote(block_hash, voter) {
                Ok(Some(h)) => {
                    if let Some(pending) = self.pending_commit.take() {
                        if pending.block.hash() == h {
                            return Some(self.commit_pending(pending));
                        }
                        self.pending_commit = Some(pending);
                    }
                    return None;
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }
        None
    }

    /// Cast a local vote for the pending block; broadcast a signed vote when a key is configured.
    /// Returns the committed hash when quorum is reached.
    fn vote_locally_and_maybe_broadcast(&mut self, block_hash: Hash) -> Option<Hash> {
        let voter = self.producer.proposer();
        let round = self.consensus.round();
        match self.consensus.vote(block_hash, voter) {
            Ok(committed) => {
                if let Some(ref key) = self.validator_signing_key {
                    let vote = ConsensusVote::sign(round, block_hash, key);
                    self.note_signed_vote_and_maybe_slash(&vote);
                    let _ = self.p2p.broadcast_vote(&vote);
                }
                if let Some(h) = committed {
                    if let Some(pending) = self.pending_commit.take() {
                        if pending.block.hash() == h {
                            return Some(self.commit_pending(pending));
                        }
                        // Hash mismatch — restore pending (should not happen).
                        self.pending_commit = Some(pending);
                    }
                }
                None
            }
            Err(e) => {
                boing_telemetry::component_warn(
                    "boing_node::node",
                    "consensus",
                    "local_vote_failed",
                    e,
                );
                None
            }
        }
    }

    /// Apply a verified consensus vote from the network. Commits when quorum is reached.
    pub fn on_consensus_vote(&mut self, vote: ConsensusVote) -> Option<Hash> {
        if vote.verify().is_err() {
            return None;
        }
        if !self.consensus.validators().contains(&vote.validator) {
            return None;
        }
        // Detect conflicting signed votes even before / without a matching pending proposal.
        self.note_signed_vote_and_maybe_slash(&vote);

        // Buffer votes that arrive before we have a matching pending proposal.
        let have_pending = self
            .pending_commit
            .as_ref()
            .map(|p| p.block.hash() == vote.block_hash)
            .unwrap_or(false);
        if !have_pending || self.consensus.round() != vote.round {
            if self.consensus.round() == vote.round
                || self.consensus.pending_block().is_none()
            {
                self.early_votes
                    .entry((vote.round, vote.block_hash))
                    .or_default()
                    .push(vote.validator);
            }
            return None;
        }
        match self.consensus.vote(vote.block_hash, vote.validator) {
            Ok(Some(h)) => {
                if let Some(pending) = self.pending_commit.take() {
                    if pending.block.hash() == h {
                        return Some(self.commit_pending(pending));
                    }
                    self.pending_commit = Some(pending);
                }
                None
            }
            Ok(None) => None,
            Err(boing_consensus::ConsensusError::Equivocation { validator, round }) => {
                // Slash already attempted via note_signed_vote; ensure local path still fires.
                let _ = self.apply_equivocation_slash(validator, round);
                None
            }
            Err(e) => {
                boing_telemetry::component_warn(
                    "boing_node::node",
                    "consensus",
                    "peer_vote_failed",
                    e,
                );
                None
            }
        }
    }

    /// Burn a fraction of the offender's active stake (local + gossiped evidence; deduped per round).
    /// Records an auditable slash with an appeal window when first applied.
    fn apply_equivocation_slash(&mut self, validator: AccountId, round: u64) -> bool {
        let key = (validator, round);
        if self.slashed_equivocations.contains_key(&key) {
            return false;
        }
        let burned = boing_tokenomics::slash_equivocation_stake(&mut self.state, &validator);
        let slash_id = self.slash_registry.record_slash(
            validator.0,
            burned,
            SlashReason::Equivocation,
            round,
            boing_tokenomics::EQUIVOCATION_APPEAL_WINDOW_BLOCKS,
        );
        self.slashed_equivocations.insert(key, slash_id);
        self.persist_slash_registry();
        if burned > 0 {
            self.refresh_native_aggregates();
            info!(
                "Equivocation slash: validator={} round={} burned={} slash_id={}",
                hex::encode(validator.0),
                round,
                burned,
                slash_id
            );
            true
        } else {
            boing_telemetry::component_warn(
                "boing_node::node",
                "consensus",
                "equivocation_no_stake",
                format!("validator={:?} round={round} slash_id={slash_id}", validator),
            );
            true // still mark as processed so we do not retry forever
        }
    }

    /// Submit an appeal for a recorded slash (must be within the appeal window at current chain height).
    pub fn submit_slash_appeal(
        &mut self,
        slash_id: u64,
        evidence: Vec<u8>,
    ) -> Result<u64, SlashingError> {
        let id = self
            .slash_registry
            .submit_appeal(slash_id, evidence, self.chain.height())?;
        self.persist_slash_registry();
        Ok(id)
    }

    /// Resolve a pending appeal. If approved, restores burned stake from the fee burn sink.
    /// Returns the amount restored (0 if rejected or nothing to restore).
    pub fn resolve_slash_appeal(
        &mut self,
        appeal_id: u64,
        approved: bool,
    ) -> Result<u128, SlashingError> {
        self.slash_registry.resolve_appeal(appeal_id, approved)?;
        self.persist_slash_registry();
        if !approved {
            return Ok(0);
        }
        let Some(appeal) = self.slash_registry.get_appeal(appeal_id) else {
            return Err(SlashingError::AppealNotFound);
        };
        let slash_id = appeal.slash_id;
        let Some(slash) = self.slash_registry.get_slash(slash_id) else {
            return Err(SlashingError::SlashNotFound);
        };
        let validator = AccountId(slash.validator);
        let amount = slash.amount;
        let restored =
            boing_tokenomics::restore_equivocation_slash(&mut self.state, &validator, amount);
        if restored > 0 {
            self.refresh_native_aggregates();
            info!(
                "Slash appeal approved: slash_id={} appeal_id={} restored={}",
                slash_id, appeal_id, restored
            );
        }
        Ok(restored)
    }

    /// Apply verified gossiped equivocation evidence (slash + optional rebroadcast).
    pub fn on_equivocation_evidence(&mut self, evidence: EquivocationEvidence) -> bool {
        if evidence.verify().is_err() {
            return false;
        }
        if !self.consensus.validators().contains(&evidence.validator()) {
            return false;
        }
        let applied = self.apply_equivocation_slash(evidence.validator(), evidence.round());
        if applied {
            let _ = self.p2p.broadcast_equivocation(&evidence);
        }
        applied
    }

    /// Remember a signed vote; if it conflicts with a prior vote, slash and gossip evidence.
    fn note_signed_vote_and_maybe_slash(&mut self, vote: &ConsensusVote) {
        let key = (vote.round, vote.validator);
        if let Some(prev) = self.observed_votes.get(&key) {
            if prev.block_hash != vote.block_hash {
                if let Ok(ev) =
                    EquivocationEvidence::try_from_votes(prev.clone(), vote.clone())
                {
                    if self.apply_equivocation_slash(ev.validator(), ev.round()) {
                        let _ = self.p2p.broadcast_equivocation(&ev);
                    }
                }
            }
            return;
        }
        self.observed_votes.insert(key, vote.clone());
    }

    /// Validate a network block, enter voting, self-vote, and commit on quorum.
    /// Does **not** append until consensus quorum (unlike the former immediate-import path).
    pub fn handle_network_block(
        &mut self,
        block: &boing_primitives::Block,
    ) -> Result<Option<Hash>, BlockValidationError> {
        let (latest_hash, height) = (self.chain.latest_hash(), self.chain.height());
        let (new_state, receipts) = import_block(
            block,
            latest_hash,
            height,
            &self.state,
            &self.consensus,
            &self.executor,
        )?;

        if let Err(e) = self.consensus.accept_proposal(block.clone()) {
            return Err(BlockValidationError::ExecutionFailed(e.to_string()));
        }

        self.pending_commit = Some(PendingCommit {
            block: block.clone(),
            receipts,
            new_state,
        });

        let block_hash = block.hash();
        if let Some(h) = self.vote_locally_and_maybe_broadcast(block_hash) {
            return Ok(Some(h));
        }
        Ok(self.drain_early_votes_for_pending(block_hash))
    }

    /// Catch-up import for blocks fetched via block-sync (votes for that round may already be gone).
    /// Validates and appends immediately, then aligns the consensus round.
    pub fn import_network_block_catchup(
        &mut self,
        block: &boing_primitives::Block,
    ) -> Result<(), BlockValidationError> {
        let (latest_hash, height) = (self.chain.latest_hash(), self.chain.height());
        let (new_state, receipts) = import_block(
            block,
            latest_hash,
            height,
            &self.state,
            &self.consensus,
            &self.executor,
        )?;
        self.state = new_state;
        self.chain
            .append(block.clone())
            .expect("block chains (validated by import_block)");
        self.consensus
            .sync_round(block.header.height.saturating_add(1));
        self.pending_commit = None;
        for r in &receipts {
            self.receipts.insert(r.tx_id, r.clone());
        }
        self.persist_block_and_state(block, &receipts);
        self.refresh_native_aggregates();
        self.emit_head_subscriber_event();
        self.maybe_refresh_stake_validators(block.header.height);
        self.maybe_publish_vrf_proof_for_current_round();
        Ok(())
    }

    /// Import a gossiped block via the quorum path ([`Self::handle_network_block`]).
    pub fn import_network_block(
        &mut self,
        block: &boing_primitives::Block,
    ) -> Result<(), BlockValidationError> {
        self.handle_network_block(block).map(|_| ())
    }

    /// Submit a signed intent for solver fulfillment.
    pub fn submit_intent(
        &self,
        signed: boing_primitives::SignedIntent,
    ) -> Result<boing_primitives::Hash, crate::intent_pool::IntentPoolError> {
        self.intent_pool.submit(signed)
    }

    /// Dry-run execution fee vs committed balance so underfunded deploys are rejected at submit
    /// instead of stalling block production.
    fn precheck_sender_can_pay_fee(&self, signed: &SignedTransaction) -> Result<(), MempoolError> {
        if self.mempool.contains_tx_id(&signed.tx.id()) {
            return Err(MempoolError::Duplicate);
        }
        let have = self
            .state
            .get(&signed.tx.sender)
            .map(|s| s.balance)
            .unwrap_or(0);
        let mut state_copy = self.state.snapshot();
        let height = self.chain.height();
        let ts = self
            .chain
            .get_block_by_height(height)
            .map(|b| b.header.timestamp)
            .unwrap_or(0);
        let vm = Vm::with_qa_registry(self.mempool.qa_registry().clone());
        let exec_ctx = boing_execution::VmExecutionContext {
            block_height: height.saturating_add(1),
            block_timestamp: ts,
        };
        if let Ok(out) = vm.execute_with_context(&signed.tx, &mut state_copy, exec_ctx) {
            let need = boing_tokenomics::fee_for_gas(out.gas_used);
            if have < need {
                return Err(MempoolError::InsufficientFee { have, need });
            }
        }
        Ok(())
    }

    /// Submit a signed transaction to the mempool.
    pub fn submit_transaction(&self, signed: SignedTransaction) -> Result<(), MempoolError> {
        self.precheck_sender_can_pay_fee(&signed)?;
        match self.mempool.insert(signed.clone()) {
            Ok(()) => Ok(()),
            Err(MempoolError::QaPendingPool(tx_hash)) => {
                let item = boing_qa::pool::PendingQaItem::from_signed(&signed)
                    .map_err(|e| MempoolError::QaPoolEnqueue(e.to_string()))?;
                match self.qa_pool.add(item) {
                    Ok(()) | Err(PoolError::Duplicate) => {}
                    Err(PoolError::PoolDisabled) => return Err(MempoolError::QaPoolDisabled),
                    Err(PoolError::PoolFull) => return Err(MempoolError::QaPoolFull),
                    Err(PoolError::DeployerCapExceeded) => {
                        return Err(MempoolError::QaPoolDeployerCap)
                    }
                    Err(e) => return Err(MempoolError::QaPoolEnqueue(e.to_string())),
                }
                Err(MempoolError::QaPendingPool(tx_hash))
            }
            Err(e) => Err(e),
        }
    }

    /// Vote on a pending QA pool item; on Allow, admits the signed tx to the mempool (skipping deploy QA).
    pub fn qa_pool_vote(
        &self,
        tx_hash: Hash,
        voter: AccountId,
        vote: QaPoolVote,
    ) -> Result<QaPoolVoteResult, PoolError> {
        self.qa_pool.vote(tx_hash, voter, vote)?;
        match self.qa_pool.resolve(tx_hash) {
            PoolResolution::Pending => Ok(QaPoolVoteResult::Pending),
            PoolResolution::Reject => Ok(QaPoolVoteResult::Rejected),
            PoolResolution::Allow(bytes) => {
                let signed: SignedTransaction =
                    bincode::deserialize(&bytes).map_err(|_| PoolError::Deserialization)?;
                match self.mempool.insert_after_pool_allow(signed) {
                    Ok(()) => Ok(QaPoolVoteResult::AllowedAdmitted),
                    Err(MempoolError::Duplicate) => Ok(QaPoolVoteResult::AllowedAlreadyInMempool),
                    Err(e) => Ok(QaPoolVoteResult::AllowedMempoolFailed(e.to_string())),
                }
            }
        }
    }

    fn apply_qa_pool_expirations(&self) {
        for (_h, res) in self.qa_pool.prune_expired() {
            if let PoolResolution::Allow(bytes) = res {
                if let Ok(signed) = bincode::deserialize::<SignedTransaction>(&bytes) {
                    let _ = self.mempool.insert_after_pool_allow(signed);
                }
            }
        }
    }

    /// Tick liveness: if this node is not the round leader, has pending mempool txs, and has waited
    /// longer than [`boing_tokenomics::LIVENESS_LEADER_TIMEOUT_SECS`] for a proposal, count a miss
    /// against the expected leader and slash at [`boing_tokenomics::LIVENESS_MISS_THRESHOLD`].
    ///
    /// Call from the validator loop alongside [`Self::produce_block_if_ready`]. No-op for
    /// single-validator sets. Local mempool non-empty is a heuristic to avoid idle false positives.
    pub fn tick_liveness(&mut self) {
        if self.consensus.num_validators() <= 1 {
            return;
        }
        if self.pending_commit.is_some() {
            self.leader_wait_started = None;
            return;
        }
        let next_height = self.chain.height().saturating_add(1);
        let leader = self.consensus.leader(next_height);
        if leader == self.producer.proposer() {
            self.leader_wait_started = None;
            return;
        }
        if self.mempool.is_empty() {
            self.leader_wait_started = None;
            return;
        }
        let now = Instant::now();
        let started = match self.leader_wait_started {
            Some((h, t)) if h == next_height => t,
            _ => {
                self.leader_wait_started = Some((next_height, now));
                return;
            }
        };
        let timeout = Duration::from_secs(boing_tokenomics::LIVENESS_LEADER_TIMEOUT_SECS);
        if now.duration_since(started) < timeout {
            return;
        }
        // Count one miss and restart the wait window for this height.
        self.leader_wait_started = Some((next_height, now));
        let streak = self
            .liveness_miss_streak
            .entry(leader)
            .and_modify(|s| *s = s.saturating_add(1))
            .or_insert(1);
        if *streak < boing_tokenomics::LIVENESS_MISS_THRESHOLD {
            return;
        }
        *streak = 0;
        let burned = boing_tokenomics::slash_liveness_stake(&mut self.state, &leader);
        let slash_id = self.slash_registry.record_slash(
            leader.0,
            burned,
            SlashReason::Liveness,
            next_height,
            boing_tokenomics::LIVENESS_APPEAL_WINDOW_BLOCKS,
        );
        self.persist_slash_registry();
        if burned > 0 {
            self.refresh_native_aggregates();
            info!(
                "Liveness slash: leader={} height={} burned={} slash_id={}",
                hex::encode(leader.0),
                next_height,
                burned,
                slash_id
            );
        }
    }

    /// Produce one block from mempool if there are pending txs and this node is the round leader.
    /// Broadcasts the proposal and self-votes; appends only when quorum is reached (immediate for
    /// single-validator).
    pub fn produce_block_if_ready(&mut self) -> Option<boing_primitives::Hash> {
        self.apply_qa_pool_expirations();
        // Do not start a new proposal while one is pending votes.
        if self.pending_commit.is_some() {
            return None;
        }

        let proposal = self.producer.produce_proposal(
            &self.chain,
            &self.mempool,
            &mut self.state,
            &self.executor,
            &self.consensus,
        )?;

        if let Err(e) = self.consensus.propose(proposal.block.clone()) {
            boing_telemetry::component_warn(
                "boing_node::node",
                "block_producer",
                "consensus_propose_failed",
                e,
            );
            return None;
        }

        let block_hash = proposal.block.hash();
        let _ = self.p2p.broadcast_block(&proposal.block);
        self.pending_commit = Some(PendingCommit {
            block: proposal.block,
            receipts: proposal.receipts,
            new_state: proposal.new_state,
        });

        if let Some(h) = self.vote_locally_and_maybe_broadcast(block_hash) {
            return Some(h);
        }
        self.drain_early_votes_for_pending(block_hash)
    }
}

impl Default for BoingNode {
    fn default() -> Self {
        Self::new()
    }
}
