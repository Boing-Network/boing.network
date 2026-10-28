//! Boing node — wires consensus, execution, state, and P2P together.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use boing_consensus::ConsensusEngine;
use boing_execution::{BlockExecutor, TransactionScheduler, Vm};
use boing_governance::{SlashReason, SlashRegistry, SlashingError};
use boing_p2p::{P2pEvent, P2pNode};
use boing_primitives::{
    phase28_register_with_pop, Account, AccountId, AccountState, Block, BlsDualKeyRegistry,
    BlsPublicKeyG1, BlsSignatureG2, ConsensusVote, EquivocationEvidence, ExecutionReceipt, Hash,
    Phase27BlsError, Phase28BlsError, QaPoolVoteKind, SignedTransaction, TransactionPayload,
    VrfProofGossip,
};
use boing_qa::pool::{PendingQaQueue, PoolError, PoolResolution, QaPoolVote};
use boing_qa::{QaPoolGovernanceConfig, RuleRegistry};
use boing_state::{ChainNativeAggregates, StateStore};
use boing_tokenomics::NetworkFeePolicy;
use ed25519_dalek::SigningKey;
use tokio::sync::{broadcast, mpsc};
use tracing::info;

use crate::block_producer::BlockProducer;
use crate::block_validation::{import_block, BlockValidationError};
use crate::chain::ChainState;
use crate::dapp_registry::DappRegistry;
use crate::intent_pool::IntentPool;
use crate::logging;
use crate::media_blob_store::MediaBlobStore;
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
    /// Open validator-set mode config (`None` = static/stake modes). See [`Self::enable_open_validator_set`].
    pub open_validator_params: Option<boing_state::OpenValidatorParams>,
    /// Open mode: how long an anchor waits for an offline scheduled leader before proposing itself.
    pub anchor_fallback_timeout: Duration,
    /// Open mode: (height, since) the local anchor has been waiting on a non-anchor leader.
    pub anchor_fallback_wait: Option<(u64, Instant)>,
    /// Round in which this node last cast its own vote (never vote twice in one round).
    pub last_local_vote_round: Option<u64>,
    /// Sidecar for L1 media blob bytes (`data/blobs/`). Required for `boing_getBlob` serving.
    pub media_blob_store: Option<MediaBlobStore>,
    /// Phase 27 host-local BLS dual-key registry (`bls_dual_key_registry.bin`; not slash V5).
    pub bls_dual_key_registry: BlsDualKeyRegistry,
}

/// Default wait before an anchor proposes in place of an offline open-validator leader.
pub const DEFAULT_ANCHOR_FALLBACK_TIMEOUT: Duration = Duration::from_secs(3);

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

