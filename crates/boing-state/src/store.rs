//! State store — Sparse Merkle tree for compact proofs.

use std::collections::HashMap;

use boing_primitives::{Account, AccountId, AccountState, Hash};

use crate::sparse_merkle::SparseMerkleTree;

/// Type alias for persisted contract storage entries: ((contract, key), value).
pub type ContractStorageEntry = ((AccountId, [u8; 32]), [u8; 32]);

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
        self.tree.insert(account.id, &account.state);
        self.accounts.insert(account.id, account.state);
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

    /// Compute state root from Sparse Merkle tree. Rebuilds tree from current
    /// accounts to include changes made via get_mut (e.g. by the VM).
    pub fn state_root(&mut self) -> Hash {
        self.tree = SparseMerkleTree::new();
        for (id, state) in &self.accounts {
            self.tree.insert(*id, state);
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

    /// Snapshot current state for simulation (does not include contract storage).
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
        out
    }

    /// Create a checkpoint of current state for revert.
    pub fn checkpoint(&self) -> StateCheckpoint {
        StateCheckpoint {
            accounts: self.accounts.iter().map(|(k, v)| (*k, v.clone())).collect(),
            contract_code: self.contract_code.iter().map(|(k, v)| (*k, v.clone())).collect(),
            contract_storage: self.contract_storage.iter().map(|(k, v)| (*k, *v)).collect(),
        }
    }

    /// Revert state to checkpoint (e.g. after failed block execution).
    pub fn revert(&mut self, cp: StateCheckpoint) {
        self.accounts = cp.accounts;
        self.contract_code = cp.contract_code;
        self.contract_storage = cp.contract_storage;
        self.tree = SparseMerkleTree::new();
        for (id, st) in &self.accounts {
            self.tree.insert(*id, st);
        }
    }

    /// Generate Merkle proof for an account. Ensures tree is synced with accounts.
    pub fn prove_account(&mut self, id: &AccountId) -> Option<crate::MerkleProof> {
        self.state_root(); // sync tree with accounts
        self.tree.prove(id)
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

    /// Top N accounts by stake (for validator set derivation).
    pub fn top_stakers(&self, n: usize) -> Vec<AccountId> {
        let mut accounts: Vec<_> = self.accounts.iter().collect();
        accounts.sort_by(|a, b| b.1.stake.cmp(&a.1.stake));
        accounts.into_iter().take(n).map(|(id, _)| *id).collect()
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
        state.insert(Account { id: a, state: AccountState { balance: 100, nonce: 0, stake: 0 } });
        state.insert(Account { id: b, state: AccountState { balance: 50, nonce: 0, stake: 0 } });
        let cp = state.checkpoint();
        state.get_mut(&a).unwrap().balance = 90;
        state.insert(Account { id: AccountId([3u8; 32]), state: AccountState { balance: 10, nonce: 0, stake: 0 } });
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
        state.insert(Account { id: a, state: AccountState { balance: 0, nonce: 0, stake: 100 } });
        state.insert(Account { id: b, state: AccountState { balance: 0, nonce: 0, stake: 500 } });
        state.insert(Account { id: c, state: AccountState { balance: 0, nonce: 0, stake: 200 } });
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
}
