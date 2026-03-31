//! Community QA pool — pending queue for Unsure deploys, governed by [`crate::pool_config::QaPoolGovernanceConfig`].
//!
//! Governance-listed **administrators** vote Allow/Reject. Queue size is **hard-capped** to prevent congestion.
//! See QUALITY-ASSURANCE-NETWORK.md §8.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use boing_primitives::{AccountId, Hash, SignedTransaction};

use crate::pool_config::{QaPoolExpiryPolicy, QaPoolGovernanceConfig};

/// Result of [`PendingQaQueue::resolve`]: either still voting, allow with tx bytes for mempool, or reject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PoolResolution {
    /// Quorum/threshold not met and not expired.
    Pending,
    /// Pool approved; bincode of [`SignedTransaction`] to insert (skipping deploy QA).
    Allow(Vec<u8>),
    Reject,
}

/// Single vote from a governance administrator (or any voter in dev open mode).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QaPoolVote {
    Allow,
    Reject,
    Abstain,
}

/// A deployment pending QA pool review.
#[derive(Clone, Debug)]
pub struct PendingQaItem {
    pub tx_hash: Hash,
    pub bytecode_hash: [u8; 32],
    pub deployer: AccountId,
    pub entered_at: Instant,
    pub votes: HashMap<AccountId, QaPoolVote>,
    /// Bincode-encoded [`SignedTransaction`]; admitted to mempool on [`PoolResolution::Allow`].
    pub signed_tx_bincode: Vec<u8>,
}

impl PendingQaItem {
    /// Build from a verified deploy transaction (signature must be valid).
    pub fn from_signed(signed: &SignedTransaction) -> Result<Self, PoolError> {
        signed.verify().map_err(|_| PoolError::InvalidSignature)?;
        let tx_hash = signed.tx.id();
        let bytecode_hash = bytecode_blake3_digest(&signed.tx);
        let deployer = signed.tx.sender;
        let signed_tx_bincode = bincode::serialize(signed).map_err(|_| PoolError::Serialization)?;
        Ok(Self {
            tx_hash,
            bytecode_hash,
            deployer,
            entered_at: Instant::now(),
            votes: HashMap::new(),
            signed_tx_bincode,
        })
    }

    /// For tests: supply bincode explicitly.
    pub fn new_for_test(
        tx_hash: Hash,
        bytecode_hash: [u8; 32],
        deployer: AccountId,
        signed_tx_bincode: Vec<u8>,
    ) -> Self {
        Self {
            tx_hash,
            bytecode_hash,
            deployer,
            entered_at: Instant::now(),
            votes: HashMap::new(),
            signed_tx_bincode,
        }
    }

    /// Count Allow and Reject votes (excluding Abstain).
    pub fn vote_counts(&self) -> (usize, usize) {
        let mut allow = 0;
        let mut reject = 0;
        for v in self.votes.values() {
            match v {
                QaPoolVote::Allow => allow += 1,
                QaPoolVote::Reject => reject += 1,
                QaPoolVote::Abstain => {}
            }
        }
        (allow, reject)
    }

    /// Whether the item has reached the deadline.
    pub fn is_expired(&self, max_time: Duration) -> bool {
        self.entered_at.elapsed() >= max_time
    }

    pub fn age_secs(&self) -> u64 {
        self.entered_at.elapsed().as_secs()
    }
}

fn bytecode_blake3_digest(tx: &boing_primitives::Transaction) -> [u8; 32] {
    tx.payload
        .as_contract_deploy()
        .map(|(bc, ..)| *blake3::hash(bc).as_bytes())
        .unwrap_or([0u8; 32])
}

/// Policy when deadline expires without quorum (mirrors governance JSON).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultOnExpiry {
    Reject,
    Allow,
}

/// In-memory pending QA queue; parameters come from [`QaPoolGovernanceConfig`].
pub struct PendingQaQueue {
    inner: Mutex<PendingQaQueueInner>,
    governance: Mutex<QaPoolGovernanceConfig>,
}

