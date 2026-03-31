//! Transaction mempool — pending transactions awaiting inclusion in a block.
//! Runs protocol QA on ContractDeploy before accepting; rejects with structured reason when QA fails.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use boing_primitives::{AccountId, Hash, SignedTransaction};
use boing_qa::{check_contract_deploy_full_with_metadata, QaReject, QaResult, RuleRegistry};

/// Default max pending transactions per sender (matches SECURITY-STANDARDS / RateLimitConfig).
pub const DEFAULT_MAX_PENDING_PER_SENDER: usize = 16;

/// In-memory mempool. Tracks pending transactions by sender nonce.
pub struct Mempool {
    inner: Mutex<MempoolInner>,
    max_pending_per_sender: usize,
    /// QA rule registry (blocklist, content blocklist, etc.). Replace via [Mempool::with_qa_registry] or governance.
    qa_registry: RuleRegistry,
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

/// Default rule registry for QA (max bytecode size, etc.). Can be replaced with on-chain config later.
fn default_qa_registry() -> RuleRegistry {
    RuleRegistry::new()
}

#[derive(Default)]
struct MempoolInner {
    /// Pending txs by sender, then by nonce.
    by_sender: HashMap<AccountId, BTreeMap<u64, SignedTransaction>>,
    /// All tx IDs for dedup.
    by_id: HashMap<boing_primitives::Hash, ()>,
    /// Count of pending txs.
    len: usize,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MempoolInner::default()),
            max_pending_per_sender: DEFAULT_MAX_PENDING_PER_SENDER,
            qa_registry: default_qa_registry(),
        }
    }

    pub fn with_max_pending_per_sender(mut self, max: usize) -> Self {
        self.max_pending_per_sender = max.max(1);
        self
    }

    /// Use a custom QA rule registry (e.g. loaded from file or from governance). Content blocklist and other rules are applied from this registry.
    pub fn with_qa_registry(mut self, registry: RuleRegistry) -> Self {
        self.qa_registry = registry;
        self
    }

    /// Rule registry used for deploy admission; the VM should use the same instance for execution alignment.
    pub fn qa_registry(&self) -> &RuleRegistry {
        &self.qa_registry
    }

    /// Replace QA registry (governance execution). Prefer [`crate::node::BoingNode::set_qa_policy`] to keep VM/executor in sync.
    pub fn set_qa_registry(&mut self, registry: RuleRegistry) {
        self.qa_registry = registry;
    }

    /// Insert a signed transaction. Rejects duplicates, invalid nonces, per-sender cap, and ContractDeploy that fail QA.
    pub fn insert(&self, signed: SignedTransaction) -> Result<(), MempoolError> {
        self.insert_inner(signed, false)
    }

    /// Insert after community QA pool approved (`Unsure` path). Skips deploy-time registry QA so the tx does not loop as Unsure.
    pub fn insert_after_pool_allow(&self, signed: SignedTransaction) -> Result<(), MempoolError> {
        self.insert_inner(signed, true)
    }

    fn insert_inner(
        &self,
        signed: SignedTransaction,
        skip_deploy_qa: bool,
    ) -> Result<(), MempoolError> {
        signed
            .verify()
            .map_err(|_| MempoolError::InvalidSignature)?;
        if !skip_deploy_qa {
            if let Some((bytecode, purpose, desc_hash, asset_name, asset_symbol)) =
                signed.tx.payload.as_contract_deploy()
            {
                let registry = &self.qa_registry;
                match check_contract_deploy_full_with_metadata(
                    bytecode,
                    purpose,
                    desc_hash,
                    asset_name,
                    asset_symbol,
                    registry,
                ) {
                    QaResult::Reject(reject) => return Err(MempoolError::QaRejected(reject)),
                    QaResult::Unsure => {
                        return Err(MempoolError::QaPendingPool(signed.tx.id()));
                    }
                    QaResult::Allow => {}
                }
            }
        }
        let tx_id = signed.tx.id();
        let mut inner = self.inner.lock().unwrap();
        if inner.by_id.contains_key(&tx_id) {
            return Err(MempoolError::Duplicate);
        }
        let sender = signed.tx.sender;
        let nonce = signed.tx.nonce;
        let is_replacement = inner
            .by_sender
            .get(&sender)
            .map(|m| m.contains_key(&nonce))
            .unwrap_or(false);
        if !is_replacement {
            let sender_count = inner.by_sender.get(&sender).map(|m| m.len()).unwrap_or(0);
            if sender_count >= self.max_pending_per_sender {
                return Err(MempoolError::PendingLimitExceeded {
                    sender,
                    limit: self.max_pending_per_sender,
                });
            }
        }
        let prev = inner
            .by_sender
            .entry(sender)
            .or_default()
            .insert(nonce, signed);
        if let Some(old_signed) = prev {
            inner.by_id.remove(&old_signed.tx.id());
        } else {
            inner.len += 1;
        }
        inner.by_id.insert(tx_id, ());
        Ok(())
    }

    /// Remove and return signed transactions up to `max` for block inclusion.
    /// Returns txs in nonce order (per sender). Callers can re-insert on failure (e.g. consensus).
    pub fn drain_for_block(&self, max: usize) -> Vec<SignedTransaction> {
        let mut inner = self.inner.lock().unwrap();
        let mut candidates: Vec<(AccountId, u64)> = Vec::new();
        for (sender, by_nonce) in inner.by_sender.iter() {
            for nonce in by_nonce.keys() {
                candidates.push((*sender, *nonce));
            }
        }
        candidates.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0 .0.cmp(&b.0 .0)));
        let mut out = Vec::with_capacity(max.min(candidates.len()));
        for (sender, nonce) in candidates.into_iter().take(max) {
            if let Some(by_nonce) = inner.by_sender.get_mut(&sender) {
                if let Some(signed) = by_nonce.remove(&nonce) {
                    inner.by_id.remove(&signed.tx.id());
                    inner.len = inner.len.saturating_sub(1);
                    out.push(signed);
                }
            }
        }
        out
    }

    /// Re-insert signed transactions (e.g. after block production or consensus failure).
    /// Duplicates are skipped; invalid signatures are skipped. Used to restore txs when a block is not committed.
    pub fn reinsert(&self, signed_txs: Vec<SignedTransaction>) {
        for signed in signed_txs {
            let _ = self.insert(signed);
        }
    }

    /// Number of pending transactions.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("Duplicate transaction")]
    Duplicate,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Pending limit exceeded: sender has too many pending txs (max {limit})")]
    PendingLimitExceeded { sender: AccountId, limit: usize },
    /// Protocol QA rejected this deployment; rule_id and message give user feedback.
    #[error("QA rejected: {0}")]
    QaRejected(QaReject),
    /// Deployment referred to community QA pool (Unsure); not accepted until pool decides.
    #[error("Deployment referred to community QA pool (tx_hash={0})")]
    QaPendingPool(Hash),
    #[error("QA pool enqueue failed: {0}")]
    QaPoolEnqueue(String),
    #[error("QA pool disabled by governance (no administrators configured)")]
    QaPoolDisabled,
    #[error("QA pool is at capacity (max_pending_items); try later")]
    QaPoolFull,
    #[error("QA pool deployer pending limit exceeded")]
    QaPoolDeployerCap,
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, Transaction, TransactionPayload};
    use boing_qa::RuleRegistry;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn unsure_deploy_insert_after_pool_allow_skips_qa() {
        let reg = RuleRegistry::new().with_always_review_categories(vec!["meme".to_string()]);
        let pool = Mempool::new().with_qa_registry(reg);
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let tx = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: vec![0x00],
                purpose_category: "meme".to_string(),
                description_hash: None,
                asset_name: None,
                asset_symbol: None,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        let signed = SignedTransaction::new(tx, &key);
        assert!(matches!(
            pool.insert(signed.clone()),
            Err(MempoolError::QaPendingPool(_))
        ));
        assert!(pool.insert_after_pool_allow(signed).is_ok());
        assert_eq!(pool.len(), 1);
    }
}
