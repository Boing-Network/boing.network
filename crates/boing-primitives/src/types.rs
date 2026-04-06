//! Core data structures for Boing blockchain.
//!
//! Account model with explicit access lists for parallel execution.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::hash::{Hash, hasher};

/// Account identifier (32 bytes, typically derived from pubkey).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct AccountId(pub [u8; 32]);

impl AccountId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        AccountId(bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Some(AccountId(arr))
    }
}

impl std::fmt::Debug for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AccountId({})", hex::encode(&self.0[..8]))
    }
}

/// Nonce-based contract address (legacy Boing deploy): `BLAKE3(sender || deploy_tx_nonce_le)`.
pub fn nonce_derived_contract_address(sender: &AccountId, deploy_tx_nonce: u64) -> AccountId {
    let mut h = hasher();
    h.update(&sender.0);
    h.update(&deploy_tx_nonce.to_le_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    AccountId(out)
}

const CREATE2_DOMAIN: &[u8] = b"boing.create2.v1\0";

/// Salt-derived contract address: `BLAKE3(domain || deployer || salt || BLAKE3(bytecode))`.
/// Shares the same 32-byte [`AccountId`] space as Ed25519-derived accounts; accidental collision is negligible.
pub fn create2_contract_address(deployer: &AccountId, salt: &[u8; 32], bytecode: &[u8]) -> AccountId {
    let mut ch = hasher();
    ch.update(bytecode);
    let code_hash = ch.finalize();
    let mut h = hasher();
    h.update(CREATE2_DOMAIN);
    h.update(&deployer.0);
    h.update(salt);
    h.update(code_hash.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    AccountId(out)
}

/// Access list — accounts this transaction reads/writes.
/// Enables parallel scheduling (transactions with disjoint access lists run in parallel).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessList {
    pub read: Vec<AccountId>,
    pub write: Vec<AccountId>,
}

fn suggested_access_list_for_contract_deploy(
    sender: AccountId,
    bytecode: &[u8],
    create2_salt: Option<[u8; 32]>,
) -> AccessList {
    let mut read = vec![sender];
    let mut write = vec![sender];
    if let Some(salt) = create2_salt {
        let addr = create2_contract_address(&sender, &salt, bytecode);
        read.push(addr);
        write.push(addr);
    }
    AccessList::new(read, write)
}

impl AccessList {
    pub fn new(read: Vec<AccountId>, write: Vec<AccountId>) -> Self {
        Self { read, write }
    }

    /// All accounts touched (read or write).
    pub fn all(&self) -> impl Iterator<Item = &AccountId> {
        self.read.iter().chain(self.write.iter())
    }

    /// True if this access list overlaps with another (shared account).
    pub fn conflicts_with(&self, other: &AccessList) -> bool {
        let a: std::collections::HashSet<_> = self.all().collect();
        let b: std::collections::HashSet<_> = other.all().collect();
        !a.is_disjoint(&b)
    }
}

/// Transaction — the unit of execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub nonce: u64,
    pub sender: AccountId,
    pub payload: TransactionPayload,
    pub access_list: AccessList,
}

impl Transaction {
    /// Human-readable summary for signing display in wallets.
    pub fn display_for_signing(&self) -> String {
        let mut payload_str = match &self.payload {
            TransactionPayload::Transfer { to, amount } => {
                format!("Transfer {} to {}", amount, hex::encode(&to.0[..8]))
            }
            TransactionPayload::Bond { amount } => format!("Bond {} stake", amount),
            TransactionPayload::Unbond { amount } => format!("Unbond {} stake", amount),
            TransactionPayload::ContractCall { contract, .. } => {
                format!("Call contract {}", hex::encode(&contract.0[..8]))
            }
            TransactionPayload::ContractDeploy { .. }
            | TransactionPayload::ContractDeployWithPurpose { .. }
            | TransactionPayload::ContractDeployWithPurposeAndMetadata { .. } => "Deploy contract".into(),
        };
        if self.payload.deploy_create2_salt().is_some() {
            payload_str = format!("{payload_str} (salt-derived)");
        }
        format!(
            "From: {} | Nonce: {} | {}",
            hex::encode(&self.sender.0[..8]),
            self.nonce,
            payload_str
        )
    }

    pub fn id(&self) -> Hash {
        let mut h = hasher();
        h.update(&bincode::serialize(self).unwrap_or_default());
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        Hash(out)
    }

