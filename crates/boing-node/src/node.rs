//! Boing node — wires consensus, execution, state, and P2P together.

use std::collections::HashMap;

use boing_consensus::ConsensusEngine;
use boing_execution::{BlockExecutor, TransactionScheduler, Vm};
use boing_p2p::{P2pEvent, P2pNode};
use boing_primitives::{
    Account, AccountId, AccountState, Block, ExecutionReceipt, Hash, SignedTransaction,
};
use boing_qa::pool::{PendingQaQueue, PoolError, PoolResolution, QaPoolVote};
use boing_qa::{QaPoolGovernanceConfig, RuleRegistry};
use boing_state::StateStore;
use tokio::sync::mpsc;

use crate::block_producer::BlockProducer;
use crate::block_validation::import_block;
use crate::chain::ChainState;
use crate::dapp_registry::DappRegistry;
use crate::intent_pool::IntentPool;
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

impl BoingNode {
    /// Create a node with inert P2P (for tests).
    pub fn new() -> Self {
        let proposer = AccountId([1u8; 32]);
        let genesis = ChainState::genesis(proposer);
        let chain = ChainState::from_genesis(genesis.clone());
        let mut consensus = ConsensusEngine::single_validator(proposer);
        let _ = consensus.propose_and_commit(genesis);

        let mut state = StateStore::new();
        state.insert(Account {
            id: proposer,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
            },
        });

        // Keep mempool, BlockExecutor, and Vm in sync: all use this registry for deploy QA.
        let qa_registry = RuleRegistry::new();

        Self {
            chain,
            consensus,
            state,
            executor: BlockExecutor::with_qa_registry(qa_registry.clone()),
            producer: BlockProducer::new(proposer).with_max_txs(100),
            vm: Vm::with_qa_registry(qa_registry.clone()),
            scheduler: TransactionScheduler::new(),
            mempool: Mempool::new().with_qa_registry(qa_registry),
            p2p: P2pNode::default(),
            dapp_registry: DappRegistry::new(),
            intent_pool: IntentPool::new(),
            qa_pool: pending_qa_pool_default(),
            persistence: None,
            receipts: HashMap::new(),
        }
    }

    /// Create a node with optional data directory for persistence.
    /// If data_dir is Some and contains persisted data, loads from disk. Otherwise starts fresh.
    pub fn with_data_dir(
        data_dir: Option<impl AsRef<std::path::Path>>,
    ) -> Result<Self, PersistenceError> {
        let mut node = Self::new();

        if let Some(ref path) = data_dir {
            let path = path.as_ref();
            let persistence = Persistence::new(path);

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

            if let Some(ref p) = node.persistence {
                let load_reg = p.load_qa_registry()?;
                let load_pool = p.load_qa_pool_config()?;
                if load_reg.is_some() || load_pool.is_some() {
                    let reg = load_reg.unwrap_or_else(|| node.mempool.qa_registry().clone());
                    let pool =
                        load_pool.unwrap_or_else(QaPoolGovernanceConfig::development_default);
                    node.apply_qa_policy_without_persist(reg, pool);
                }
            }
        }

        Ok(node)
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
                tracing::warn!("Failed to save qa_registry.json: {}", e);
            }
            if let Err(e) = p.save_qa_pool_config(&pool_config) {
                tracing::warn!("Failed to save qa_pool_config.json: {}", e);
            }
        }
    }

    /// Create a node with live P2P. Returns the node and a receiver for incoming blocks/txs.
    /// Enables block request/response so peers can fetch blocks from us.
    /// When data_dir is Some, enables disk persistence.
    pub fn with_p2p(
        p2p_listen: &str,
        data_dir: Option<impl AsRef<std::path::Path>>,
    ) -> Result<(Self, mpsc::Receiver<P2pEvent>), boing_p2p::P2pError> {
        let mut node = Self::with_data_dir(data_dir)
            .map_err(|e| boing_p2p::P2pError::Network(e.to_string()))?;
        let chain = node.chain.clone();
        let (p2p, event_rx) = P2pNode::new(
            p2p_listen,
            Some(std::sync::Arc::new(ChainBlockProvider(chain))),
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
                tracing::warn!("Persistence: failed to save block: {}", e);
            }
            if let Err(e) = p.save_receipts(block.header.height, receipts) {
                tracing::warn!("Persistence: failed to save receipts: {}", e);
            }
            if let Err(e) = p.save_chain_meta(block.header.height, block.hash()) {
                tracing::warn!("Persistence: failed to save chain meta: {}", e);
            }
            if let Err(e) = p.save_state(&self.state) {
                tracing::warn!("Persistence: failed to save state: {}", e);
            }
        }
    }

    /// Import a block from the network if it chains to our tip.
    pub fn import_network_block(
        &mut self,
        block: &boing_primitives::Block,
    ) -> Result<(), crate::block_validation::BlockValidationError> {
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
        for r in &receipts {
            self.receipts.insert(r.tx_id, r.clone());
        }
        self.persist_block_and_state(block, &receipts);
        Ok(())
    }

    /// Submit a signed intent for solver fulfillment.
    pub fn submit_intent(
        &self,
        signed: boing_primitives::SignedIntent,
    ) -> Result<boing_primitives::Hash, crate::intent_pool::IntentPoolError> {
        self.intent_pool.submit(signed)
    }

    /// Submit a signed transaction to the mempool.
    pub fn submit_transaction(&self, signed: SignedTransaction) -> Result<(), MempoolError> {
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

    /// Produce one block from mempool if there are pending txs.
    /// Broadcasts the block via P2P on success.
    pub fn produce_block_if_ready(&mut self) -> Option<boing_primitives::Hash> {
        self.apply_qa_pool_expirations();
        let (hash, receipts) = self.producer.produce_block(
            &self.chain,
            &self.mempool,
            &mut self.state,
            &self.executor,
            &mut self.consensus,
        )?;
        if let Some(block) = self.chain.get_block_by_hash(&hash) {
            for r in &receipts {
                self.receipts.insert(r.tx_id, r.clone());
            }
            self.persist_block_and_state(&block, &receipts);
            let _ = self.p2p.broadcast_block(&block);
        }
        Some(hash)
    }
}

impl Default for BoingNode {
    fn default() -> Self {
        Self::new()
    }
}