struct PendingQaQueueInner {
    items: HashMap<Hash, PendingQaItem>,
}

impl PendingQaQueue {
    pub fn from_governance_config(config: QaPoolGovernanceConfig) -> Self {
        Self {
            inner: Mutex::new(PendingQaQueueInner {
                items: HashMap::new(),
            }),
            governance: Mutex::new(config),
        }
    }

    pub fn governance_config(&self) -> QaPoolGovernanceConfig {
        self.governance.lock().unwrap().clone()
    }

    /// Replace governance parameters (e.g. after a governance proposal executes). Pending items are kept.
    pub fn set_governance_config(&self, config: QaPoolGovernanceConfig) {
        tracing::info!(
            target: "boing_qa::pool",
            max_pending = config.max_pending_items,
            admin_count = config.administrator_accounts().len(),
            dev_open = config.dev_open_voting,
            accepts = config.accepts_new_pending(),
            "QA pool governance config updated"
        );
        *self.governance.lock().unwrap() = config;
    }

    pub fn accepts_new_pending(&self) -> bool {
        self.governance.lock().unwrap().accepts_new_pending()
    }

    pub fn pending_len(&self) -> usize {
        self.inner.lock().unwrap().items.len()
    }

    fn deployer_pending_count(inner: &PendingQaQueueInner, deployer: AccountId) -> usize {
        inner.items.values().filter(|i| i.deployer == deployer).count()
    }

    /// Add an item that received Unsure from automation.
    pub fn add(&self, item: PendingQaItem) -> Result<(), PoolError> {
        let cfg = self.governance.lock().unwrap().clone();
        if !cfg.accepts_new_pending() {
            return Err(PoolError::PoolDisabled);
        }
        let mut inner = self.inner.lock().unwrap();
        if inner.items.contains_key(&item.tx_hash) {
            return Err(PoolError::Duplicate);
        }
        if inner.items.len() >= cfg.max_pending_items as usize {
            return Err(PoolError::PoolFull);
        }
        if cfg.max_pending_per_deployer > 0 {
            let c = Self::deployer_pending_count(&inner, item.deployer);
            if c >= cfg.max_pending_per_deployer as usize {
                return Err(PoolError::DeployerCapExceeded);
            }
        }
        inner.items.insert(item.tx_hash, item);
        tracing::debug!(
            target: "boing_qa::pool",
            pending = inner.items.len(),
            "QA pool enqueue (Unsure deploy)"
        );
        Ok(())
    }

    /// Record a vote from a governance administrator.
    pub fn vote(&self, tx_hash: Hash, voter: AccountId, vote: QaPoolVote) -> Result<(), PoolError> {
        let cfg = self.governance.lock().unwrap();
        if !cfg.voter_may_vote(voter) {
            return Err(PoolError::NotAdministrator);
        }
        drop(cfg);
        let mut inner = self.inner.lock().unwrap();
        let item = inner.items.get_mut(&tx_hash).ok_or(PoolError::NotFound)?;
        item.votes.insert(voter, vote);
        Ok(())
    }