    /// Heuristic minimum [`AccessList`] for parallel scheduling (see `TECHNICAL-SPECIFICATION.md` §4.2).
    /// Used by RPC simulation hints — does not reflect every storage key a contract may touch.
    pub fn suggested_parallel_access_list(&self) -> AccessList {
        match &self.payload {
            TransactionPayload::Transfer { to, .. } => {
                AccessList::new(vec![self.sender, *to], vec![self.sender, *to])
            }
            TransactionPayload::ContractCall { contract, .. } => AccessList::new(
                vec![self.sender, *contract],
                vec![self.sender, *contract],
            ),
            TransactionPayload::ContractDeploy {
                bytecode,
                create2_salt,
            } => suggested_access_list_for_contract_deploy(self.sender, bytecode, *create2_salt),
            TransactionPayload::ContractDeployWithPurpose {
                bytecode,
                create2_salt,
                ..
            } => suggested_access_list_for_contract_deploy(self.sender, bytecode, *create2_salt),
            TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode,
                create2_salt,
                ..
            } => suggested_access_list_for_contract_deploy(self.sender, bytecode, *create2_salt),
            TransactionPayload::Bond { .. } | TransactionPayload::Unbond { .. } => {
                AccessList::new(vec![self.sender], vec![self.sender])
            }
        }
    }

    /// `true` if every account in [`suggested_parallel_access_list`] appears in `access_list` (read or write).
    pub fn access_list_covers_parallel_suggestion(&self) -> bool {
        let sug = self.suggested_parallel_access_list();
        let have: HashSet<AccountId> = self.access_list.all().copied().collect();
        let need: HashSet<AccountId> = sug.all().copied().collect();
        need.is_subset(&have)
    }
}

/// Transaction payload (extensible).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionPayload {
    Transfer { to: AccountId, amount: u128 },
    ContractCall { contract: AccountId, calldata: Vec<u8> },
    /// Contract deploy (legacy; no purpose declaration).
    ContractDeploy {
        bytecode: Vec<u8>,
        /// When `Some`, contract address is [`create2_contract_address`] instead of [`nonce_derived_contract_address`].
        #[serde(default)]
        create2_salt: Option<[u8; 32]>,
    },
    /// Contract deploy with optional QA purpose declaration (no deploy-time metadata).
    ContractDeployWithPurpose {
        bytecode: Vec<u8>,
        purpose_category: String,
        description_hash: Option<Vec<u8>>,
        #[serde(default)]
        create2_salt: Option<[u8; 32]>,
    },
    /// Contract deploy with purpose and optional asset metadata (name/symbol) for content-policy checks.
    ContractDeployWithPurposeAndMetadata {
        bytecode: Vec<u8>,
        purpose_category: String,
        description_hash: Option<Vec<u8>>,
        /// Optional asset name (e.g. token name). Checked against governance content blocklist. Max 256 bytes UTF-8.
        asset_name: Option<String>,
        /// Optional asset symbol (e.g. ticker). Checked against governance content blocklist. Max 32 bytes UTF-8.
        asset_symbol: Option<String>,
        #[serde(default)]
        create2_salt: Option<[u8; 32]>,
    },
    /// Bond stake to become/l remain a validator.
    Bond { amount: u128 },
    /// Unbond stake (with optional unbonding period).
    Unbond { amount: u128 },
}

/// Borrowed fields exposed by [`TransactionPayload::as_contract_deploy`].
pub type ContractDeployFields<'a> = (
    &'a [u8],
    Option<&'a str>,
    Option<&'a [u8]>,
    Option<&'a str>,
    Option<&'a str>,
);

impl TransactionPayload {
    /// Salt for salt-derived deploy, if this payload is a contract deploy variant.
    pub fn deploy_create2_salt(&self) -> Option<[u8; 32]> {
        match self {
            TransactionPayload::ContractDeploy { create2_salt, .. } => *create2_salt,
            TransactionPayload::ContractDeployWithPurpose { create2_salt, .. } => *create2_salt,
            TransactionPayload::ContractDeployWithPurposeAndMetadata { create2_salt, .. } => *create2_salt,
            _ => None,
        }
    }

    /// Max length for asset_name in ContractDeployWithPurpose (UTF-8 bytes).
    pub const MAX_ASSET_NAME_LEN: usize = 256;
    /// Max length for asset_symbol in ContractDeployWithPurpose (UTF-8 bytes).
    pub const MAX_ASSET_SYMBOL_LEN: usize = 32;

