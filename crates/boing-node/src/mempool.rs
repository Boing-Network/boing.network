//! Transaction mempool — pending transactions awaiting inclusion in a block.
//! Runs protocol QA on ContractDeploy before accepting; rejects with structured reason when QA fails.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use boing_primitives::{AccountId, Hash, SignedTransaction};
use boing_qa::{check_contract_deploy_full_with_metadata, QaReject, QaResult, RuleRegistry};

/// Default max pending transactions per sender (matches SECURITY-STANDARDS / RateLimitConfig).
pub const DEFAULT_MAX_PENDING_PER_SENDER: usize = 16;

/// Default cap on all pending transactions (security review #7). Override with `BOING_MEMPOOL_MAX_TXS`.
pub const DEFAULT_MAX_TOTAL_PENDING: usize = 10_000;

fn max_total_from_env() -> usize {
    std::env::var("BOING_MEMPOOL_MAX_TXS")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_MAX_TOTAL_PENDING)
}

/// In-memory mempool. Tracks pending transactions by sender nonce.
pub struct Mempool {
    inner: Mutex<MempoolInner>,
    max_pending_per_sender: usize,
    /// Global cap on pending txs. When full, a new tx evicts the newest (highest-nonce) tx of the sender
    /// holding the most pending txs, if that sender holds more than the newcomer would; otherwise it is
    /// rejected with [`MempoolError::MempoolFull`]. Boing has no fee bidding (fees are fixed by the
    /// protocol schedule), so fairness across senders replaces "lowest fee first" eviction.
    max_total: usize,
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
            max_total: max_total_from_env(),
            qa_registry: default_qa_registry(),
        }
    }

    pub fn with_max_pending_per_sender(mut self, max: usize) -> Self {
        self.max_pending_per_sender = max.max(1);
        self
    }

    /// Global pending-tx cap (minimum **1**).
    pub fn with_max_total(mut self, max: usize) -> Self {
        self.max_total = max.max(1);
        self
    }

    pub fn max_total(&self) -> usize {
        self.max_total
    }

    /// Runtime cap on distinct pending nonces per sender (minimum **1**).
    pub fn set_max_pending_per_sender(&mut self, max: usize) {
        self.max_pending_per_sender = max.max(1);
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
        self.insert_with_qa_mode(
            signed,
            if skip_deploy_qa {
                DeployQaMode::Skip
            } else {
                DeployQaMode::Enforce
            },
        )
    }

    /// Admit an Unsure deploy so the producer can register it on-chain (public membership).
    pub fn insert_unsure_for_inclusion(
        &self,
        signed: SignedTransaction,
    ) -> Result<(), MempoolError> {
        self.insert_with_qa_mode(signed, DeployQaMode::AdmitUnsure)
    }

    fn insert_with_qa_mode(
        &self,
        signed: SignedTransaction,
        qa_mode: DeployQaMode,
    ) -> Result<(), MempoolError> {
        self.insert_checked(signed, qa_mode, false)
    }

    /// `restoring`: txs that were already in the pool (re-inserted after a failed block) skip the global
    /// cap, so a full pool never drops them.
    fn insert_checked(
        &self,
        signed: SignedTransaction,
        qa_mode: DeployQaMode,
        restoring: bool,
    ) -> Result<(), MempoolError> {
        signed.verify().map_err(|e| match e {
            boing_primitives::SignatureError::LegacySignatureRejected => {
                MempoolError::LegacySignature
            }
            _ => MempoolError::InvalidSignature,
        })?;
        signed
            .tx
            .payload
            .validate_batch()
            .map_err(MempoolError::InvalidBatch)?;
        if qa_mode != DeployQaMode::Skip {
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
                        if qa_mode != DeployQaMode::AdmitUnsure {
                            return Err(MempoolError::QaPendingPool(signed.tx.id()));
                        }
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
            if !restoring && inner.len >= self.max_total {
                Self::evict_for(&mut inner, sender, sender_count, self.max_total)?;
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

    /// Make room for one tx from `newcomer` (holding `newcomer_count` pending): evict the highest nonce of
    /// the sender with the most pending txs (ties: highest account id, so the choice is deterministic)
    /// when that sender would still hold more than the newcomer afterwards.
    fn evict_for(
        inner: &mut MempoolInner,
        newcomer: AccountId,
        newcomer_count: usize,
        max_total: usize,
    ) -> Result<(), MempoolError> {
        let victim = inner
            .by_sender
            .iter()
            .filter(|(id, m)| **id != newcomer && !m.is_empty())
            .max_by(|a, b| a.1.len().cmp(&b.1.len()).then_with(|| a.0 .0.cmp(&b.0 .0)))
            .map(|(id, m)| (*id, m.len()));
        match victim {
            Some((id, count)) if count > newcomer_count + 1 => {
                let by_nonce = inner.by_sender.get_mut(&id).expect("victim exists");
                let (_, evicted) = by_nonce.pop_last().expect("non-empty");
                if by_nonce.is_empty() {
                    inner.by_sender.remove(&id);
                }
                inner.by_id.remove(&evicted.tx.id());
                inner.len = inner.len.saturating_sub(1);
                Ok(())
            }
            _ => Err(MempoolError::MempoolFull { limit: max_total }),
        }
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
            let _ = self.insert_checked(signed, DeployQaMode::Skip, true);
        }
    }

    /// Number of pending transactions.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len
    }

    /// Whether a pending transaction with this id ([`Transaction::id`] on the signed payload) is in the pool.
    pub fn contains_tx_id(&self, tx_id: &Hash) -> bool {
        self.inner.lock().unwrap().by_id.contains_key(tx_id)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Next nonce for a **new** transaction from `sender`, given the committed `chain_nonce`
    /// (`AccountState::nonce` on chain). When this sender already has pending txs in the mempool,
    /// returns one past the highest pending nonce (so callers do not rebuild the same `tx.id()` and
    /// hit [`MempoolError::Duplicate`]). Used by the testnet faucet RPC.
    pub fn suggested_next_nonce(&self, sender: AccountId, chain_nonce: u64) -> u64 {
        let inner = self.inner.lock().unwrap();
        match inner.by_sender.get(&sender) {
            Some(by_nonce) if !by_nonce.is_empty() => {
                let max_pending = *by_nonce.keys().next_back().expect("non-empty");
                max_pending.saturating_add(1).max(chain_nonce)
            }
            _ => chain_nonce,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeployQaMode {
    Enforce,
    Skip,
    AdmitUnsure,
}

#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("Duplicate transaction")]
    Duplicate,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Legacy signature rejected: sign with the chain id (v2 signing domain) after the activation height")]
    LegacySignature,
    #[error("Invalid batch: {0}")]
    InvalidBatch(boing_primitives::BatchValidationError),
    #[error("Pending limit exceeded: sender has too many pending txs (max {limit})")]
    PendingLimitExceeded { sender: AccountId, limit: usize },
    #[error("Mempool full ({limit} pending txs); try again later")]
    MempoolFull { limit: usize },
    #[error("insufficient balance for fee: have {have}, need {need}")]
    InsufficientFee { have: u128, need: u128 },
    /// Protocol QA rejected this deployment; rule_id and message give user feedback.
    #[error("QA rejected: {0}")]
    QaRejected(QaReject),
    /// Deployment referred to community QA pool (Unsure); not accepted until pool decides.
    #[error("Deployment referred to community QA pool (tx_hash={0})")]
    QaPendingPool(Hash),
    #[error("QA pool enqueue failed: {0}")]
    QaPoolEnqueue(String),
    #[error("QA pool disabled by governance")]
    QaPoolDisabled,
    #[error("QA pool is at capacity (max_pending_items); try later")]
    QaPoolFull,
    #[error("QA pool deployer pending limit exceeded")]
    QaPoolDeployerCap,
    /// Open validator registration / deregistration / unjail would fail (or the mode is not active).
    #[error("open validator tx rejected: {0}")]
    OpenValidatorRejected(String),
    /// Name registry tx would fail (or the registry is not active yet).
    #[error("name tx rejected: {0}")]
    NameRejected(String),
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

    #[test]
    fn pending_per_sender_limit_blocks_extra_nonces() {
        let pool = Mempool::new().with_max_pending_per_sender(2);
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        for nonce in 0u64..2 {
            let tx = Transaction {
                nonce,
                sender,
                payload: TransactionPayload::Transfer {
                    to: AccountId([2u8; 32]),
                    amount: 1,
                },
                access_list: AccessList::default(),
            };
            pool.insert(SignedTransaction::new(tx, &key)).unwrap();
        }
        let tx3 = Transaction {
            nonce: 2,
            sender,
            payload: TransactionPayload::Transfer {
                to: AccountId([2u8; 32]),
                amount: 1,
            },
            access_list: AccessList::default(),
        };
        assert!(matches!(
            pool.insert(SignedTransaction::new(tx3, &key)),
            Err(MempoolError::PendingLimitExceeded { limit: 2, .. })
        ));
    }

    #[test]
    fn suggested_next_nonce_accounts_for_pending_same_sender() {
        let pool = Mempool::new();
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let to = AccountId([3u8; 32]);
        let tx0 = Transaction {
            nonce: 5,
            sender,
            payload: TransactionPayload::Transfer { to, amount: 1 },
            access_list: AccessList::default(),
        };
        pool.insert(SignedTransaction::new(tx0, &key)).unwrap();
        assert_eq!(pool.suggested_next_nonce(sender, 5), 6);
        assert_eq!(pool.suggested_next_nonce(sender, 4), 6);
    }

    #[test]
    fn suggested_next_nonce_empty_mempool_is_chain_nonce() {
        let pool = Mempool::new();
        let sender = AccountId([9u8; 32]);
        assert_eq!(pool.suggested_next_nonce(sender, 12), 12);
    }

    #[test]
    fn set_max_pending_per_sender_updates_limit() {
        let mut pool = Mempool::new().with_max_pending_per_sender(16);
        pool.set_max_pending_per_sender(1);
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let t0 = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::Transfer {
                to: AccountId([2u8; 32]),
                amount: 1,
            },
            access_list: AccessList::default(),
        };
        pool.insert(SignedTransaction::new(t0, &key)).unwrap();
        let t1 = Transaction {
            nonce: 1,
            sender,
            payload: TransactionPayload::Transfer {
                to: AccountId([2u8; 32]),
                amount: 1,
            },
            access_list: AccessList::default(),
        };
        assert!(matches!(
            pool.insert(SignedTransaction::new(t1, &key)),
            Err(MempoolError::PendingLimitExceeded { limit: 1, .. })
        ));
    }

    fn transfer(key: &SigningKey, nonce: u64) -> SignedTransaction {
        let sender = AccountId(key.verifying_key().to_bytes());
        SignedTransaction::new(
            Transaction {
                nonce,
                sender,
                payload: TransactionPayload::Transfer { to: AccountId([2u8; 32]), amount: 1 },
                access_list: AccessList::default(),
            },
            key,
        )
    }

    #[test]
    fn global_cap_evicts_from_heaviest_sender_and_rejects_when_fair() {
        // Security review #7.
        let pool = Mempool::new().with_max_total(6);
        let whale = SigningKey::generate(&mut OsRng);
        let whale_id = AccountId(whale.verifying_key().to_bytes());
        for n in 0..6 {
            pool.insert(transfer(&whale, n)).unwrap();
        }
        assert_eq!(pool.len(), 6);
        // newcomers displace the whale's newest txs
        let mut small = Vec::new();
        for _ in 0..3 {
            let k = SigningKey::generate(&mut OsRng);
            pool.insert(transfer(&k, 0)).unwrap();
            small.push(k);
        }
        assert_eq!(pool.len(), 6);
        assert_eq!(pool.suggested_next_nonce(whale_id, 0), 3, "nonces 3..5 evicted, 0..2 kept");
        // whale 3 vs a newcomer with 0: 3 > 0 + 1, so the whale loses one more
        let k4 = SigningKey::generate(&mut OsRng);
        pool.insert(transfer(&k4, 0)).unwrap();
        assert_eq!(pool.suggested_next_nonce(whale_id, 0), 2);
        // whale 2 vs newcomer 0: 2 > 0 + 1, still evicts
        let k5 = SigningKey::generate(&mut OsRng);
        pool.insert(transfer(&k5, 0)).unwrap();
        // every sender now holds 1: further newcomers are rejected, nobody is evicted
        let k6 = SigningKey::generate(&mut OsRng);
        assert!(matches!(
            pool.insert(transfer(&k6, 0)),
            Err(MempoolError::MempoolFull { limit: 6 })
        ));
        // a sender cannot evict others to grow beyond them
        assert!(matches!(
            pool.insert(transfer(&small[0], 1)),
            Err(MempoolError::MempoolFull { .. })
        ));
        assert_eq!(pool.len(), 6);
        // replacing an existing nonce is not an insertion and still works when full
        let k4_id = AccountId(k4.verifying_key().to_bytes());
        let replacement = SignedTransaction::new(
            Transaction {
                nonce: 0,
                sender: k4_id,
                payload: TransactionPayload::Transfer { to: AccountId([7u8; 32]), amount: 2 },
                access_list: AccessList::default(),
            },
            &k4,
        );
        pool.insert(replacement).unwrap();
        assert_eq!(pool.len(), 6);
    }

    #[test]
    fn reinsert_ignores_global_cap() {
        let pool = Mempool::new().with_max_total(2);
        let a = SigningKey::generate(&mut OsRng);
        pool.insert(transfer(&a, 0)).unwrap();
        pool.insert(transfer(&a, 1)).unwrap();
        let drained = pool.drain_for_block(2);
        let b = SigningKey::generate(&mut OsRng);
        let c = SigningKey::generate(&mut OsRng);
        pool.insert(transfer(&b, 0)).unwrap();
        pool.insert(transfer(&c, 0)).unwrap();
        pool.reinsert(drained);
        assert_eq!(pool.len(), 4, "txs of a failed block are restored even when the pool is full");
    }
}