    /// Evaluate quorum / thresholds / expiry. Removes item when resolved.
    pub fn resolve(&self, tx_hash: Hash) -> PoolResolution {
        let cfg = self.governance.lock().unwrap().clone();
        let max_time = Duration::from_secs(cfg.review_window_secs);
        let default_on_expiry = match cfg.default_on_expiry {
            QaPoolExpiryPolicy::Reject => DefaultOnExpiry::Reject,
            QaPoolExpiryPolicy::Allow => DefaultOnExpiry::Allow,
        };
        let quorum_fraction = cfg.quorum_fraction;
        let allow_threshold = cfg.allow_threshold_fraction;
        let reject_threshold = cfg.reject_threshold_fraction;
        let effective_pool = cfg.effective_electorate_size();

        let mut inner = self.inner.lock().unwrap();
        let Some(item) = inner.items.get(&tx_hash) else {
            return PoolResolution::Pending;
        };

        if item.is_expired(max_time) {
            let item = match inner.items.remove(&tx_hash) {
                Some(i) => i,
                None => return PoolResolution::Pending,
            };
            return match default_on_expiry {
                DefaultOnExpiry::Reject => PoolResolution::Reject,
                DefaultOnExpiry::Allow => PoolResolution::Allow(item.signed_tx_bincode),
            };
        }

        let (allow, reject) = item.vote_counts();
        let total_votes = allow + reject;

        let quorum_met = (total_votes as f64 / effective_pool as f64) >= quorum_fraction;
        if !quorum_met {
            return PoolResolution::Pending;
        }

        if total_votes == 0 {
            return PoolResolution::Pending;
        }

        let allow_ratio = allow as f64 / total_votes as f64;
        let reject_ratio = reject as f64 / total_votes as f64;

        if allow_ratio >= allow_threshold {
            let item = match inner.items.remove(&tx_hash) {
                Some(i) => i,
                None => return PoolResolution::Pending,
            };
            tracing::info!(
                target: "boing_qa::pool",
                tx_hash = %hex::encode(item.tx_hash.0),
                "QA pool resolved Allow"
            );
            return PoolResolution::Allow(item.signed_tx_bincode);
        }
        if reject_ratio >= reject_threshold {
            let _ = inner.items.remove(&tx_hash);
            tracing::info!(
                target: "boing_qa::pool",
                tx_hash = %hex::encode(tx_hash.0),
                "QA pool resolved Reject"
            );
            return PoolResolution::Reject;
        }

        PoolResolution::Pending
    }

    /// Get a pending item by tx hash.
    pub fn get(&self, tx_hash: &Hash) -> Option<PendingQaItem> {
        self.inner.lock().unwrap().items.get(tx_hash).cloned()
    }

    /// List all pending items (includes bincode; use [`Self::list_summaries`] for RPC).
    pub fn list(&self) -> Vec<PendingQaItem> {
        self.inner.lock().unwrap().items.values().cloned().collect()
    }

    /// JSON-friendly rows without signed tx bytes.
    pub fn list_summaries(&self) -> Vec<QaPoolItemSummary> {
        self.list()
            .into_iter()
            .map(|i| QaPoolItemSummary {
                tx_hash: format!("0x{}", hex::encode(i.tx_hash.0)),
                bytecode_hash: format!("0x{}", hex::encode(i.bytecode_hash)),
                deployer: format!("0x{}", hex::encode(i.deployer.0)),
                allow_votes: i.vote_counts().0,
                reject_votes: i.vote_counts().1,
                age_secs: i.age_secs(),
            })
            .collect()
    }

    /// Remove expired items and return resolutions (for background tick).
    pub fn prune_expired(&self) -> Vec<(Hash, PoolResolution)> {
        let cfg = self.governance.lock().unwrap().clone();
        let max_time = Duration::from_secs(cfg.review_window_secs);
        let default_on_expiry = match cfg.default_on_expiry {
            QaPoolExpiryPolicy::Reject => DefaultOnExpiry::Reject,
            QaPoolExpiryPolicy::Allow => DefaultOnExpiry::Allow,
        };
        drop(cfg);

        let mut inner = self.inner.lock().unwrap();
        let mut out = Vec::new();
        let to_check: Vec<Hash> = inner.items.keys().copied().collect();
        for h in to_check {
            let Some(item) = inner.items.get(&h) else {
                continue;
            };
            if !item.is_expired(max_time) {
                continue;
            }
            let Some(item) = inner.items.remove(&h) else {
                continue;
            };
            let res = match default_on_expiry {
                DefaultOnExpiry::Reject => PoolResolution::Reject,
                DefaultOnExpiry::Allow => PoolResolution::Allow(item.signed_tx_bincode),
            };
            out.push((h, res));
        }
        out
    }
}

