//! State store — Sparse Merkle tree for compact proofs.

use std::collections::HashMap;

use boing_primitives::{Account, AccountId, AccountState, Hash};

use crate::qa_pending::QaPendingRecord;
use crate::sparse_merkle::{
    hash_account_leaf, hash_contract_code, SparseMerkleTree,
};

/// Type alias for persisted contract storage entries: ((contract, key), value).
pub type ContractStorageEntry = ((AccountId, [u8; 32]), [u8; 32]);

/// Chain-wide sums over the committed account table (balance / stake fields only).
///
/// This is **not** “circulating supply”, “total minted”, or protocol treasury accounting; it is the
/// sum of per-account `balance` and `stake` as held in state. Totals use saturating arithmetic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChainNativeAggregates {
    pub account_count: u64,
    pub total_balance: u128,
    pub total_stake: u128,
    /// `total_balance + total_stake` (saturating); same as summing `(balance + stake)` per account in plain integer math.
    pub total_native_held: u128,
}

/// Return type of `export_for_persistence`: accounts, contract code, contract storage.
pub type PersistenceExport = (
    Vec<(AccountId, AccountState)>,
    Vec<(AccountId, Vec<u8>)>,
    Vec<ContractStorageEntry>,
);

/// Checkpoint handle for revert. Created by `checkpoint()`.
#[derive(Clone)]
pub struct StateCheckpoint {
    accounts: HashMap<AccountId, AccountState>,
    contract_code: HashMap<AccountId, Vec<u8>>,
    contract_storage: HashMap<(AccountId, [u8; 32]), [u8; 32]>,
    qa_pending: HashMap<Hash, QaPendingRecord>,
}

/// State store with Sparse Merkle tree for state_root.
#[derive(Default)]
pub struct StateStore {
    accounts: HashMap<AccountId, AccountState>,
    tree: SparseMerkleTree,
    /// Contract bytecode by account.
    pub contract_code: HashMap<AccountId, Vec<u8>>,
    /// Contract storage: (contract, key) -> value.
    pub contract_storage: HashMap<(AccountId, [u8; 32]), [u8; 32]>,
    /// On-chain Unsure QA items (public-membership path).
    qa_pending: HashMap<Hash, QaPendingRecord>,
}