    /// Returns (bytecode, purpose_category, description_hash, asset_name, asset_symbol) if this is a contract deploy payload.
    pub fn as_contract_deploy(&self) -> Option<ContractDeployFields<'_>> {
        match self {
            TransactionPayload::ContractDeploy { bytecode, .. } => {
                Some((bytecode.as_slice(), None, None, None, None))
            }
            TransactionPayload::ContractDeployWithPurpose {
                bytecode,
                purpose_category,
                description_hash,
                ..
            } => Some((
                bytecode.as_slice(),
                Some(purpose_category.as_str()),
                description_hash.as_deref(),
                None,
                None,
            )),
            TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode,
                purpose_category,
                description_hash,
                asset_name,
                asset_symbol,
                ..
            } => Some((
                bytecode.as_slice(),
                Some(purpose_category.as_str()),
                description_hash.as_deref(),
                asset_name.as_deref(),
                asset_symbol.as_deref(),
            )),
            _ => None,
        }
    }
}

/// Account state — balance, nonce, and staked amount.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    pub balance: u128,
    pub nonce: u64,
    /// Staked balance (bonded for validation).
    pub stake: u128,
}

/// Full account (id + state).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub state: AccountState,
}

/// Compute Merkle root of transaction IDs (simple binary tree).
pub fn tx_root(txs: &[Transaction]) -> Hash {
    if txs.is_empty() {
        return Hash::ZERO;
    }
    let leaves: Vec<Hash> = txs.iter().map(|tx| tx.id()).collect();
    merkle_root_impl(&leaves)
}

/// BLAKE3 hash of `bincode(receipt)` — one leaf in [`receipts_root`].
pub fn receipt_leaf_hash(receipt: &ExecutionReceipt) -> Hash {
    let mut h = crate::hash::hasher();
    h.update(&bincode::serialize(receipt).unwrap_or_default());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    Hash(out)
}

/// Merkle root of execution receipts (same tree shape as [`tx_root`]); empty → [`Hash::ZERO`].
pub fn receipts_root(receipts: &[ExecutionReceipt]) -> Hash {
    if receipts.is_empty() {
        return Hash::ZERO;
    }
    let leaves: Vec<Hash> = receipts.iter().map(receipt_leaf_hash).collect();
    merkle_root_impl(&leaves)
}

fn merkle_root_impl(hashes: &[Hash]) -> Hash {
    if hashes.len() == 1 {
        return hashes[0];
    }
    let mut next = Vec::new();
    for chunk in hashes.chunks(2) {
        let h = if chunk.len() == 2 {
            hash_pair(&chunk[0], &chunk[1])
        } else {
            hash_pair(&chunk[0], &chunk[0])
        };
        next.push(h);
    }
    merkle_root_impl(&next)
}

fn hash_pair(a: &Hash, b: &Hash) -> Hash {
    let mut h = crate::hash::hasher();
    h.update(a.as_bytes());
    h.update(b.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    Hash(out)
}

/// Block header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub parent_hash: Hash,
    pub height: u64,
    pub timestamp: u64,
    pub proposer: AccountId,
    pub tx_root: Hash,
    /// Merkle root over receipt leaves ([`receipt_leaf_hash`]) in transaction order.
    pub receipts_root: Hash,
    pub state_root: Hash,
}

/// Block — header + transactions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn hash(&self) -> Hash {
        let mut h = hasher();
        h.update(&bincode::serialize(&self.header).unwrap_or_default());
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        Hash(out)
    }
}

/// Max `return_data` bytes stored per receipt (indexer / RPC bound).
pub const MAX_RECEIPT_RETURN_DATA_BYTES: usize = 24 * 1024;
/// Max UTF-8 bytes for optional `error` on failed receipts.
pub const MAX_RECEIPT_ERROR_STRING_BYTES: usize = 2048;
/// Max indexed topics per log (Boing VM `LOG0`..`LOG4`).
pub const MAX_EXECUTION_LOG_TOPICS: usize = 4;
/// Max payload bytes per log entry.
pub const MAX_EXECUTION_LOG_DATA_BYTES: usize = 1024;
/// Max log entries per transaction (receipt bound).
pub const MAX_EXECUTION_LOGS_PER_TX: usize = 24;