/// Public view for RPC (`boing_qaPoolList`).
#[derive(Clone, Debug, serde::Serialize)]
pub struct QaPoolItemSummary {
    pub tx_hash: String,
    pub bytecode_hash: String,
    pub deployer: String,
    pub allow_votes: usize,
    pub reject_votes: usize,
    pub age_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool_config::QaPoolGovernanceConfig;
    use boing_primitives::{AccessList, Transaction, TransactionPayload};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn sample_deploy_tx() -> SignedTransaction {
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let tx = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: vec![0x00],
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        SignedTransaction::new(tx, &key)
    }

    fn config_with_admins(ids: &[AccountId]) -> QaPoolGovernanceConfig {
        let mut c = QaPoolGovernanceConfig::development_default();
        c.dev_open_voting = false;
        c.administrators = ids
            .iter()
            .map(|a| format!("0x{}", hex::encode(a.0)))
            .collect();
        c
    }

    #[test]
    fn pool_add_vote_resolve_allow_with_admins() {
        let queue = PendingQaQueue::from_governance_config(config_with_admins(&[
            AccountId::from_bytes([1u8; 32]),
            AccountId::from_bytes([2u8; 32]),
            AccountId::from_bytes([3u8; 32]),
        ]));

        let signed = sample_deploy_tx();
        let tx_hash = signed.tx.id();
        let item = PendingQaItem::from_signed(&signed).unwrap();
        queue.add(item).unwrap();

        queue
            .vote(tx_hash, AccountId::from_bytes([1u8; 32]), QaPoolVote::Allow)
            .unwrap();
        queue
            .vote(tx_hash, AccountId::from_bytes([2u8; 32]), QaPoolVote::Allow)
            .unwrap();
        queue
            .vote(tx_hash, AccountId::from_bytes([3u8; 32]), QaPoolVote::Reject)
            .unwrap();

        let r = queue.resolve(tx_hash);
        assert!(matches!(r, PoolResolution::Allow(_)));
        if let PoolResolution::Allow(bytes) = r {
            let back: SignedTransaction = bincode::deserialize(&bytes).unwrap();
            assert_eq!(back.tx.id(), tx_hash);
        }
    }

    #[test]
    fn open_voting_when_dev_flag_and_no_admins() {
        let queue = PendingQaQueue::from_governance_config(QaPoolGovernanceConfig::development_default());
        let signed = sample_deploy_tx();
        let tx_hash = signed.tx.id();
        queue.add(PendingQaItem::from_signed(&signed).unwrap()).unwrap();
        queue
            .vote(tx_hash, AccountId::from_bytes([9u8; 32]), QaPoolVote::Allow)
            .unwrap();
        assert!(matches!(queue.resolve(tx_hash), PoolResolution::Allow(_)));
    }

    #[test]
    fn pool_full_respects_max_pending() {
        let mut c = QaPoolGovernanceConfig::development_default();
        c.max_pending_items = 1;
        let queue = PendingQaQueue::from_governance_config(c);
        let a = sample_deploy_tx();
        let b = sample_deploy_tx();
        queue.add(PendingQaItem::from_signed(&a).unwrap()).unwrap();
        assert_eq!(queue.add(PendingQaItem::from_signed(&b).unwrap()), Err(PoolError::PoolFull));
    }

    #[test]
    fn production_default_disables_pool_until_admins_configured() {
        let queue = PendingQaQueue::from_governance_config(QaPoolGovernanceConfig::production_default());
        let signed = sample_deploy_tx();
        assert_eq!(
            queue.add(PendingQaItem::from_signed(&signed).unwrap()),
            Err(PoolError::PoolDisabled)
        );
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PoolError {
    #[error("Item already in pool")]
    Duplicate,
    #[error("Voter is not a governance QA administrator")]
    NotAdministrator,
    #[error("QA pool is disabled by governance (configure administrators or dev_open_voting)")]
    PoolDisabled,
    #[error("QA pool is at max_pending_items capacity")]
    PoolFull,
    #[error("Deployer has reached max pending pool items per address")]
    DeployerCapExceeded,
    #[error("Item not found")]
    NotFound,
    #[error("Invalid transaction signature")]
    InvalidSignature,
    #[error("Serialization failed")]
    Serialization,
    #[error("Deserialization failed")]
    Deserialization,
}