impl StateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: &AccountId) -> Option<&AccountState> {
        self.accounts.get(id)
    }

    pub fn get_mut(&mut self, id: &AccountId) -> Option<&mut AccountState> {
        self.accounts.get_mut(id)
    }

    pub fn insert(&mut self, account: Account) {
        self.accounts.insert(account.id, account.state);
        // Tree is rebuilt on `state_root()` so code/storage commitments stay consistent.
    }

    pub fn set_contract_code(&mut self, account: AccountId, bytecode: Vec<u8>) {
        self.contract_code.insert(account, bytecode);
    }

    pub fn get_contract_code(&self, account: &AccountId) -> Option<&Vec<u8>> {
        self.contract_code.get(account)
    }

    /// One 32-byte storage slot for `contract` (Boing VM `SLOAD` semantics; missing → zero word).
    pub fn get_contract_storage(&self, contract: &AccountId, key: &[u8; 32]) -> [u8; 32] {
        self.contract_storage
            .get(&(*contract, *key))
            .copied()
            .unwrap_or([0u8; 32])
    }

    /// Per-contract storage SMT root (`Hash::ZERO` if the contract has no slots).
    pub fn storage_root_of(&self, contract: &AccountId) -> Hash {
        let mut tree = SparseMerkleTree::new();
        let mut any = false;
        for ((c, key), value) in &self.contract_storage {
            if c == contract {
                any = true;
                tree.insert_storage_slot(*key, value);
            }
        }
        if any {
            tree.root()
        } else {
            Hash::ZERO
        }
    }

    /// Code hash for an account (`Hash::ZERO` if undeployed / empty).
    pub fn code_hash_of(&self, account: &AccountId) -> Hash {
        match self.contract_code.get(account) {
            Some(code) => hash_contract_code(code),
            None => Hash::ZERO,
        }
    }

    /// Compute state root from Sparse Merkle tree. Rebuilds tree from current
    /// accounts; each leaf commits to balance/nonce/stake **and** code_hash + storage_root.
    /// Also includes accounts that have code or storage but no balance entry yet.
    pub fn state_root(&mut self) -> Hash {
        self.tree = SparseMerkleTree::new();
        let mut ids: std::collections::HashSet<AccountId> =
            self.accounts.keys().copied().collect();
        for id in self.contract_code.keys() {
            ids.insert(*id);
        }
        for ((c, _), _) in &self.contract_storage {
            ids.insert(*c);
        }
        for id in ids {
            let state = self
                .accounts
                .get(&id)
                .cloned()
                .unwrap_or(AccountState {
                    balance: 0,
                    nonce: 0,
                    stake: 0, ..Default::default() });
            let code_hash = self.code_hash_of(&id);
            let storage_root = self.storage_root_of(&id);
            let leaf = hash_account_leaf(&state, &code_hash, &storage_root);
            self.tree.insert_leaf_hash(id.0, leaf);
        }
        for (subject, rec) in &self.qa_pending {
            self.tree
                .insert_leaf_hash(QaPendingRecord::smt_key(subject), rec.commitment_hash());
        }
        self.tree.root()
    }

    /// Merge account state from parallel execution view.
    pub fn merge_account(&mut self, id: AccountId, state: AccountState) {
        self.accounts.insert(id, state);
    }

    /// Merge contract storage from parallel execution view.
    pub fn merge_contract_storage(&mut self, contract: AccountId, key: [u8; 32], value: [u8; 32]) {
        self.contract_storage.insert((contract, key), value);
    }

    /// Snapshot current state for simulation (includes contract code and storage).
    pub fn snapshot(&self) -> StateStore {
        let mut out = StateStore::new();
        for (id, st) in &self.accounts {
            out.insert(boing_primitives::Account { id: *id, state: st.clone() });
        }
        for (id, code) in &self.contract_code {
            out.set_contract_code(*id, code.clone());
        }
        for ((contract, key), value) in &self.contract_storage {
            out.merge_contract_storage(*contract, *key, *value);
        }
        out.qa_pending = self.qa_pending.clone();
        out
    }

    /// Create a checkpoint of current state for revert.
    pub fn checkpoint(&self) -> StateCheckpoint {
        StateCheckpoint {
            accounts: self.accounts.iter().map(|(k, v)| (*k, v.clone())).collect(),
            contract_code: self.contract_code.iter().map(|(k, v)| (*k, v.clone())).collect(),
            contract_storage: self.contract_storage.iter().map(|(k, v)| (*k, *v)).collect(),
            qa_pending: self.qa_pending.clone(),
        }
    }

    /// Revert state to checkpoint (e.g. after failed block execution).
    pub fn revert(&mut self, cp: StateCheckpoint) {
        boing_telemetry::component_debug(
            "boing_state::store",
            "state",
            "reverted_to_checkpoint",
            "state store reverted to checkpoint",
        );
        self.accounts = cp.accounts;
        self.contract_code = cp.contract_code;
        self.contract_storage = cp.contract_storage;
        self.qa_pending = cp.qa_pending;
        self.tree = SparseMerkleTree::new();
        let _ = self.state_root();
    }

    /// Generate Merkle proof for an account. Ensures tree is synced (includes code/storage).
    pub fn prove_account(&mut self, id: &AccountId) -> Option<crate::MerkleProof> {
        self.state_root(); // sync tree with accounts + code/storage commitments
        self.tree.prove(id)
    }

    /// Merkle proof for a single contract storage slot against the contract's storage SMT.
    /// Returns `None` if the slot is absent (zero) or the contract has no storage.
    pub fn prove_storage(
        &self,
        contract: &AccountId,
        slot: &[u8; 32],
    ) -> Option<crate::MerkleProof> {
        if !self.contract_storage.contains_key(&(*contract, *slot)) {
            return None;
        }
        let mut tree = SparseMerkleTree::new();
        for ((c, key), value) in &self.contract_storage {
            if c == contract {
                tree.insert_storage_slot(*key, value);
            }
        }
        tree.prove(&AccountId(*slot))
    }

    /// Export state for disk persistence.
    pub fn export_for_persistence(&self) -> PersistenceExport {
        let accounts: Vec<_> = self.accounts.iter().map(|(k, v)| (*k, v.clone())).collect();
        let contract_code: Vec<_> = self.contract_code.iter().map(|(k, v)| (*k, v.clone())).collect();
        let contract_storage: Vec<_> = self.contract_storage.iter().map(|(k, v)| (*k, *v)).collect();
        (accounts, contract_code, contract_storage)
    }

    /// Load state from persisted data.
    pub fn load_from_persistence(
        accounts: Vec<(AccountId, AccountState)>,
        contract_code: Vec<(AccountId, Vec<u8>)>,
        contract_storage: Vec<ContractStorageEntry>,
    ) -> Self {
        let mut state = StateStore::new();
        for (id, account_state) in accounts {
            state.insert(Account { id, state: account_state });
        }
        for (id, code) in contract_code {
            state.set_contract_code(id, code);
        }
        for ((contract, key), value) in contract_storage {
            state.merge_contract_storage(contract, key, value);
        }
        state
    }

    pub fn qa_pending(&self, subject: &Hash) -> Option<&QaPendingRecord> {
        self.qa_pending.get(subject)
    }

    pub fn qa_pending_mut(&mut self, subject: &Hash) -> Option<&mut QaPendingRecord> {
        self.qa_pending.get_mut(subject)
    }

    pub fn insert_qa_pending(&mut self, subject: Hash, record: QaPendingRecord) {
        self.qa_pending.insert(subject, record);
    }

    pub fn remove_qa_pending(&mut self, subject: &Hash) -> Option<QaPendingRecord> {
        self.qa_pending.remove(subject)
    }

    pub fn export_qa_pending(&self) -> Vec<(Hash, QaPendingRecord)> {
        self.qa_pending.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    pub fn load_qa_pending(&mut self, items: Vec<(Hash, QaPendingRecord)>) {
        self.qa_pending = items.into_iter().collect();
    }

    /// Top N accounts by stake (for validator set derivation).
    pub fn top_stakers(&self, n: usize) -> Vec<AccountId> {
        let mut accounts: Vec<_> = self.accounts.iter().collect();
        accounts.sort_by(|a, b| b.1.stake.cmp(&a.1.stake));
        accounts.into_iter().take(n).map(|(id, _)| *id).collect()
    }

    /// Sum balances and stakes over all accounts (O(n); intended for node cache refresh after commits).
    pub fn compute_native_aggregates(&self) -> ChainNativeAggregates {
        let mut total_balance: u128 = 0;
        let mut total_stake: u128 = 0;
        let mut account_count: u64 = 0;
        for st in self.accounts.values() {
            account_count = account_count.saturating_add(1);
            total_balance = total_balance.saturating_add(st.balance);
            total_stake = total_stake.saturating_add(st.stake);
        }
        let total_native_held = total_balance.saturating_add(total_stake);
        ChainNativeAggregates {
            account_count,
            total_balance,
            total_stake,
            total_native_held,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_revert() {
        let mut state = StateStore::new();
        let a = AccountId([1u8; 32]);
        let b = AccountId([2u8; 32]);
        state.insert(Account { id: a, state: AccountState { balance: 100, nonce: 0, stake: 0, ..Default::default() } });
        state.insert(Account { id: b, state: AccountState { balance: 50, nonce: 0, stake: 0, ..Default::default() } });
        let cp = state.checkpoint();
        state.get_mut(&a).unwrap().balance = 90;
        state.insert(Account { id: AccountId([3u8; 32]), state: AccountState { balance: 10, nonce: 0, stake: 0, ..Default::default() } });
        assert_eq!(state.get(&a).unwrap().balance, 90);
        state.revert(cp);
        assert_eq!(state.get(&a).unwrap().balance, 100);
        assert!(state.get(&AccountId([3u8; 32])).is_none());
    }

    #[test]
    fn test_top_stakers() {
        let mut state = StateStore::new();
        let a = AccountId([1u8; 32]);
        let b = AccountId([2u8; 32]);
        let c = AccountId([3u8; 32]);
        state.insert(Account { id: a, state: AccountState { balance: 0, nonce: 0, stake: 100, ..Default::default() } });
        state.insert(Account { id: b, state: AccountState { balance: 0, nonce: 0, stake: 500, ..Default::default() } });
        state.insert(Account { id: c, state: AccountState { balance: 0, nonce: 0, stake: 200, ..Default::default() } });
        let top = state.top_stakers(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0], b);
        assert_eq!(top[1], c);
    }

    #[test]
    fn test_get_contract_storage() {
        let mut state = StateStore::new();
        let c = AccountId([9u8; 32]);
        let k = [7u8; 32];
        let v = [8u8; 32];
        assert_eq!(state.get_contract_storage(&c, &k), [0u8; 32]);
        state.merge_contract_storage(c, k, v);
        assert_eq!(state.get_contract_storage(&c, &k), v);
    }

    #[test]
    fn state_root_includes_code_and_storage() {
        let mut state = StateStore::new();
        let c = AccountId([9u8; 32]);
        state.insert(Account {
            id: c,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 0, ..Default::default() },
        });
        let root_empty = state.state_root();

        state.set_contract_code(c, vec![0x60, 0x00]);
        let root_with_code = state.state_root();
        assert_ne!(root_empty, root_with_code);

        state.merge_contract_storage(c, [1u8; 32], [2u8; 32]);
        let root_with_storage = state.state_root();
        assert_ne!(root_with_code, root_with_storage);

        // Account proof verifies against the inclusive root.
        let proof = state.prove_account(&c).unwrap();
        assert!(proof.verify());
        assert_eq!(proof.root, root_with_storage);

        let slot_proof = state.prove_storage(&c, &[1u8; 32]).unwrap();
        assert!(slot_proof.verify());
    }

    #[test]
    fn test_compute_native_aggregates() {
        let mut state = StateStore::new();
        let a = AccountId([1u8; 32]);
        let b = AccountId([2u8; 32]);
        state.insert(Account {
            id: a,
            state: AccountState {
                balance: 100,
                nonce: 0,
                stake: 50, ..Default::default() },
        });
        state.insert(Account {
            id: b,
            state: AccountState {
                balance: 25,
                nonce: 0,
                stake: 0, ..Default::default() },
        });
        let agg = state.compute_native_aggregates();
        assert_eq!(agg.account_count, 2);
        assert_eq!(agg.total_balance, 125);
        assert_eq!(agg.total_stake, 50);
        assert_eq!(agg.total_native_held, 175);
    }
}
