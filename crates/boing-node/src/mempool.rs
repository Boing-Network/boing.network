//! Transaction mempool — pending transactions awaiting inclusion in a block.
//! Runs protocol QA on ContractDeploy before accepting; rejects with structured reason when QA fails.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use boing_primitives::{AccountId, Hash, SignedTransaction};
use boing_qa::{check_contract_deploy_full_with_metadata, QaReject, QaResult, RuleRegistry};

/// Default max pending transactions per sender (matches SECURITY-STANDARDS / RateLimitConfig).
pub const DEFAULT_MAX_PENDING_PER_SENDER: usize = 16;

/// Legacy pending cap for **zero-tip** txs only when execution gas is fully waived (`bps == 0`).
/// Prefer a micro base fee ([`boing_tokenomics::MICRO_FEE_EXECUTION_GAS_BPS`]) so this path stays off.
pub const DEFAULT_MAX_FREE_PENDING_PER_SENDER: usize = 4;

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
    /// Cap on zero-tip pending txs per sender (optional-fee anti-spam). Ignored when
    /// [`Self::optional_fees_active`] is false.
    max_free_pending_per_sender: usize,
    /// Global cap on pending txs. When full, eviction prefers lowest-tip then highest-nonce of the
    /// busiest other sender; otherwise reject with [`MempoolError::MempoolFull`].
    max_total: usize,
    /// QA rule registry (blocklist, content blocklist, etc.). Replace via [Mempool::with_qa_registry] or governance.
    qa_registry: RuleRegistry,
    /// Deploy QA accepts the native-value opcodes (set by the node from the next block height).
    native_value_opcodes: std::sync::atomic::AtomicBool,
    /// When true (execution gas fully waived), zero-tip txs use the legacy free-pending cap.
    /// Tip-aware drain/eviction always runs regardless of this flag.
    optional_fees_active: std::sync::atomic::AtomicBool,
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
            max_free_pending_per_sender: DEFAULT_MAX_FREE_PENDING_PER_SENDER,
            max_total: max_total_from_env(),
            qa_registry: default_qa_registry(),
            native_value_opcodes: std::sync::atomic::AtomicBool::new(false),
            optional_fees_active: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Accept (`true`) the native-value opcodes in deploy QA: the node sets this when the next block
    /// is at/after the native-value activation height, so earlier deploys are checked as before.
    pub fn set_native_value_opcodes(&self, allow: bool) {
        self.native_value_opcodes
            .store(allow, std::sync::atomic::Ordering::Relaxed);
    }

    /// Enable legacy free-tx pending caps (only when execution gas is fully waived).
    /// Tip ordering does not depend on this flag.
    pub fn set_optional_fees_active(&self, active: bool) {
        self.optional_fees_active
            .store(active, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn optional_fees_active(&self) -> bool {
        self.optional_fees_active
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn with_max_pending_per_sender(mut self, max: usize) -> Self {
        self.max_pending_per_sender = max.max(1);
        self
    }

    pub fn with_max_free_pending_per_sender(mut self, max: usize) -> Self {
        self.max_free_pending_per_sender = max.max(1);
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

    /// Ed25519 signature check under the current process-wide tx-verify policy.
    /// Prefer [`Self::verify_signature_with`] on admission paths (issue #61 item 34).
    pub fn verify_signature(signed: &SignedTransaction) -> Result<(), MempoolError> {
        Self::verify_signature_with(signed, boing_primitives::tx_verify_policy())
    }

    /// Ed25519 signature check under an explicit policy (avoids TOCTOU vs process-wide atomics).
    pub fn verify_signature_with(
        signed: &SignedTransaction,
        policy: boing_primitives::TxSigPolicy,
    ) -> Result<(), MempoolError> {
        signed.verify_with(policy).map_err(|e| match e {
            boing_primitives::SignatureError::LegacySignatureRejected => {
                MempoolError::LegacySignature
            }
            _ => MempoolError::InvalidSignature,
        })
    }

    /// Insert a signed transaction. Rejects duplicates, invalid nonces, per-sender cap, and ContractDeploy that fail QA.
    pub fn insert(&self, signed: SignedTransaction) -> Result<(), MempoolError> {
        self.insert_with_policy(signed, boing_primitives::tx_verify_policy())
    }

    /// Like [`Self::insert`], but verifies under `policy` instead of the process-wide value.
    pub fn insert_with_policy(
        &self,
        signed: SignedTransaction,
        policy: boing_primitives::TxSigPolicy,
    ) -> Result<(), MempoolError> {
        self.insert_inner(signed, false, policy)
    }

    /// Insert after community QA pool approved (`Unsure` path). Skips deploy-time registry QA so the tx does not loop as Unsure.
    pub fn insert_after_pool_allow(&self, signed: SignedTransaction) -> Result<(), MempoolError> {
        self.insert_after_pool_allow_with_policy(signed, boing_primitives::tx_verify_policy())
    }

    /// Like [`Self::insert_after_pool_allow`] with an explicit signature policy.
    pub fn insert_after_pool_allow_with_policy(
        &self,
        signed: SignedTransaction,
        policy: boing_primitives::TxSigPolicy,
    ) -> Result<(), MempoolError> {
        self.insert_inner(signed, true, policy)
    }

    fn insert_inner(
        &self,
        signed: SignedTransaction,
        skip_deploy_qa: bool,
        policy: boing_primitives::TxSigPolicy,
    ) -> Result<(), MempoolError> {
        self.insert_with_qa_mode(
            signed,
            if skip_deploy_qa {
                DeployQaMode::Skip
            } else {
                DeployQaMode::Enforce
            },
            policy,
        )
    }

    /// Admit an Unsure deploy so the producer can register it on-chain (public membership).
    pub fn insert_unsure_for_inclusion(
        &self,
        signed: SignedTransaction,
    ) -> Result<(), MempoolError> {
        self.insert_unsure_for_inclusion_with_policy(signed, boing_primitives::tx_verify_policy())
    }

    /// Like [`Self::insert_unsure_for_inclusion`] with an explicit signature policy.
    pub fn insert_unsure_for_inclusion_with_policy(
        &self,
        signed: SignedTransaction,
        policy: boing_primitives::TxSigPolicy,
    ) -> Result<(), MempoolError> {
        self.insert_with_qa_mode(signed, DeployQaMode::AdmitUnsure, policy)
    }

    fn insert_with_qa_mode(
        &self,
        signed: SignedTransaction,
        qa_mode: DeployQaMode,
        policy: boing_primitives::TxSigPolicy,
    ) -> Result<(), MempoolError> {
        self.insert_checked(signed, qa_mode, false, policy)
    }

    /// `restoring`: txs that were already in the pool (re-inserted after a failed block) skip the global
    /// cap, so a full pool never drops them.
    fn insert_checked(
        &self,
        signed: SignedTransaction,
        qa_mode: DeployQaMode,
        restoring: bool,
        policy: boing_primitives::TxSigPolicy,
    ) -> Result<(), MempoolError> {
        Self::verify_signature_with(&signed, policy)?;
        signed
            .tx
            .payload
            .validate_batch()
            .map_err(MempoolError::InvalidBatch)?;
        if qa_mode != DeployQaMode::Skip {
            if let Some((bytecode, purpose, desc_hash, asset_name, asset_symbol)) =
                signed.tx.payload.as_contract_deploy()
            {
                let native;
                let registry = if self
                    .native_value_opcodes
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    native = self.qa_registry.clone().with_native_value_opcodes(true);
                    &native
                } else {
                    &self.qa_registry
                };
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
        let tip = signed.priority_tip;
        let mut inner = self.inner.lock().unwrap();
        if inner.by_id.contains_key(&tx_id) {
            return Err(MempoolError::Duplicate);
        }
        let sender = signed.tx.sender;
        let nonce = signed.tx.nonce;
        // Replacements must strictly raise the tip so nodes converge on the same pending tx
        // (previously any signed replacement could undercut the fee).
        if let Some(existing) = inner.by_sender.get(&sender).and_then(|m| m.get(&nonce)) {
            if tip <= existing.priority_tip {
                return Err(MempoolError::UnderpricedReplacement {
                    have: existing.priority_tip,
                    got: tip,
                });
            }
        }
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
            // Free-path anti-spam: tighter pending cap when tip is zero and optional fees are on.
            if tip == 0 && self.optional_fees_active() {
                let free_count = inner
                    .by_sender
                    .get(&sender)
                    .map(|m| m.values().filter(|s| s.priority_tip == 0).count())
                    .unwrap_or(0);
                if free_count >= self.max_free_pending_per_sender {
                    return Err(MempoolError::FreePendingLimitExceeded {
                        sender,
                        limit: self.max_free_pending_per_sender,
                    });
                }
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

    /// Make room for one tx from `newcomer` (holding `newcomer_count` pending): evict the lowest-tip
    /// (then highest-nonce) tx of the sender with the most pending txs (ties: highest account id)
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
                // Prefer dropping the lowest tip; among equals, highest nonce (newest).
                let evict_nonce = by_nonce
                    .iter()
                    .min_by(|a, b| {
                        a.1.priority_tip
                            .cmp(&b.1.priority_tip)
                            .then_with(|| b.0.cmp(a.0))
                    })
                    .map(|(n, _)| *n)
                    .expect("non-empty");
                let evicted = by_nonce.remove(&evict_nonce).expect("nonce present");
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
    ///
    /// Ordering: a k-way merge over per-sender queues. Each sender's txs are always emitted in
    /// nonce order (a later nonce can never precede an earlier one, so a high tip on nonce 5 cannot
    /// make it execute before nonce 3 and fail). Among the senders' next-ready txs, higher
    /// `priority_tip` goes first, then lower nonce, then sender id (deterministic). With all tips `0`
    /// this is exactly the historical (nonce, sender) order.
    ///
    /// Cost is `O(max * log(senders))` rather than sorting every pending tx each block.
    pub fn drain_for_block(&self, max: usize) -> Vec<SignedTransaction> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let mut inner = self.inner.lock().unwrap();
        // Max-heap on (tip, Reverse(nonce), Reverse(sender)): highest tip, then lowest nonce, then lowest sender.
        let mut heap: BinaryHeap<(u128, Reverse<u64>, Reverse<[u8; 32]>)> = BinaryHeap::new();
        for (sender, by_nonce) in inner.by_sender.iter() {
            if let Some((nonce, signed)) = by_nonce.iter().next() {
                heap.push((signed.priority_tip, Reverse(*nonce), Reverse(sender.0)));
            }
        }
        let mut out = Vec::with_capacity(max.min(inner.len));
        while out.len() < max {
            let Some((_tip, Reverse(nonce), Reverse(sender_bytes))) = heap.pop() else {
                break;
            };
            let sender = AccountId(sender_bytes);
            let (signed, next) = {
                let Some(by_nonce) = inner.by_sender.get_mut(&sender) else {
                    continue;
                };
                let Some(signed) = by_nonce.remove(&nonce) else {
                    continue;
                };
                let next = by_nonce
                    .iter()
                    .next()
                    .map(|(n, s)| (s.priority_tip, Reverse(*n), Reverse(sender.0)));
                if by_nonce.is_empty() {
                    inner.by_sender.remove(&sender);
                }
                (signed, next)
            };
            inner.by_id.remove(&signed.tx.id());
            inner.len = inner.len.saturating_sub(1);
            out.push(signed);
            if let Some(n) = next {
                heap.push(n);
            }
        }
        out
    }

    /// Re-insert signed transactions (e.g. after block production or consensus failure).
    /// Duplicates are skipped; invalid signatures are skipped. Used to restore txs when a block is not committed.
    pub fn reinsert(&self, signed_txs: Vec<SignedTransaction>) {
        let policy = boing_primitives::tx_verify_policy();
        for signed in signed_txs {
            let _ = self.insert_checked(signed, DeployQaMode::Skip, true, policy);
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
    #[error(
        "Free-tx pending limit exceeded: sender has too many zero-tip pending txs (max {limit}); attach a priority tip or wait"
    )]
    FreePendingLimitExceeded { sender: AccountId, limit: usize },
    #[error("Mempool full ({limit} pending txs); try again later")]
    MempoolFull { limit: usize },
    /// Same (sender, nonce) replacement must carry a strictly higher `priority_tip`.
    #[error(
        "underpriced replacement: pending tip is {have}, replacement tip is {got} (must be higher)"
    )]
    UnderpricedReplacement { have: u128, got: u128 },
    #[error("insufficient balance for fee: have {have}, need {need}")]
    InsufficientFee { have: u128, need: u128 },
    /// The tx nonce is below the sender's committed nonce, so it can never execute.
    #[error("stale nonce: account nonce is {expected}, tx nonce is {got}")]
    StaleNonce { expected: u64, got: u64 },
    /// The sender has no account on chain (nothing to charge, nothing that could execute).
    #[error("unknown sender: account has no on-chain balance; fund it before sending")]
    UnknownSender,
    #[error("free tx rejected: sender balance {have} below free_tx_min_balance {need}")]
    FreeTxBalanceGate { have: u128, need: u128 },
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
    /// PutBlob rejected (feature off, oversize, bad retention, …).
    #[error("media blob rejected: {0}")]
    MediaBlobRejected(String),
    /// Governance consensus tx rejected (activation / allowlist / phase).
    #[error("governance tx rejected: {0}")]
    GovernanceRejected(String),
    /// Native value on contract calls before the activation height.
    #[error("native value on contract calls is not active until height {from}")]
    NativeValueNotActive { from: u64 },
    /// Gossip path refused an expensive VM precheck under load (issue #61 item 18).
    #[error("gossip precheck throttled; try again later")]
    GossipPrecheckThrottled,
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
                payload: TransactionPayload::Transfer {
                    to: AccountId([2u8; 32]),
                    amount: 1,
                },
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
        assert_eq!(
            pool.suggested_next_nonce(whale_id, 0),
            3,
            "nonces 3..5 evicted, 0..2 kept"
        );
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
        // same-tip replacement is rejected; a higher tip still works when the pool is full
        let k4_id = AccountId(k4.verifying_key().to_bytes());
        let same_tip = SignedTransaction::new(
            Transaction {
                nonce: 0,
                sender: k4_id,
                payload: TransactionPayload::Transfer {
                    to: AccountId([7u8; 32]),
                    amount: 2,
                },
                access_list: AccessList::default(),
            },
            &k4,
        );
        assert!(matches!(
            pool.insert(same_tip),
            Err(MempoolError::UnderpricedReplacement { .. })
        ));
        const CHAIN: u64 = 6913;
        boing_primitives::set_tx_verify_policy(boing_primitives::TxSigPolicy {
            chain_id: Some(CHAIN),
            require_v2: false,
        });
        let higher = SignedTransaction::new_for_chain_with_tip(
            Transaction {
                nonce: 0,
                sender: k4_id,
                payload: TransactionPayload::Transfer {
                    to: AccountId([8u8; 32]),
                    amount: 3,
                },
                access_list: AccessList::default(),
            },
            &k4,
            CHAIN,
            1,
        );
        pool.insert(higher).unwrap();
        assert_eq!(pool.len(), 6);
    }

    #[test]
    fn replacement_rejects_lower_or_equal_tip() {
        const CHAIN: u64 = 6913;
        boing_primitives::set_tx_verify_policy(boing_primitives::TxSigPolicy {
            chain_id: Some(CHAIN),
            require_v2: false,
        });
        let pool = Mempool::new();
        let key = SigningKey::generate(&mut OsRng);
        let id = AccountId(key.verifying_key().to_bytes());
        let mk = |to: u8, tip: u128| {
            SignedTransaction::new_for_chain_with_tip(
                Transaction {
                    nonce: 0,
                    sender: id,
                    payload: TransactionPayload::Transfer {
                        to: AccountId([to; 32]),
                        amount: 1,
                    },
                    access_list: AccessList::default(),
                },
                &key,
                CHAIN,
                tip,
            )
        };
        pool.insert(mk(1, 5)).unwrap();
        assert!(matches!(
            pool.insert(mk(2, 4)),
            Err(MempoolError::UnderpricedReplacement { have: 5, got: 4 })
        ));
        assert!(matches!(
            pool.insert(mk(3, 5)),
            Err(MempoolError::UnderpricedReplacement { have: 5, got: 5 })
        ));
        pool.insert(mk(4, 6)).unwrap();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn drain_untipped_matches_nonce_then_sender_order_and_respects_max() {
        let pool = Mempool::new();
        let keys: Vec<SigningKey> = (0..5).map(|_| SigningKey::generate(&mut OsRng)).collect();
        for k in &keys {
            for n in 0..3u64 {
                pool.insert(transfer(k, n)).unwrap();
            }
        }
        let first = pool.drain_for_block(7);
        assert_eq!(first.len(), 7);
        let keyed: Vec<(u64, [u8; 32])> =
            first.iter().map(|s| (s.tx.nonce, s.tx.sender.0)).collect();
        let mut sorted = keyed.clone();
        sorted.sort();
        assert_eq!(keyed, sorted, "untipped order is (nonce, sender)");
        assert_eq!(pool.len(), 8);
        let rest = pool.drain_for_block(100);
        assert_eq!(rest.len(), 8);
        assert!(pool.is_empty());
        for k in &keys {
            let id = AccountId(k.verifying_key().to_bytes());
            let all: Vec<u64> = first
                .iter()
                .chain(rest.iter())
                .filter(|s| s.tx.sender == id)
                .map(|s| s.tx.nonce)
                .collect();
            assert_eq!(all, vec![0, 1, 2]);
        }
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
        assert_eq!(
            pool.len(),
            4,
            "txs of a failed block are restored even when the pool is full"
        );
    }
}