/// Leading byte of `ContractDeploy` (all variants) **payload**: the remainder is **init code**.
///
/// The node runs init once at deploy (`CALLER` = tx sender, `ADDRESS` = new contract). Logs are
/// recorded on the deploy transaction receipt. The memory slice passed to `RETURN` becomes the
/// **runtime bytecode** stored for the contract (empty slice if execution ends via `STOP` without
/// `RETURN`). Payloads **without** this prefix keep legacy behavior: bytes are stored verbatim and
/// no VM execution runs at deploy.
///
/// `0xFD` is not a valid Boing VM opcode; it is stripped before QA bytecode walks and init execution.
pub const CONTRACT_DEPLOY_INIT_CODE_MARKER: u8 = 0xFD;

/// `true` if deploy bytecode uses [`CONTRACT_DEPLOY_INIT_CODE_MARKER`] (init + `RETURN` → runtime).
#[inline]
pub fn contract_deploy_uses_init_code(bytecode: &[u8]) -> bool {
    bytecode.first().copied() == Some(CONTRACT_DEPLOY_INIT_CODE_MARKER)
}

/// Bytecode validated at deploy and passed to the interpreter for init (marker stripped when present).
#[inline]
pub fn contract_deploy_init_body(bytecode: &[u8]) -> &[u8] {
    if contract_deploy_uses_init_code(bytecode) {
        bytecode.get(1..).unwrap_or(&[])
    } else {
        bytecode
    }
}

/// One log entry emitted during contract execution (indexer-facing; no bloom in header).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

fn sanitize_logs(mut logs: Vec<ExecutionLog>) -> Vec<ExecutionLog> {
    logs.truncate(MAX_EXECUTION_LOGS_PER_TX);
    for log in logs.iter_mut() {
        log.topics.truncate(MAX_EXECUTION_LOG_TOPICS);
        if log.data.len() > MAX_EXECUTION_LOG_DATA_BYTES {
            log.data.truncate(MAX_EXECUTION_LOG_DATA_BYTES);
        }
    }
    logs
}

/// Result of applying one transaction in a block (execution summary for indexers; no log bloom yet).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub tx_id: Hash,
    pub block_height: u64,
    pub tx_index: u32,
    pub success: bool,
    pub gas_used: u64,
    pub return_data: Vec<u8>,
    /// VM logs from `ContractCall`, or from `ContractDeploy` when init-code mode is used
    /// ([`CONTRACT_DEPLOY_INIT_CODE_MARKER`]); otherwise empty (e.g. transfers, legacy deploy).
    pub logs: Vec<ExecutionLog>,
    pub error: Option<String>,
}

impl ExecutionReceipt {
    pub fn from_tx_outcome(
        tx: &Transaction,
        block_height: u64,
        tx_index: u32,
        success: bool,
        gas_used: u64,
        return_data: Vec<u8>,
        logs: Vec<ExecutionLog>,
        error: Option<String>,
    ) -> Self {
        let mut return_data = return_data;
        if return_data.len() > MAX_RECEIPT_RETURN_DATA_BYTES {
            return_data.truncate(MAX_RECEIPT_RETURN_DATA_BYTES);
        }
        let error = error.and_then(|mut s| {
            if s.len() > MAX_RECEIPT_ERROR_STRING_BYTES {
                s.truncate(MAX_RECEIPT_ERROR_STRING_BYTES);
            }
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        });
        Self {
            tx_id: tx.id(),
            block_height,
            tx_index,
            success,
            gas_used,
            return_data,
            logs: sanitize_logs(logs),
            error,
        }
    }
}

#[cfg(test)]
mod receipt_root_tests {
    use super::*;

    #[test]
    fn receipts_root_empty_is_zero() {
        assert_eq!(receipts_root(&[]), Hash::ZERO);
    }

    #[test]
    fn receipts_root_order_matters() {
        let id = AccountId([9u8; 32]);
        let tx1 = Transaction {
            nonce: 0,
            sender: id,
            payload: TransactionPayload::Transfer {
                to: id,
                amount: 1,
            },
            access_list: AccessList::default(),
        };
        let tx2 = Transaction {
            nonce: 1,
            sender: id,
            payload: TransactionPayload::Transfer {
                to: id,
                amount: 2,
            },
            access_list: AccessList::default(),
        };
        let r1 = ExecutionReceipt::from_tx_outcome(&tx1, 1, 0, true, 100, vec![], vec![], None);
        let r2 = ExecutionReceipt::from_tx_outcome(&tx2, 1, 1, true, 200, vec![1], vec![], None);
        let a = receipts_root(&[r1.clone(), r2.clone()]);
        let b = receipts_root(&[r2, r1]);
        assert_ne!(a, b);
    }
}
