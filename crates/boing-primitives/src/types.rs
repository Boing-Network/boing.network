//! Core data structures for Boing blockchain.
//!
//! Account model with explicit access lists for parallel execution.

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

/// Access list — accounts this transaction reads/writes.
/// Enables parallel scheduling (transactions with disjoint access lists run in parallel).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessList {
    pub read: Vec<AccountId>,
    pub write: Vec<AccountId>,
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
        let payload_str = match &self.payload {
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
}

/// Transaction payload (extensible).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionPayload {
    Transfer { to: AccountId, amount: u128 },
    ContractCall { contract: AccountId, calldata: Vec<u8> },
    /// Contract deploy (legacy; no purpose declaration).
    ContractDeploy { bytecode: Vec<u8> },
    /// Contract deploy with optional QA purpose declaration (no deploy-time metadata).
    ContractDeployWithPurpose {
        bytecode: Vec<u8>,
        purpose_category: String,
        description_hash: Option<Vec<u8>>,
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
    /// Max length for asset_name in ContractDeployWithPurpose (UTF-8 bytes).
    pub const MAX_ASSET_NAME_LEN: usize = 256;
    /// Max length for asset_symbol in ContractDeployWithPurpose (UTF-8 bytes).
    pub const MAX_ASSET_SYMBOL_LEN: usize = 32;

    /// Returns (bytecode, purpose_category, description_hash, asset_name, asset_symbol) if this is a contract deploy payload.
    pub fn as_contract_deploy(&self) -> Option<ContractDeployFields<'_>> {
        match self {
            TransactionPayload::ContractDeploy { bytecode } => {
                Some((bytecode.as_slice(), None, None, None, None))
            }
            TransactionPayload::ContractDeployWithPurpose {
                bytecode,
                purpose_category,
                description_hash,
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