pub use crate::stake_validator_set::StakeValidatorSetConfig;

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
        // The local identity may be outside the set (open-mode candidate / follower): it then only
        // follows the chain until the on-chain registry admits it.
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
        // Genesis commit only succeeds when `local` leads round 0; either way the next block is height 1.
        consensus.sync_round(1);

        let mut state = StateStore::new();
        state.insert(Account {
            id: local_validator,
            state: AccountState {
                // Local-identity dev genesis: 1,000,000 BOING in base units (chains without a network
                // history use base units from genesis).
                balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let native_aggregates = state.compute_native_aggregates();

        let qa_registry = RuleRegistry::new();

        Self {
            chain,
            consensus,
            state,
            executor: with_chain_id_rules(BlockExecutor::with_qa_registry(qa_registry.clone())),
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
            open_validator_params: None,
            anchor_fallback_timeout: DEFAULT_ANCHOR_FALLBACK_TIMEOUT,
            anchor_fallback_wait: None,
            last_local_vote_round: None,
            media_blob_store: None,
            bls_dual_key_registry: BlsDualKeyRegistry::new(),
        }
    }

    /// Turn on the `open` validator-set mode. `params.anchors` must be the static validator list all
    /// nodes were started with. Nothing changes on-chain before `params.activation_height`; the
    /// on-chain set takes over at the first epoch boundary at/after it. Call after loading persisted
    /// state (it re-applies the persisted registry to consensus, so restarts are deterministic).
    pub fn enable_open_validator_set(&mut self, params: boing_state::OpenValidatorParams) {
        self.state.set_open_validator_params(Some(params.clone()));
        self.open_validator_params = Some(params);
        self.sync_open_validator_set();
    }

    /// Apply the on-chain open validator set (anchors ∪ active) and anchor quorum to consensus.
    /// No-op before activation or when open mode is off.
    pub fn sync_open_validator_set(&mut self) {
        let Some(params) = self.open_validator_params.clone() else {
            return;
        };
        // Committed state must carry the params (they are not persisted with state).
        if self.state.open_validator_params().is_none() {
            self.state.set_open_validator_params(Some(params.clone()));
        }
        let Some(set) = self.state.open_validators().consensus_set(&params) else {
            return;
        };
        if set.as_slice() != self.consensus.validators() {
            info!(
                "Open validator set: n={} (anchors={}, open={}) local_in_set={}",
                set.len(),
                params.anchors.len(),
                self.state.open_validators().active.len(),
                set.contains(&self.producer.proposer())
            );
            self.consensus.set_validators(set);
        }
        if self.consensus.anchors() != params.anchors.as_slice() {
            self.consensus.set_anchors(params.anchors.clone());
        }
    }

    /// JSON report for `boing_getValidators`: every validator with status, uptime and bond.
    pub fn validators_report(&self) -> serde_json::Value {
        let hex = |a: &AccountId| format!("0x{}", hex::encode(a.0));
        let reg = self.state.open_validators();
        let mode = if self.open_validator_params.is_some() {
            "open"
        } else if self.stake_validator_set.is_some() {
            "stake"
        } else {
            "static"
        };
        let set = self.consensus.validators();
        let mut out = Vec::new();
        let anchors: Vec<AccountId> = self
            .open_validator_params
            .as_ref()
            .map(|p| p.anchors.clone())
            .unwrap_or_default();
        for v in set {
            if reg.records.contains_key(v) {
                continue;
            }
            let s = reg.stats(v);
            out.push(serde_json::json!({
                "id": hex(v),
                "kind": if anchors.contains(v) || self.open_validator_params.is_none() { "anchor" } else { "static" },
                "status": "active",
                "in_set": true,
                "owner": null,
                "last_jail": null,
                "bond": "0",
                "registered_at": null,
                "jailed_until": null,
                "jail_count": 0,
                "scheduled_proposals": s.scheduled,
                "missed_proposals": s.missed,
                "uptime_pct": s.uptime_pct(),
            }));
        }
        for (id, r) in &reg.records {
            let s = reg.stats(id);
            let jailed_until = match r.status {
                boing_state::ValidatorStatus::Jailed { until } => Some(until),
                _ => None,
            };
            out.push(serde_json::json!({
                "id": hex(id),
                "kind": "open",
                // Liveness at the most recent jailing (the live window is reset on jailing). Node-local
                // display data; null if this node never observed a jailing (e.g. jailed before upgrade).
                "last_jail": reg.last_jail.get(id).map(|j| serde_json::json!({
                    "jailed_at": j.jailed_at,
                    "scheduled_proposals": j.scheduled,
                    "missed_proposals": j.missed,
                    "uptime_pct": j.stats().uptime_pct(),
                })),
                "status": r.status.as_str(),
                "in_set": set.contains(id),
                "owner": hex(&r.owner),
                "bond": r.bond.to_string(),
                "bond_boing": match self.executor.denomination_at(self.chain.height()) {
                    boing_primitives::Denomination::BaseUnits => boing_primitives::format_boing(r.bond),
                    boing_primitives::Denomination::WholeBoing => r.bond.to_string(),
                },
                "registered_at": r.registered_at,
                "jailed_until": jailed_until,
                "jail_count": r.jail_count,
                "scheduled_proposals": s.scheduled,
                "missed_proposals": s.missed,
                "uptime_pct": s.uptime_pct(),
            }));
        }
        let height = self.chain.height();
        let params = self.open_validator_params.as_ref().map(|p| {
            let next_epoch = height
                .checked_div(p.epoch_len)
                .map(|e| (e + 1) * p.epoch_len);
            serde_json::json!({
                "activation_height": p.activation_height,
                "activated_at": reg.activated_at,
                // Ledger units a registration in the next block debits; `bond_boing` is the whole-BOING
                // configured price.
                "bond": self.executor.denomination_at(height.saturating_add(1)).scale(p.bond).to_string(),
                "bond_boing": p.bond.to_string(),
                "deregister_delay": p.deregister_delay,
                "epoch_len": p.epoch_len,
                "next_epoch_height": next_epoch,
                "max_open_validators": p.max_open_validators,
                "liveness_window": p.liveness_window,
                "miss_threshold_bps": p.miss_threshold_bps,
                "min_liveness_samples": p.min_liveness_samples,
                "jail_cooldown": p.jail_cooldown,
                "anchors": p.anchors.iter().map(hex).collect::<Vec<_>>(),
            })
        });
        serde_json::json!({
            "mode": mode,
            "height": height,
            "quorum": self.consensus.quorum_size(),
            "validator_count": set.len(),
            "open": params,
            "liveness_note": "Uptime counts scheduled proposals over the last liveness_window produced blocks (idle time is not counted). Votes are not recorded on-chain and do not affect jailing.",
            "validators": out,
        })
    }

    /// Open mode: whether the local anchor should now propose in place of the scheduled leader.
    fn anchor_fallback_due(&mut self) -> bool {
        let local = self.producer.proposer();
        let next_height = self.chain.height().saturating_add(1);
        if self.consensus.fallback_anchor(next_height) != Some(local) {
            self.anchor_fallback_wait = None;
            return false;
        }
        if self.consensus.leader(next_height) == local || self.mempool.is_empty() {
            self.anchor_fallback_wait = None;
            return false;
        }
        let now = Instant::now();
        match self.anchor_fallback_wait {
            Some((h, since)) if h == next_height => {
                now.duration_since(since) >= self.anchor_fallback_timeout
            }
            _ => {
                self.anchor_fallback_wait = Some((next_height, now));
                self.anchor_fallback_timeout.is_zero()
            }
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
                // Only the retention window is kept resident (older receipts stay on disk).
                let first_receipt_height = crate::receipt_retention::first_retained_height(
                    height,
                    crate::receipt_retention::retention_blocks(),
                );
                for h in first_receipt_height..=height {
                    if let Some(list) = persistence.load_receipts_for_height(h)? {
                        for r in list {
                            node.receipts.insert(r.tx_id, r);
                        }
                    }
                }
            }

            node.persistence = Some(persistence);
            match MediaBlobStore::open(path) {
                Ok(store) => node.media_blob_store = Some(store),
                Err(e) => tracing::warn!("media blob store: could not open: {e}"),
            }

            let (load_reg, load_pool, load_slash, load_fee) = {
                let p = node.persistence.as_ref().expect("just set");
                (
                    p.load_qa_registry()?,
                    p.load_qa_pool_config()?,
                    p.load_slash_registry()?,
                    p.load_network_fee_config()?,
                )
            };
            if load_reg.is_some() || load_pool.is_some() {
                let reg = load_reg.unwrap_or_else(|| node.mempool.qa_registry().clone());
                let pool = load_pool.unwrap_or_else(QaPoolGovernanceConfig::development_default);
                node.apply_qa_policy_without_persist(reg, pool);
            }
            if let Some(fee) = load_fee {
                node.apply_fee_policy_without_persist(fee);
            }
            if let Some(ref p) = node.persistence {
                if let Ok(Some(pending)) = p.load_qa_pending() {
                    node.state.load_qa_pending(pending);
                }
            }
            if let Some(slash_reg) = load_slash {
                node.slash_registry = slash_reg;
                node.rebuild_slashed_equivocations_from_registry();
            }
            if let Some(ref p) = node.persistence {
                match p.load_bls_dual_key_registry() {
                    Ok(Some(reg)) => node.bls_dual_key_registry = reg,
                    Ok(None) => {}
                    Err(e) => logging::log_persistence_warn("load_bls_dual_key_registry", &e),
                }
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

    fn persist_bls_dual_key_registry(&self) {
        if let Some(ref p) = self.persistence {
            if let Err(e) = p.save_bls_dual_key_registry(&self.bls_dual_key_registry) {
                logging::log_persistence_warn("save_bls_dual_key_registry", &e);
            }
        }
    }

    /// Register a host-local dual-key binding (curve-validated). Persists when a data dir is set.
    ///
    /// Phase 27 helper kept for tests; production RPC uses [`Self::register_bls_dual_key_with_pop`].
    pub fn register_bls_dual_key(
        &mut self,
        account: AccountId,
        bls_pubkey: BlsPublicKeyG1,
    ) -> Result<(), Phase27BlsError> {
        self.bls_dual_key_registry.register(account, bls_pubkey)?;
        self.persist_bls_dual_key_registry();
        Ok(())
    }

    /// Phase 28: register only after a valid chain-bound BLS proof-of-possession.
    pub fn register_bls_dual_key_with_pop(
        &mut self,
        chain_id: u64,
        account: AccountId,
        bls_pubkey: BlsPublicKeyG1,
        pop: &BlsSignatureG2,
    ) -> Result<(), Phase28BlsError> {
        phase28_register_with_pop(
            &mut self.bls_dual_key_registry,
            chain_id,
            account,
            bls_pubkey,
            pop,
        )?;
        self.persist_bls_dual_key_registry();
        Ok(())
    }

    /// Apply QA registry + pool config without writing disk (used when loading from persistence).
    fn apply_qa_policy_without_persist(
        &mut self,
        registry: RuleRegistry,
        pool_config: QaPoolGovernanceConfig,
    ) {
        self.mempool.set_qa_registry(registry.clone());
        let fee = self.executor.fee_policy().clone();
        let history = self.executor.execution_history().cloned();
        self.executor = with_chain_id_rules(BlockExecutor::with_policies(
            registry.clone(),
            pool_config.clone(),
            fee,
        ));
        self.executor.set_execution_history(history);
        self.vm = Vm::with_qa_policies(registry, pool_config.clone());
        self.qa_pool.set_governance_config(pool_config);
    }

    fn apply_fee_policy_without_persist(&mut self, policy: NetworkFeePolicy) {
        self.sync_mempool_optional_fees_flag(&policy);
        self.executor.set_fee_policy(policy);
    }

    /// Legacy free-pending caps only when gas is waived *and* micro-fee height allows reduced BPS.
    fn sync_mempool_optional_fees_flag(&self, policy: &NetworkFeePolicy) {
        let next = self.chain.height().saturating_add(1);
        let active = policy.execution_gas_waived() && self.executor.micro_fees_active(next);
        self.mempool.set_optional_fees_active(active);
    }

    /// Replace native fee split / extra levies; persists `network_fee_config.json` when a data dir is set.
    pub fn set_fee_policy(&mut self, policy: NetworkFeePolicy) {
        self.apply_fee_policy_without_persist(policy.clone());
        if let Some(ref p) = self.persistence {
            if let Err(e) = p.save_network_fee_config(&policy) {
                logging::log_persistence_warn("save_network_fee_config", &e);
            }
        }
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
        if let Some(ref store) = self.media_blob_store {
            if let Err(e) = store.ingest_block_txs(block.header.height, &block.transactions) {
                tracing::warn!(
                    "media blob store: ingest height {}: {e}",
                    block.header.height
                );
            } else {
                let _ = store.prune_expired(block.header.height);
            }
        }
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

    /// Drop in-memory receipts older than the retention window (see [`crate::receipt_retention`]).
    fn prune_receipts_if_due(&mut self, height: u64) {
        if crate::receipt_retention::prune_due(height) {
            let removed = crate::receipt_retention::prune(
                &mut self.receipts,
                height,
                crate::receipt_retention::retention_blocks(),
            );
            if removed > 0 {
                tracing::debug!(
                    removed,
                    kept = self.receipts.len(),
                    height,
                    "pruned in-memory receipts"
                );
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
        self.prune_receipts_if_due(height);
        self.pending_commit = None;
        self.early_votes
            .retain(|(r, _), _| *r >= height.saturating_add(1));
        self.refresh_native_aggregates();
        self.emit_head_subscriber_event();
        self.maybe_refresh_stake_validators(height);
        self.sync_open_validator_set();
        self.sync_failed_serve_slash_records();
        self.anchor_fallback_wait = None;
        // Successful commit clears leader-timeout accounting for this height's proposer.
        self.liveness_miss_streak
            .insert(pending.block.header.proposer, 0);
        self.leader_wait_started = None;
        info!("Block committed: height={} hash={:?}", height, hash);
        self.maybe_publish_vrf_proof_for_current_round();
        hash
    }

    /// Mirror consensus FailedServe `slash_id`s into the node-local SlashRegistry (Phase 12).
    /// Balance burn happens in VM settle; approved appeals restore balance via
    /// [`Self::resolve_slash_appeal`] (Phase 13).
    fn sync_failed_serve_slash_records(&mut self) {
        let mut mirrored = 0usize;
        for c in self.state.sample_serve().challenges_with_slash_id() {
            let Some(id) = c.slash_id else {
                continue;
            };
            if self.slash_registry.get_slash(id).is_some() {
                continue;
            }
            self.slash_registry.record_slash_with_id(
                id,
                c.provider.0,
                c.provider_penalty,
                SlashReason::BlobFailedServe,
                c.deadline_height.saturating_add(1),
                boing_primitives::PHASE12_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
            );
            mirrored = mirrored.saturating_add(1);
        }
        if mirrored > 0 {
            self.persist_slash_registry();
        }
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

    /// Refresh consensus validators from stake when configured and `committed_height` is an epoch
    /// boundary at/after activation (see [`crate::stake_validator_set`]). Deterministic: no node-local input.
    fn maybe_refresh_stake_validators(&mut self, committed_height: u64) {
        let Some(cfg) = self.stake_validator_set.clone() else {
            return;
        };
        if !cfg.is_refresh_height(committed_height) {
            return;
        }
        // `min_stake` is configured in whole BOING; compare in ledger units.
        let mut cfg = cfg;
        cfg.min_stake = self
            .executor
            .denomination_at(committed_height)
            .scale(cfg.min_stake);
        let next = crate::stake_validator_set::next_validator_set(
            &self.state,
            &cfg,
            self.consensus.validators(),
        );
        if let Some(ref p) = self.persistence {
            if let Err(e) = p.save_stake_validator_set(committed_height, &next) {
                logging::log_persistence_warn("save_stake_validator_set", &e);
            }
        }
        if next.as_slice() == self.consensus.validators() {
            return;
        }
        let local = self.producer.proposer();
        info!(
            "Stake validator set refresh at height={}: n={} local_in_set={}",
            committed_height,
            next.len(),
            next.contains(&local)
        );
        self.consensus.set_validators(next);
        self.maybe_publish_vrf_proof_for_current_round();
    }

    /// After a restart, re-apply the stake-derived set that was active at the tip. Uses the set persisted
    /// at the last epoch boundary; if it is missing (e.g. first start with stake mode on an advanced chain)
    /// it is recomputed from the current committed state and a warning is logged. Returns the applied set.
    pub fn restore_stake_validator_set(&mut self) -> Option<Vec<AccountId>> {
        let mut cfg = self.stake_validator_set.clone()?;
        let tip = self.chain.height();
        cfg.min_stake = self.executor.denomination_at(tip).scale(cfg.min_stake);
        let boundary = cfg.last_refresh_height(tip)?;
        let persisted = self
            .persistence
            .as_ref()
            .and_then(|p| p.load_stake_validator_set().ok().flatten())
            .filter(|(h, set)| *h == boundary && !set.is_empty())
            .map(|(_, set)| set);
        let set = match persisted {
            Some(set) => set,
            None => {
                tracing::warn!(
                    "stake validator set for boundary {} not persisted; recomputing from state at tip {} (peers must agree)",
                    boundary,
                    tip
                );
                crate::stake_validator_set::next_validator_set(
                    &self.state,
                    &cfg,
                    self.consensus.validators(),
                )
            }
        };
        if set.as_slice() != self.consensus.validators() {
            info!(
                "Restored stake validator set (boundary {}): n={}",
                boundary,
                set.len()
            );
            self.consensus.set_validators(set.clone());
        }
        Some(set)
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
        if !self.consensus.validators().contains(&voter) {
            return None; // follower / not-yet-admitted open candidate
        }
        if self.last_local_vote_round == Some(round) {
            // Already voted this round (e.g. for a proposal an anchor fallback replaced). Voting again
            // would be equivocation; anchor quorum does not need our vote.
            return None;
        }
        match self.consensus.vote(block_hash, voter) {
            Ok(committed) => {
                self.last_local_vote_round = Some(round);
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
            if self.consensus.round() == vote.round || self.consensus.pending_block().is_none() {
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
    ///
    /// Phase 15: also seeds the consensus [`boing_state::SlashAppealRegistry`] so Submit/Resolve
    /// txs (30/31) can restore stake on multi-validator replay. Detection/apply itself remains
    /// node-local (evidence gossip); validators that saw the same evidence apply the same slash
    /// (same social/ops model as today's local SlashRegistry mirror).
    fn apply_equivocation_slash(&mut self, validator: AccountId, round: u64) -> bool {
        let key = (validator, round);
        if self.slashed_equivocations.contains_key(&key) {
            return false;
        }
        let burned = boing_tokenomics::slash_equivocation_stake(&mut self.state, &validator);
        let slash_id = self
            .state
            .slash_appeals_mut()
            .record_slash(
                validator,
                burned,
                boing_state::ConsensusSlashReason::Equivocation,
                round,
                boing_tokenomics::EQUIVOCATION_APPEAL_WINDOW_BLOCKS,
            )
            .unwrap_or_else(|_| {
                // Extremely defensive: fall back to local-only id if consensus insert somehow fails.
                self.slash_registry.record_slash(
                    validator.0,
                    burned,
                    SlashReason::Equivocation,
                    round,
                    boing_tokenomics::EQUIVOCATION_APPEAL_WINDOW_BLOCKS,
                )
            });
        self.slash_registry.record_slash_with_id(
            slash_id,
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
                format!(
                    "validator={:?} round={round} slash_id={slash_id}",
                    validator
                ),
            );
            true // still mark as processed so we do not retry forever
        }
    }

    /// Submit an appeal for a recorded slash (must be within the appeal window at current chain height).
    ///
    /// **Deprecated for FailedServe / equivocation / liveness:** prefer consensus tx
    /// `SubmitSlashAppeal` (index 30). This RPC only mutates the node-local `SlashRegistry` mirror.
    pub fn submit_slash_appeal(
        &mut self,
        slash_id: u64,
        evidence: Vec<u8>,
    ) -> Result<u64, SlashingError> {
        if self.state.slash_appeals().get_slash(slash_id).is_some() {
            // Consensus book owns appeals for this slash_id; avoid dual-path confusion.
            return Err(SlashingError::AppealAlreadyExists);
        }
        let id = self
            .slash_registry
            .submit_appeal(slash_id, evidence, self.chain.height())?;
        self.persist_slash_registry();
        Ok(id)
    }

    /// Resolve a pending appeal. If approved, restores from the fee burn sink:
    /// stake for equivocation/liveness/fraud; **balance** for BlobFailedServe (Phase 13).
    /// Returns the amount restored (0 if rejected or nothing to restore).
    ///
    /// **Deprecated for FailedServe / equivocation / liveness:** prefer consensus tx
    /// `ResolveSlashAppeal` (index 31). Refuses restore when the consensus appeal book already
    /// reversed the slash (no double credit).
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
        if self.state.slash_appeals().is_reversed(slash_id) {
            // Consensus Resolve already restored — do not double-credit via operator RPC.
            return Ok(0);
        }
        let Some(slash) = self.slash_registry.get_slash(slash_id) else {
            return Err(SlashingError::SlashNotFound);
        };
        let account = AccountId(slash.validator);
        let amount = slash.amount;
        let reason = slash.reason.clone();
        let restored = match &reason {
            SlashReason::BlobFailedServe => {
                boing_tokenomics::restore_failed_serve_penalty(&mut self.state, &account, amount)
            }
            _ => boing_tokenomics::restore_equivocation_slash(&mut self.state, &account, amount),
        };
        if restored > 0 {
            self.refresh_native_aggregates();
            info!(
                "Slash appeal approved: slash_id={} appeal_id={} restored={} reason={:?}",
                slash_id, appeal_id, restored, reason
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
                if let Ok(ev) = EquivocationEvidence::try_from_votes(prev.clone(), vote.clone()) {
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
        self.prune_receipts_if_due(block.header.height);
        self.refresh_native_aggregates();
        self.emit_head_subscriber_event();
        self.maybe_refresh_stake_validators(block.header.height);
        self.sync_open_validator_set();
        self.sync_failed_serve_slash_records();
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

    /// State-free-of-clone admission gates, run before any snapshot-based dry run so that spam from
    /// throwaway keys costs a hash lookup instead of an O(state) clone + VM execution:
    ///
    /// * the sender must have an account (the VM answers `AccountNotFound` otherwise: such a tx
    ///   occupies a mempool / block slot and can neither pay a fee nor consume its nonce);
    /// * the tx nonce must not be below the account nonce (it can never execute; future nonces stay
    ///   admitted and queue behind earlier txs);
    /// * once the failed-tx fee is active, the balance must cover that fee (otherwise the tx cannot
    ///   pay for a failed execution either).
    ///
    /// Skipped at the redenomination height (balances change unit there; the full dry run handles it).
    fn precheck_sender_viable(&self, signed: &SignedTransaction) -> Result<(), MempoolError> {
        let next = self.chain.height().saturating_add(1);
        let Some(acct) = self.state.get(&signed.tx.sender) else {
            return Err(MempoolError::UnknownSender);
        };
        if signed.tx.nonce < acct.nonce {
            return Err(MempoolError::StaleNonce {
                expected: acct.nonce,
                got: signed.tx.nonce,
            });
        }
        if self.executor.redenomination_height() != Some(next)
            && self.executor.failed_tx_fee_active(next)
        {
            let (policy, _) = self.executor.fee_rule_at(next);
            let need = boing_execution::failed_tx_fee_amount_in(
                &policy,
                self.executor.denomination_at(next),
            );
            if acct.balance < need {
                return Err(MempoolError::InsufficientFee {
                    have: acct.balance,
                    need,
                });
            }
        }
        Ok(())
    }

    /// Transfer admission without a state clone. Same decision as
    /// [`Self::precheck_sender_can_pay_fee_full`] (covered by a differential test): a transfer the VM
    /// would reject for nonce / amount is admitted (it fails in-block and pays the failed-tx fee);
    /// one that would execute must leave the sender able to pay the fee + tip.
    #[doc(hidden)] // public only so the differential test in tests/admission_gates.rs can call it
    pub fn precheck_transfer_fast(
        &self,
        signed: &SignedTransaction,
        to: &AccountId,
        amount: u128,
    ) -> Result<(), MempoolError> {
        let next = self.chain.height().saturating_add(1);
        let (nonce, have) = self
            .state
            .get(&signed.tx.sender)
            .map(|s| (Some(s.nonce), s.balance))
            .unwrap_or((None, 0));
        let policy = self.executor.fee_policy();
        if signed.priority_tip == 0
            && policy.execution_gas_waived()
            && policy.free_tx_min_balance > 0
            && have < policy.free_tx_min_balance
        {
            return Err(MempoolError::FreeTxBalanceGate {
                have,
                need: policy.free_tx_min_balance,
            });
        }
        if nonce != Some(signed.tx.nonce) || have < amount {
            return Ok(()); // the VM rejects it: admitted, fails in-block
        }
        let need = self
            .executor
            .network_fee_at(boing_execution::GAS_PER_TRANSFER, &signed.tx.payload, next)
            .saturating_add(signed.priority_tip);
        let remaining = if *to == signed.tx.sender {
            have
        } else {
            have - amount
        };
        if remaining < need {
            return Err(MempoolError::InsufficientFee { have, need });
        }
        Ok(())
    }

    /// Dry-run execution fee vs committed balance so underfunded deploys are rejected at submit
    /// instead of stalling block production.
    fn precheck_sender_can_pay_fee(&self, signed: &SignedTransaction) -> Result<(), MempoolError> {
        if self.mempool.contains_tx_id(&signed.tx.id()) {
            return Err(MempoolError::Duplicate);
        }
        if let TransactionPayload::Transfer { to, amount } = &signed.tx.payload {
            if self.executor.redenomination_height() != Some(self.chain.height().saturating_add(1))
            {
                return self.precheck_transfer_fast(signed, to, *amount);
            }
        }
        self.precheck_sender_can_pay_fee_full(signed)
    }

    /// Reference (snapshot + VM dry run) form of [`Self::precheck_sender_can_pay_fee`].
    #[doc(hidden)]
    pub fn precheck_sender_can_pay_fee_full(
        &self,
        signed: &SignedTransaction,
    ) -> Result<(), MempoolError> {
        let mut state_copy = self.state.snapshot();
        let height = self.chain.height();
        self.executor
            .prepare_state_for_height(height.saturating_add(1), &mut state_copy);
        let have = state_copy
            .get(&signed.tx.sender)
            .map(|s| s.balance)
            .unwrap_or(0);
        let ts = self
            .chain
            .get_block_by_height(height)
            .map(|b| b.header.timestamp)
            .unwrap_or(0);
        let vm = self.admission_vm();
        let exec_ctx = boing_execution::VmExecutionContext {
            block_height: height.saturating_add(1),
            block_timestamp: ts,
            native_value: false, // set by the VM from the height
        };
        let policy = self.executor.fee_policy();
        // Free-path balance gate (anti-spam): zero-tip txs when execution gas is waived.
        if signed.priority_tip == 0
            && policy.execution_gas_waived()
            && policy.free_tx_min_balance > 0
            && have < policy.free_tx_min_balance
        {
            return Err(MempoolError::FreeTxBalanceGate {
                have,
                need: policy.free_tx_min_balance,
            });
        }
        if let Ok(out) = vm.execute_with_context(&signed.tx, &mut state_copy, exec_ctx) {
            let need = self
                .executor
                .network_fee_at(out.gas_used, &signed.tx.payload, height.saturating_add(1))
                .saturating_add(signed.priority_tip);
            let remaining = state_copy
                .get(&signed.tx.sender)
                .map(|s| s.balance)
                .unwrap_or(0);
            if remaining < need {
                return Err(MempoolError::InsufficientFee { have, need });
            }
        }
        Ok(())
    }

    /// Open validator txs are admitted only when open mode is configured and active at the next height,
    /// and only if they would execute (bad node signature, duplicates, wrong owner, cooldown → rejected
    /// up front instead of wasting a block slot). Keeps pre-activation chains free of the new variants.
    fn precheck_open_validator_tx(&self, signed: &SignedTransaction) -> Result<(), MempoolError> {
        if !matches!(
            signed.tx.payload,
            TransactionPayload::ValidatorRegister { .. }
                | TransactionPayload::ValidatorDeregister { .. }
                | TransactionPayload::ValidatorUnjail { .. }
        ) {
            return Ok(());
        }
        let mut state_copy = self.state.snapshot();
        self.executor
            .prepare_state_for_height(self.chain.height().saturating_add(1), &mut state_copy);
        if state_copy.open_validator_params().is_none() {
            state_copy.set_open_validator_params(self.open_validator_params.clone());
        }
        let exec_ctx = boing_execution::VmExecutionContext {
            block_height: self.chain.height().saturating_add(1),
            block_timestamp: 0,
            native_value: false, // set by the VM from the height
        };
        let vm = self.admission_vm();
        match vm.execute_with_context(&signed.tx, &mut state_copy, exec_ctx) {
            Ok(_) => Ok(()),
            Err(boing_execution::VmError::InvalidNonce { .. }) => Ok(()), // queued behind earlier txs
            Err(e) => Err(MempoolError::OpenValidatorRejected(e.to_string())),
        }
    }

    /// PutBlob / BlobAnnounce / renew / sample admitted only when media blobs are active.
    fn precheck_media_blob_tx(&self, signed: &SignedTransaction) -> Result<(), MempoolError> {
        let next = self.chain.height().saturating_add(1);
        let require_active = || -> Result<(), MempoolError> {
            if !self.executor.vm().media_blobs_active(next) {
                return Err(MempoolError::MediaBlobRejected(format!(
                    "media blobs not active (activation={:?})",
                    self.executor.vm().media_blobs_from()
                )));
            }
            Ok(())
        };
        match &signed.tx.payload {
            TransactionPayload::PutBlob {
                retention,
                content_type,
                data,
            } => {
                require_active()?;
                let max = self.executor.vm().media_blob_max_bytes(next);
                if let Err(e) = boing_primitives::validate_put_blob_with_cap(
                    *retention,
                    content_type,
                    data,
                    max,
                ) {
                    return Err(MempoolError::MediaBlobRejected(e.to_string()));
                }
            }
            TransactionPayload::BlobAnnounce {
                blob_id,
                size,
                retention,
                content_type,
            } => {
                require_active()?;
                let max = self.executor.vm().media_blob_max_bytes(next);
                if let Err(e) = boing_primitives::validate_blob_announce_with_cap(
                    blob_id,
                    *size,
                    *retention,
                    content_type,
                    max,
                ) {
                    return Err(MempoolError::MediaBlobRejected(e.to_string()));
                }
            }
            TransactionPayload::RenewMediaBlobPin {
                blob_id,
                size,
                extend_blocks,
            } => {
                require_active()?;
                if let Err(e) =
                    boing_primitives::validate_renew_media_blob_pin(blob_id, *size, *extend_blocks)
                {
                    return Err(MempoolError::MediaBlobRejected(e.to_string()));
                }
                if let Some(store) = self.media_blob_store() {
                    match store.get_meta(blob_id) {
                        Some(m) if m.size == *size => {}
                        Some(_) => {
                            return Err(MempoolError::MediaBlobRejected(
                                "renew size does not match sidecar meta".into(),
                            ));
                        }
                        None => {
                            return Err(MempoolError::MediaBlobRejected(
                                "renew target blob not in sidecar".into(),
                            ));
                        }
                    }
                }
            }
            TransactionPayload::PayBlobSample {
                blob_id,
                chunk_index,
                chunk_count,
            } => {
                require_active()?;
                let max_chunks = boing_primitives::media_blob_max_das_chunks_at(
                    next,
                    self.executor.vm().media_blobs_phase7_cap_from(),
                );
                if let Err(e) = boing_primitives::validate_pay_blob_sample_with_max_chunks(
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    max_chunks,
                ) {
                    return Err(MempoolError::MediaBlobRejected(e.to_string()));
                }
            }
            TransactionPayload::RegisterBlobSampleOffer {
                blob_id,
                chunk_index,
                chunk_count,
                ask_fee,
                expires_at_height,
            } => {
                require_active()?;
                if let Err(e) = boing_primitives::validate_register_blob_sample_offer(
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    *ask_fee,
                    *expires_at_height,
                ) {
                    return Err(MempoolError::MediaBlobRejected(e.to_string()));
                }
            }
            TransactionPayload::SettleBlobSampleMatch {
                blob_id,
                chunk_index,
                chunk_count,
                provider,
            } => {
                require_active()?;
                let max_chunks = boing_primitives::media_blob_max_das_chunks_at(
                    next,
                    self.executor.vm().media_blobs_phase7_cap_from(),
                );
                if let Err(e) = boing_primitives::validate_pay_blob_sample_with_max_chunks(
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    max_chunks,
                ) {
                    return Err(MempoolError::MediaBlobRejected(e.to_string()));
                }
                let tip = self.chain.height();
                // Prefer SMT-resident book (Phase 9); soft-fallback to sidecar replay book.
                if let Some(o) = self
                    .state
                    .sample_offers()
                    .get(blob_id, *chunk_index, provider)
                {
                    if o.chunk_count != *chunk_count {
                        return Err(MempoolError::MediaBlobRejected(
                            "settle chunk_count does not match registered offer".into(),
                        ));
                    }
                    if o.is_expired(tip) {
                        return Err(MempoolError::MediaBlobRejected(
                            "sample offer expired".into(),
                        ));
                    }
                } else if let Some(store) = self.media_blob_store() {
                    let on = store.onchain_sample_market_snapshot();
                    if let Some(o) = on.best_offer(blob_id, *chunk_index, tip) {
                        if &o.provider == provider && o.is_expired(tip) {
                            return Err(MempoolError::MediaBlobRejected(
                                "sample offer expired".into(),
                            ));
                        }
                    }
                }
            }
            TransactionPayload::CancelBlobSampleOffer {
                blob_id,
                chunk_index,
            } => {
                require_active()?;
                if self
                    .state
                    .sample_offers()
                    .get(blob_id, *chunk_index, &signed.tx.sender)
                    .is_none()
                {
                    return Err(MempoolError::MediaBlobRejected(
                        "no matching sample offer to cancel".into(),
                    ));
                }
            }
            TransactionPayload::ChallengeBlobSampleServe {
                blob_id,
                chunk_index,
                chunk_count,
                provider,
            } => {
                require_active()?;
                let max_chunks = boing_primitives::media_blob_max_das_chunks_at(
                    next,
                    self.executor.vm().media_blobs_phase7_cap_from(),
                );
                if let Err(e) = boing_primitives::validate_pay_blob_sample_with_max_chunks(
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    max_chunks,
                ) {
                    return Err(MempoolError::MediaBlobRejected(e.to_string()));
                }
                if provider == &signed.tx.sender {
                    return Err(MempoolError::MediaBlobRejected(
                        "cannot challenge own sample serve".into(),
                    ));
                }
                let bond = boing_primitives::PHASE11_SERVE_CHALLENGE_BOND;
                let penalty = boing_primitives::PHASE11_FAILED_SERVE_PENALTY;
                let challenger_bal = self
                    .state
                    .get(&signed.tx.sender)
                    .map(|s| s.balance)
                    .unwrap_or(0);
                if challenger_bal < bond {
                    return Err(MempoolError::MediaBlobRejected(
                        "insufficient balance for serve-challenge bond".into(),
                    ));
                }
                let provider_bal = self.state.get(provider).map(|s| s.balance).unwrap_or(0);
                if provider_bal < penalty {
                    return Err(MempoolError::MediaBlobRejected(
                        "provider balance below FailedServe penalty".into(),
                    ));
                }
                if !self.state.sample_offers().has_settle_receipt(
                    blob_id,
                    *chunk_index,
                    provider,
                    &signed.tx.sender,
                ) {
                    return Err(MempoolError::MediaBlobRejected(
                        "challenge requires prior SettleBlobSampleMatch".into(),
                    ));
                }
            }
            TransactionPayload::RespondBlobSampleServe {
                blob_id,
                chunk_index,
                challenger,
                ..
            } => {
                require_active()?;
                if self
                    .state
                    .sample_serve()
                    .get(blob_id, *chunk_index, &signed.tx.sender, challenger)
                    .is_none()
                {
                    return Err(MempoolError::MediaBlobRejected(
                        "no matching open serve challenge".into(),
                    ));
                }
            }
            // Phase 15: Submit/Resolve always admitted (not gated on media-blobs activation) so
            // equivocation/liveness appeals work when media is inactive.
            TransactionPayload::SubmitSlashAppeal { slash_id, evidence } => {
                if evidence.len() > boing_primitives::PHASE14_MAX_APPEAL_EVIDENCE_BYTES {
                    return Err(MempoolError::MediaBlobRejected(format!(
                        "appeal evidence exceeds max {} bytes",
                        boing_primitives::PHASE14_MAX_APPEAL_EVIDENCE_BYTES
                    )));
                }
                if self.state.slash_appeals().get_slash(*slash_id).is_none() {
                    return Err(MempoolError::MediaBlobRejected(
                        "slash record not found in consensus appeal book".into(),
                    ));
                }
            }
            TransactionPayload::ResolveSlashAppeal { .. } => {
                if !self.consensus.validators().contains(&signed.tx.sender) {
                    return Err(MempoolError::MediaBlobRejected(
                        "ResolveSlashAppeal requires a consensus validator".into(),
                    ));
                }
                // Phase 19/31/32: reject unattested 31 early when multi-pin requires attested
                // or BLS aggregate (flip active).
                let next = self.chain.height().saturating_add(1);
                let flip_from = self.executor.vm().bls_committee_flip_from();
                let pinned_len = self
                    .state
                    .slash_appeal_resolvers()
                    .map(|s| s.len())
                    .unwrap_or(0);
                if boing_primitives::phase32_committee_requires_bls_aggregate(
                    pinned_len, next, flip_from,
                ) {
                    return Err(MempoolError::MediaBlobRejected(
                        "ResolveSlashAppeal (31) rejected when BLS committee flip active and |slash_appeal_resolvers| > 1; use ResolveSlashAppealBlsAggregate (33)"
                            .into(),
                    ));
                }
                if boing_primitives::phase32_committee_requires_attested(
                    pinned_len, next, flip_from,
                ) {
                    return Err(MempoolError::MediaBlobRejected(
                        "ResolveSlashAppeal (31) rejected when |slash_appeal_resolvers| > 1; use ResolveSlashAppealAttested (32)"
                            .into(),
                    ));
                }
            }
            TransactionPayload::ResolveSlashAppealAttested { .. } => {
                if !self.consensus.validators().contains(&signed.tx.sender) {
                    return Err(MempoolError::MediaBlobRejected(
                        "ResolveSlashAppeal requires a consensus validator".into(),
                    ));
                }
                // Phase 32: when flip active, multi-pin must use BLS aggregate 33 (not Ed25519 32).
                let next = self.chain.height().saturating_add(1);
                let flip_from = self.executor.vm().bls_committee_flip_from();
                let pinned_len = self
                    .state
                    .slash_appeal_resolvers()
                    .map(|s| s.len())
                    .unwrap_or(0);
                if boing_primitives::phase32_committee_requires_bls_aggregate(
                    pinned_len, next, flip_from,
                ) {
                    return Err(MempoolError::MediaBlobRejected(
                        "ResolveSlashAppealAttested (32) rejected when BLS committee flip active and |slash_appeal_resolvers| > 1; use ResolveSlashAppealBlsAggregate (33)"
                            .into(),
                    ));
                }
            }
            TransactionPayload::ResolveSlashAppealBlsAggregate { .. } => {
                // Phase 31/32: BLS aggregate path is consensus-admitted.
                // Unset flip ⇒ optional; flip active + multi-pin ⇒ required (enforced by rejecting 31/32).
                if !boing_primitives::PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED {
                    return Err(MempoolError::MediaBlobRejected(
                        "ResolveSlashAppealBlsAggregate (33) consensus admit not wired; use ResolveSlashAppealAttested (32)"
                            .into(),
                    ));
                }
                if let Err(e) = boing_primitives::phase32_admit_bls_aggregate_resolve_tx() {
                    return Err(MempoolError::MediaBlobRejected(e.to_string()));
                }
                if !self.consensus.validators().contains(&signed.tx.sender) {
                    return Err(MempoolError::MediaBlobRejected(
                        "ResolveSlashAppealBlsAggregate requires a consensus validator submitter"
                            .into(),
                    ));
                }
            }
            TransactionPayload::GrantBlobDownload { blob_id, grantee } => {
                require_active()?;
                let Some(store) = self.media_blob_store() else {
                    return Err(MempoolError::MediaBlobRejected(
                        "media blob store unavailable".into(),
                    ));
                };
                let Some(meta) = store.get_meta(blob_id) else {
                    return Err(MempoolError::MediaBlobRejected(
                        "grant target blob not in sidecar".into(),
                    ));
                };
                if meta.publisher != signed.tx.sender.0 {
                    return Err(MempoolError::MediaBlobRejected(
                        "only the blob publisher may grant download access".into(),
                    ));
                }
                if grantee.0 == meta.publisher {
                    return Err(MempoolError::MediaBlobRejected(
                        "publisher already has download access".into(),
                    ));
                }
            }
            TransactionPayload::RevokeBlobDownload { blob_id, .. } => {
                require_active()?;
                let Some(store) = self.media_blob_store() else {
                    return Err(MempoolError::MediaBlobRejected(
                        "media blob store unavailable".into(),
                    ));
                };
                let Some(meta) = store.get_meta(blob_id) else {
                    return Err(MempoolError::MediaBlobRejected(
                        "revoke target blob not in sidecar".into(),
                    ));
                };
                if meta.publisher != signed.tx.sender.0 {
                    return Err(MempoolError::MediaBlobRejected(
                        "only the blob publisher may revoke download access".into(),
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Name registry txs are admitted only when the registry is configured and active at the next
    /// height, and only if they would execute (bad label, unmatured commit, taken name, not owner, rent
    /// unaffordable → rejected up front). Keeps pre-activation chains free of the new variants.
    fn precheck_name_tx(&self, signed: &SignedTransaction) -> Result<(), MempoolError> {
        if !matches!(
            signed.tx.payload,
            TransactionPayload::NameCommit { .. }
                | TransactionPayload::NameRegister { .. }
                | TransactionPayload::NameRenew { .. }
                | TransactionPayload::NameTransfer { .. }
                | TransactionPayload::NameSetTarget { .. }
                | TransactionPayload::NameSetPrimary { .. }
                | TransactionPayload::NameRelease { .. }
        ) {
            return Ok(());
        }
        let mut state_copy = self.state.snapshot();
        self.executor
            .prepare_state_for_height(self.chain.height().saturating_add(1), &mut state_copy);
        let exec_ctx = boing_execution::VmExecutionContext {
            block_height: self.chain.height().saturating_add(1),
            block_timestamp: 0,
            native_value: false, // set by the VM from the height
        };
        let vm = self.admission_vm();
        match vm.execute_with_context(&signed.tx, &mut state_copy, exec_ctx) {
            Ok(_) => Ok(()),
            Err(boing_execution::VmError::InvalidNonce { .. }) => Ok(()), // queued behind earlier txs
            Err(e) => Err(MempoolError::NameRejected(e.to_string())),
        }
    }

    /// Turn on the name registry with `params` (all nodes must use the same values). Call after loading
    /// persisted state; nothing changes on-chain before `params.activation_height`.
    pub fn enable_name_registry(&mut self, params: boing_state::NameRegistryParams) {
        self.state.set_name_registry_params(Some(params));
    }

    /// Switch every per-contract storage tree to full 256-bit routing from block `height` (storage-tree
    /// v2: closes the 64-bit-prefix collision). Consensus-critical: all nodes must agree, and on a live
    /// chain `height` must be in the future. Nothing changes before `height`; see
    /// `docs/STORAGE-TREE-V2-ACTIVATION.md`. Call after loading persisted state.
    pub fn enable_storage_tree_v2(&mut self, height: u64) {
        self.state.set_storage_tree_v2_activation(Some(height));
    }

    /// Enable L1 media blobs (PutBlob) from `activation_height`. All nodes must agree.
    /// Opens / keeps the sidecar when `data_dir` was configured; for in-memory tests call
    /// [`Self::attach_media_blob_store`] first if RPC getBlob is needed.
    pub fn enable_media_blobs(&mut self, activation_height: u64) {
        self.executor.set_media_blobs_from(Some(activation_height));
        self.vm.set_media_blobs_from(Some(activation_height));
    }

    /// Phase 7: raise Put/Announce max toward 256 MiB from `activation_height`.
    /// Unset / unused keeps the **16 MiB** live max (Fly boot-safe).
    pub fn enable_media_blobs_phase7_cap(&mut self, activation_height: u64) {
        self.executor
            .set_media_blobs_phase7_cap_from(Some(activation_height));
        self.vm
            .set_media_blobs_phase7_cap_from(Some(activation_height));
    }

    pub fn media_blobs_phase7_cap_activation_height(&self) -> Option<u64> {
        self.executor.vm().media_blobs_phase7_cap_from()
    }

    /// Phase 32: flip multi-pin committee verify from Ed25519 tx 32 → BLS aggregate tx 33.
    /// Unset / unused keeps Phase 31 policy (multi-pin default Ed25519 **32**).
    pub fn enable_bls_committee_flip(&mut self, activation_height: u64) {
        self.executor
            .set_bls_committee_flip_from(Some(activation_height));
        self.vm.set_bls_committee_flip_from(Some(activation_height));
    }

    pub fn bls_committee_flip_height(&self) -> Option<u64> {
        self.executor.vm().bls_committee_flip_from()
    }

    /// Phase 3: pin `blobs_root` into produced / validated headers from `activation_height`.
    pub fn enable_blobs_root_header(&mut self, activation_height: u64) {
        self.executor
            .set_blobs_root_header_from(Some(activation_height));
        self.vm.set_blobs_root_header_from(Some(activation_height));
        self.producer
            .set_blobs_root_header_from(Some(activation_height));
    }

    /// Attach a media blob sidecar (tests / custom data paths).
    pub fn attach_media_blob_store(&mut self, store: MediaBlobStore) {
        self.media_blob_store = Some(store);
    }

    pub fn media_blob_store(&self) -> Option<&MediaBlobStore> {
        self.media_blob_store.as_ref()
    }

    pub fn media_blobs_activation_height(&self) -> Option<u64> {
        self.executor.vm().media_blobs_from()
    }

    pub fn blobs_root_header_activation_height(&self) -> Option<u64> {
        self.executor.vm().blobs_root_header_from()
    }

    pub fn media_blobs_active_at_tip(&self) -> bool {
        let next = self.chain.height().saturating_add(1);
        self.executor.vm().media_blobs_active(next)
    }

    /// Media blob JSON-RPC (`boing_getBlob`, `boing_getBlobMeta`, `boing_getMediaBlobParams`, `boing_getBlobDasSample`).
    pub fn media_blobs_rpc(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, (i32, String)> {
        crate::media_blobs_rpc::dispatch(self, method, params)
    }

    /// Use a network's genesis (proposer + height-0 allocations) instead of the local-identity
    /// default. Only acts while the node is at height 0 (nothing committed beyond genesis), so it can
    /// never alter an existing chain. Call right after construction, before enabling optional
    /// registries. Returns true if block 0 changed.
    pub fn apply_genesis_spec(&mut self, spec: &crate::network_preset::GenesisSpec) -> bool {
        if self.chain.height() != 0 {
            return false;
        }
        let genesis = ChainState::genesis(spec.proposer);
        let changed = self.chain.latest_hash() != genesis.hash();
        if changed {
            self.chain = ChainState::from_genesis(genesis.clone());
        }
        let mut state = StateStore::new();
        for (id, balance) in &spec.allocations {
            state.insert(Account {
                id: *id,
                state: AccountState {
                    balance: *balance,
                    ..Default::default()
                },
            });
        }
        self.state = state;
        if let Some(p) = &self.persistence {
            if changed {
                if let Err(e) = p
                    .save_block(&genesis)
                    .and_then(|_| p.save_chain_meta(0, genesis.hash()))
                {
                    tracing::warn!("Persistence: could not rewrite genesis: {e}");
                }
            }
            if let Err(e) = p.save_state(&self.state) {
                tracing::warn!("Persistence: could not save genesis state: {e}");
            }
        }
        self.refresh_native_aggregates();
        changed
    }

    /// Name registry JSON-RPC (`boing_resolveName`, `boing_reverseName`, `boing_getNameInfo`,
    /// `boing_checkNames`, `boing_namesOwnedBy`). `Err((code, message))` on bad params.
    pub fn names_rpc(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, (i32, String)> {
        crate::names_rpc::dispatch(self, method, params)
    }

    /// Submit a signed transaction to the mempool.
    /// Update the process-wide tx signature policy for the next block height (v2 chain-id signatures
    /// are required from the activation height; before it either format is accepted).
    /// VM for admission checks and simulation, with the same validator registration proof rule as
    /// block execution (security review #14), so the mempool accepts exactly what blocks will.
    pub fn admission_vm(&self) -> Vm {
        let mut vm = Vm::with_qa_registry(self.mempool.qa_registry().clone());
        let (chain_id, from) = self.executor.registration_proof_rule();
        vm.set_registration_proof_rule(chain_id, from);
        vm.set_security_v4_from(self.executor.vm().security_v4_from());
        vm.set_base_units_from(self.executor.vm().base_units_from());
        vm.set_native_value_from(self.executor.vm().native_value_from());
        vm.set_media_blobs_from(self.executor.vm().media_blobs_from());
        vm.set_blobs_root_header_from(self.executor.vm().blobs_root_header_from());
        vm.set_media_blobs_phase7_cap_from(self.executor.vm().media_blobs_phase7_cap_from());
        vm.set_bls_committee_flip_from(self.executor.vm().bls_committee_flip_from());
        vm
    }

    pub fn refresh_tx_sig_policy(&self) -> boing_primitives::TxSigPolicy {
        let p = crate::network_preset::tx_sig_policy(
            crate::network_preset::configured_chain_id(),
            self.executor.execution_history(),
            self.chain.height().saturating_add(1),
        );
        boing_primitives::set_tx_verify_policy(p);
        p
    }

    /// Native-value rules for a tx that would land in the next block: deploy QA accepts the new
    /// opcodes only from the activation height, and value-carrying calls are rejected before it.
    fn precheck_native_value_tx(&self, signed: &SignedTransaction) -> Result<(), MempoolError> {
        let next = self.chain.height().saturating_add(1);
        let active = self.executor.native_value_active(next);
        self.mempool.set_native_value_opcodes(active);
        let carries_value = match &signed.tx.payload {
            TransactionPayload::ContractCallWithValue { .. } => true,
            TransactionPayload::Batch { calls } => calls
                .iter()
                .any(|c| matches!(c, boing_primitives::BatchCall::ContractCallWithValue { .. })),
            _ => false,
        };
        if carries_value && !active {
            return Err(MempoolError::NativeValueNotActive {
                from: self.executor.vm().native_value_from(),
            });
        }
        Ok(())
    }

    pub fn submit_transaction(&self, signed: SignedTransaction) -> Result<(), MempoolError> {
        self.refresh_tx_sig_policy();
        // Cheapest checks first: signature, duplicate, then account-level gates; the snapshot-based
        // dry runs below only ever run for authenticated txs from funded, non-stale senders.
        Mempool::verify_signature(&signed)?;
        if self.mempool.contains_tx_id(&signed.tx.id()) {
            return Err(MempoolError::Duplicate);
        }
        self.precheck_sender_viable(&signed)?;
        self.precheck_native_value_tx(&signed)?;
        self.precheck_open_validator_tx(&signed)?;
        self.precheck_name_tx(&signed)?;
        self.precheck_media_blob_tx(&signed)?;
        self.precheck_sender_can_pay_fee(&signed)?;
        match self.mempool.insert(signed.clone()) {
            Ok(()) => {
                if let TransactionPayload::QaPoolVote { subject, vote } = signed.tx.payload {
                    let _ = self.note_on_chain_qa_vote(subject, signed.tx.sender, vote);
                }
                Ok(())
            }
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
                if self.qa_pool.governance_config().public_membership {
                    let _ = self.mempool.insert_unsure_for_inclusion(signed);
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
        let stake = self.state.get(&voter).map(|s| s.stake);
        self.qa_pool.vote(tx_hash, voter, vote, stake)?;
        if self.qa_pool.governance_config().public_membership {
            // Deploy completion and voter payouts happen when signed QaPoolVote txs apply.
            return match self.qa_pool.resolve(tx_hash) {
                PoolResolution::Pending => Ok(QaPoolVoteResult::Pending),
                PoolResolution::Reject => Ok(QaPoolVoteResult::Rejected),
                PoolResolution::Allow(_) => Ok(QaPoolVoteResult::Pending),
            };
        }
        match self.qa_pool.resolve(tx_hash) {
            PoolResolution::Pending => Ok(QaPoolVoteResult::Pending),
            PoolResolution::Reject => Ok(QaPoolVoteResult::Rejected),
            PoolResolution::Allow(bytes) => {
                let signed = SignedTransaction::from_bincode(&bytes)
                    .map_err(|_| PoolError::Deserialization)?;
                match self.mempool.insert_after_pool_allow(signed) {
                    Ok(()) => Ok(QaPoolVoteResult::AllowedAdmitted),
                    Err(MempoolError::Duplicate) => Ok(QaPoolVoteResult::AllowedAlreadyInMempool),
                    Err(e) => Ok(QaPoolVoteResult::AllowedMempoolFailed(e.to_string())),
                }
            }
        }
    }

    /// Record a signed on-chain QA vote in the local pool (for RPC listing) after mempool admission.
    pub fn note_on_chain_qa_vote(
        &self,
        subject: Hash,
        voter: AccountId,
        vote: QaPoolVoteKind,
    ) -> Result<(), PoolError> {
        let qv = match vote {
            QaPoolVoteKind::Allow => QaPoolVote::Allow,
            QaPoolVoteKind::Reject => QaPoolVote::Reject,
            QaPoolVoteKind::Abstain => QaPoolVote::Abstain,
        };
        let stake = self.state.get(&voter).map(|s| s.stake);
        self.qa_pool.vote(subject, voter, qv, stake)
    }

    fn apply_qa_pool_expirations(&self) {
        for (_h, res) in self.qa_pool.prune_expired() {
            if let PoolResolution::Allow(bytes) = res {
                if let Ok(signed) = SignedTransaction::from_bincode(&bytes) {
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
        // Open mode tracks liveness deterministically on-chain (jail, no slashing); skip local slashing.
        if !self.consensus.anchors().is_empty() || self.open_validator_params.is_some() {
            self.leader_wait_started = None;
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
        // Phase 15: seed consensus appeal book (detection remains node-local; 30/31 replay restore).
        let slash_id = self
            .state
            .slash_appeals_mut()
            .record_slash(
                leader,
                burned,
                boing_state::ConsensusSlashReason::Liveness,
                next_height,
                boing_tokenomics::LIVENESS_APPEAL_WINDOW_BLOCKS,
            )
            .unwrap_or_else(|_| {
                self.slash_registry.record_slash(
                    leader.0,
                    burned,
                    SlashReason::Liveness,
                    next_height,
                    boing_tokenomics::LIVENESS_APPEAL_WINDOW_BLOCKS,
                )
            });
        self.slash_registry.record_slash_with_id(
            slash_id,
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
        let round = self.chain.height().saturating_add(1);
        if self.last_local_vote_round == Some(round) {
            return None; // already voted at this height; proposing now would equivocate
        }
        let anchor_fallback = self.anchor_fallback_due();

        let proposal = self.producer.produce_proposal_with_fallback(
            &self.chain,
            &self.mempool,
            &mut self.state,
            &self.executor,
            &self.consensus,
            anchor_fallback,
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

/// Chain-bound validator registration proofs (security review #14) for a node with a chain id and no
/// network history (new chains): v2 only from genesis. A network history, when installed, overrides
/// this with its own chain id and activation height.
fn with_chain_id_rules(mut executor: BlockExecutor) -> BlockExecutor {
    if let Some(c) = crate::network_preset::configured_chain_id() {
        executor.set_registration_proof_rule(Some(c), 0);
    }
    executor
}
