//! State store — Sparse Merkle tree for compact proofs.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use boing_primitives::{Account, AccountId, AccountState, Hash};

use crate::names::{NameRegistry, NameRegistryParams};
use crate::open_validators::{OpenValidatorParams, OpenValidatorRegistry};
use crate::qa_pending::QaPendingRecord;
use crate::sample_offers::SampleOfferRegistry;
use crate::sparse_merkle::{hash_account_leaf, hash_contract_code, SparseMerkleTree};

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
///
/// Journaled (security review #4): a checkpoint is a position in the store's undo journal, so taking
/// one is O(1) and reverting costs O(writes since the checkpoint) instead of cloning the whole state
/// and rebuilding the SMT. Dropping a checkpoint without reverting keeps the writes (same as before).
/// While no checkpoint is alive, writes are not journaled and the journal is cleared.
#[derive(Clone)]
pub struct StateCheckpoint {
    pos: usize,
    /// Ties the handle to its store and keeps journaling on while the handle lives.
    token: Arc<()>,
}

/// One undo record: the value before a write (`None` = absent).
#[derive(Debug)]
enum Undo {
    Account(AccountId, Option<AccountState>),
    Code(AccountId, Option<Vec<u8>>),
    Storage((AccountId, [u8; 32]), Option<[u8; 32]>),
    QaPending(Hash, Option<QaPendingRecord>),
    /// Whole-registry snapshot, taken at most once per checkpoint level (registries are small and
    /// capped / paid; recorded only when a tx actually touches them).
    OpenValidators(Box<OpenValidatorRegistry>),
    Names(Box<NameRegistry>),
    SampleOffers(Box<SampleOfferRegistry>),
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
    /// Open validator registry (committed once non-empty).
    open_validators: OpenValidatorRegistry,
    /// Node config for open validator mode (not committed; `None` = mode off).
    open_validator_params: Option<OpenValidatorParams>,
    /// Name registry (committed once non-empty).
    names: NameRegistry,
    /// Node config for the name registry (not committed; `None` = registry off).
    name_registry_params: Option<NameRegistryParams>,
    /// Media-blob sample-offer registry (committed once non-empty). Phase 9.
    sample_offers: SampleOfferRegistry,
    /// Undo journal for live checkpoints (see [`StateCheckpoint`]).
    journal: Vec<Undo>,
    /// Shared with every live checkpoint; `strong_count > 1` means journaling is on.
    cp_token: Arc<()>,
    /// Journal position of the most recently created live checkpoint (upper bound of live ones).
    last_cp_pos: usize,
    /// Journal index of the registry snapshot taken since `last_cp_pos`, if any.
    ov_snap_pos: Option<usize>,
    names_snap_pos: Option<usize>,
    sample_offers_snap_pos: Option<usize>,
}

impl StateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: &AccountId) -> Option<&AccountState> {
        self.accounts.get(id)
    }

    /// True when a checkpoint is alive (writes must be journaled). With none alive the journal is
    /// dropped so it never grows beyond the writes of the outermost live checkpoint.
    fn journaling(&mut self) -> bool {
        if Arc::strong_count(&self.cp_token) > 1 {
            return true;
        }
        if !self.journal.is_empty() {
            self.journal = Vec::new();
        }
        self.last_cp_pos = 0;
        self.ov_snap_pos = None;
        self.names_snap_pos = None;
        self.sample_offers_snap_pos = None;
        false
    }

    fn record_account(&mut self, id: &AccountId) {
        if self.journaling() {
            let prev = self.accounts.get(id).cloned();
            self.journal.push(Undo::Account(*id, prev));
        }
    }

    fn record_storage(&mut self, contract: AccountId, key: [u8; 32]) {
        if self.journaling() {
            let prev = self.contract_storage.get(&(contract, key)).copied();
            self.journal.push(Undo::Storage((contract, key), prev));
        }
    }

    fn record_qa(&mut self, subject: &Hash) {
        if self.journaling() {
            let prev = self.qa_pending.get(subject).cloned();
            self.journal.push(Undo::QaPending(*subject, prev));
        }
    }

    /// A snapshot at index `>= last_cp_pos` already restores the registry for every live checkpoint.
    fn snapshot_needed(&self, snap: Option<usize>) -> bool {
        !matches!(snap, Some(p) if p >= self.last_cp_pos && p < self.journal.len())
    }

    /// Number of undo records currently held (diagnostics / tests).
    pub fn journal_len(&self) -> usize {
        self.journal.len()
    }

    pub fn get_mut(&mut self, id: &AccountId) -> Option<&mut AccountState> {
        if self.accounts.contains_key(id) {
            self.record_account(id);
        }
        self.accounts.get_mut(id)
    }

    pub fn insert(&mut self, account: Account) {
        self.record_account(&account.id);
        self.accounts.insert(account.id, account.state);
        // Tree is rebuilt on `state_root()` so code/storage commitments stay consistent.
    }

    pub fn set_contract_code(&mut self, account: AccountId, bytecode: Vec<u8>) {
        if self.journaling() {
            let prev = self.contract_code.get(&account).cloned();
            self.journal.push(Undo::Code(account, prev));
        }
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
        let mut ids: std::collections::HashSet<AccountId> = self.accounts.keys().copied().collect();
        for id in self.contract_code.keys() {
            ids.insert(*id);
        }
        // Group storage by contract in one pass (was a full storage scan per account: O(accounts x slots)).
        let mut slots_by_contract: BTreeMap<AccountId, Vec<([u8; 32], [u8; 32])>> = BTreeMap::new();
        for ((c, key), value) in &self.contract_storage {
            ids.insert(*c);
            slots_by_contract.entry(*c).or_default().push((*key, *value));
        }
        for id in ids {
            let state = self.accounts.get(&id).cloned().unwrap_or(AccountState {
                balance: 0,
                nonce: 0,
                stake: 0,
                ..Default::default()
            });
            let code_hash = self.code_hash_of(&id);
            let storage_root = match slots_by_contract.get(&id) {
                Some(slots) => {
                    let mut tree = SparseMerkleTree::new();
                    for (key, value) in slots {
                        tree.insert_storage_slot(*key, value);
                    }
                    tree.root()
                }
                None => Hash::ZERO,
            };
            let leaf = hash_account_leaf(&state, &code_hash, &storage_root);
            self.tree.insert_leaf_hash(id.0, leaf);
        }
        for (subject, rec) in &self.qa_pending {
            self.tree
                .insert_leaf_hash(QaPendingRecord::smt_key(subject), rec.commitment_hash());
        }
        if !self.open_validators.is_empty() {
            self.tree.insert_leaf_hash(
                OpenValidatorRegistry::smt_key(),
                Hash(self.open_validators.commitment()),
            );
        }
        if !self.names.is_empty() {
            self.tree
                .insert_leaf_hash(NameRegistry::smt_key(), Hash(self.names.commitment()));
        }
        if !self.sample_offers.is_empty() {
            self.tree.insert_leaf_hash(
                SampleOfferRegistry::smt_key(),
                Hash(self.sample_offers.commitment()),
            );
        }
        self.tree.root()
    }

    /// Merge account state from parallel execution view.
    pub fn merge_account(&mut self, id: AccountId, state: AccountState) {
        self.record_account(&id);
        self.accounts.insert(id, state);
    }

    /// Merge contract storage from parallel execution view (also the VM `SSTORE` path).
    pub fn merge_contract_storage(&mut self, contract: AccountId, key: [u8; 32], value: [u8; 32]) {
        self.record_storage(contract, key);
        self.contract_storage.insert((contract, key), value);
    }

    /// Snapshot current state for simulation (includes contract code and storage).
    pub fn snapshot(&self) -> StateStore {
        let mut out = StateStore::new();
        for (id, st) in &self.accounts {
            out.insert(boing_primitives::Account {
                id: *id,
                state: st.clone(),
            });
        }
        for (id, code) in &self.contract_code {
            out.set_contract_code(*id, code.clone());
        }
        for ((contract, key), value) in &self.contract_storage {
            out.merge_contract_storage(*contract, *key, *value);
        }
        out.qa_pending = self.qa_pending.clone();
        out.open_validators = self.open_validators.clone();
        out.open_validator_params = self.open_validator_params.clone();
        out.names = self.names.clone();
        out.name_registry_params = self.name_registry_params.clone();
        out.sample_offers = self.sample_offers.clone();
        out
    }

    /// Create a checkpoint for revert. O(1): it marks the current journal position.
    pub fn checkpoint(&mut self) -> StateCheckpoint {
        // Drops a stale journal first when no other checkpoint is alive.
        let _ = self.journaling();
        let pos = self.journal.len();
        self.last_cp_pos = pos;
        StateCheckpoint {
            pos,
            token: self.cp_token.clone(),
        }
    }

    /// Revert state to checkpoint (e.g. after a failed tx or block). Undoes the journaled writes
    /// made since the checkpoint, newest first: O(writes), no full-state copy, no SMT rebuild (the
    /// tree is rebuilt from current state by `state_root()` when needed).
    pub fn revert(&mut self, cp: StateCheckpoint) {
        assert!(
            Arc::ptr_eq(&cp.token, &self.cp_token),
            "StateCheckpoint used on a different StateStore"
        );
        boing_telemetry::component_debug(
            "boing_state::store",
            "state",
            "reverted_to_checkpoint",
            "state store reverted to checkpoint",
        );
        while self.journal.len() > cp.pos {
            match self.journal.pop().expect("len > pos") {
                Undo::Account(id, Some(st)) => {
                    self.accounts.insert(id, st);
                }
                Undo::Account(id, None) => {
                    self.accounts.remove(&id);
                }
                Undo::Code(id, Some(code)) => {
                    self.contract_code.insert(id, code);
                }
                Undo::Code(id, None) => {
                    self.contract_code.remove(&id);
                }
                Undo::Storage(k, Some(v)) => {
                    self.contract_storage.insert(k, v);
                }
                Undo::Storage(k, None) => {
                    self.contract_storage.remove(&k);
                }
                Undo::QaPending(k, Some(r)) => {
                    self.qa_pending.insert(k, r);
                }
                Undo::QaPending(k, None) => {
                    self.qa_pending.remove(&k);
                }
                Undo::OpenValidators(reg) => self.open_validators = *reg,
                Undo::Names(reg) => self.names = *reg,
                Undo::SampleOffers(reg) => self.sample_offers = *reg,
            }
        }
        // Checkpoints created after `cp` are dead (nested scopes); keep the live-position bound.
        self.last_cp_pos = self.last_cp_pos.min(cp.pos);
        if matches!(self.ov_snap_pos, Some(p) if p >= cp.pos) {
            self.ov_snap_pos = None;
        }
        if matches!(self.names_snap_pos, Some(p) if p >= cp.pos) {
            self.names_snap_pos = None;
        }
        if matches!(self.sample_offers_snap_pos, Some(p) if p >= cp.pos) {
            self.sample_offers_snap_pos = None;
        }
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
        let contract_code: Vec<_> = self
            .contract_code
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let contract_storage: Vec<_> = self
            .contract_storage
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
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
            state.insert(Account {
                id,
                state: account_state,
            });
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
        if self.qa_pending.contains_key(subject) {
            self.record_qa(subject);
        }
        self.qa_pending.get_mut(subject)
    }

    pub fn insert_qa_pending(&mut self, subject: Hash, record: QaPendingRecord) {
        self.record_qa(&subject);
        self.qa_pending.insert(subject, record);
    }

    pub fn remove_qa_pending(&mut self, subject: &Hash) -> Option<QaPendingRecord> {
        if self.qa_pending.contains_key(subject) {
            self.record_qa(subject);
        }
        self.qa_pending.remove(subject)
    }

    pub fn export_qa_pending(&self) -> Vec<(Hash, QaPendingRecord)> {
        self.qa_pending
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }

    pub fn load_qa_pending(&mut self, items: Vec<(Hash, QaPendingRecord)>) {
        if self.journaling() {
            let subjects: Vec<Hash> = self.qa_pending.keys().chain(items.iter().map(|(k, _)| k)).copied().collect();
            for s in subjects {
                self.record_qa(&s);
            }
        }
        self.qa_pending = items.into_iter().collect();
    }

    /// Open validator registry (read).
    /// Native-ledger redenomination (whole BOING -> base units): multiplies every account's balance,
    /// stake and pending unbond, and every open-validator bond held in the registry, by `factor`
    /// (saturating). Contract storage holds no native BOING (the VM has no native-value opcodes) and the
    /// name registry stores no amounts, so nothing else carries native units. Journaled like any other
    /// write. Returns the number of accounts touched. Callers must apply it exactly once per chain (see
    /// `BlockExecutor::prepare_state_for_height`).
    pub fn redenominate(&mut self, factor: u128) -> usize {
        let mut ids: Vec<AccountId> = self.accounts.keys().copied().collect();
        ids.sort();
        for id in &ids {
            if let Some(a) = self.get_mut(id) {
                a.balance = a.balance.saturating_mul(factor);
                a.stake = a.stake.saturating_mul(factor);
                a.pending_unbond = a.pending_unbond.saturating_mul(factor);
            }
        }
        if self.open_validators.records.values().any(|r| r.bond > 0) {
            for rec in self.open_validators_mut().records.values_mut() {
                rec.bond = rec.bond.saturating_mul(factor);
            }
        }
        ids.len()
    }

    pub fn open_validators(&self) -> &OpenValidatorRegistry {
        &self.open_validators
    }

    pub fn open_validators_mut(&mut self) -> &mut OpenValidatorRegistry {
        if self.journaling() && self.snapshot_needed(self.ov_snap_pos) {
            self.ov_snap_pos = Some(self.journal.len());
            self.journal
                .push(Undo::OpenValidators(Box::new(self.open_validators.clone())));
        }
        &mut self.open_validators
    }

    /// Replace the registry (persistence load).
    pub fn load_open_validators(&mut self, reg: OpenValidatorRegistry) {
        *self.open_validators_mut() = reg;
    }

    /// Name registry (read).
    pub fn names(&self) -> &NameRegistry {
        &self.names
    }

    pub fn names_mut(&mut self) -> &mut NameRegistry {
        if self.journaling() && self.snapshot_needed(self.names_snap_pos) {
            self.names_snap_pos = Some(self.journal.len());
            self.journal.push(Undo::Names(Box::new(self.names.clone())));
        }
        &mut self.names
    }

    /// Replace the registry (persistence load).
    pub fn load_names(&mut self, reg: NameRegistry) {
        *self.names_mut() = reg;
    }

    /// Name registry node config (`None` = registry off).
    pub fn name_registry_params(&self) -> Option<&NameRegistryParams> {
        self.name_registry_params.as_ref()
    }

    pub fn set_name_registry_params(&mut self, params: Option<NameRegistryParams>) {
        self.name_registry_params = params;
    }

    /// Media-blob sample-offer registry (read).
    pub fn sample_offers(&self) -> &SampleOfferRegistry {
        &self.sample_offers
    }

    pub fn sample_offers_mut(&mut self) -> &mut SampleOfferRegistry {
        if self.journaling() && self.snapshot_needed(self.sample_offers_snap_pos) {
            self.sample_offers_snap_pos = Some(self.journal.len());
            self.journal
                .push(Undo::SampleOffers(Box::new(self.sample_offers.clone())));
        }
        &mut self.sample_offers
    }

    /// Replace the sample-offer registry (persistence load).
    pub fn load_sample_offers(&mut self, reg: SampleOfferRegistry) {
        *self.sample_offers_mut() = reg;
    }

    /// Open-mode node config (`None` = open mode off).
    pub fn open_validator_params(&self) -> Option<&OpenValidatorParams> {
        self.open_validator_params.as_ref()
    }

    pub fn set_open_validator_params(&mut self, params: Option<OpenValidatorParams>) {
        self.open_validator_params = params;
    }

    /// Deterministic end-of-block hook for open validator mode (no-op when off). Producers and
    /// importers call this after execution + block reward and before computing the state root.
    pub fn apply_open_validator_block_hook(&mut self, height: u64, proposer: AccountId) {
        if let Some(p) = self.open_validator_params.clone() {
            self.open_validators_mut().on_block(&p, height, proposer);
        }
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
        state.insert(Account {
            id: a,
            state: AccountState {
                balance: 100,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: b,
            state: AccountState {
                balance: 50,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let cp = state.checkpoint();
        state.get_mut(&a).unwrap().balance = 90;
        state.insert(Account {
            id: AccountId([3u8; 32]),
            state: AccountState {
                balance: 10,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        assert_eq!(state.get(&a).unwrap().balance, 90);
        state.revert(cp);
        assert_eq!(state.get(&a).unwrap().balance, 100);
        assert!(state.get(&AccountId([3u8; 32])).is_none());
    }

    fn acct(b: u8, balance: u128) -> Account {
        Account {
            id: AccountId([b; 32]),
            state: AccountState {
                balance,
                ..Default::default()
            },
        }
    }

    /// Security review #4: journaled checkpoints restore every state component exactly (same root
    /// as never having written), nest correctly, and leave no journal once no checkpoint is alive.
    #[test]
    fn journaled_revert_restores_everything_and_nests() {
        let mut state = StateStore::new();
        state.insert(acct(1, 100));
        state.set_contract_code(AccountId([9; 32]), vec![1, 2, 3]);
        state.merge_contract_storage(AccountId([9; 32]), [1; 32], [2; 32]);
        state.insert_qa_pending(
            Hash([5; 32]),
            QaPendingRecord {
                deployer: AccountId([1; 32]),
                deploy_nonce: 0,
                tx: boing_primitives::Transaction {
                    nonce: 0,
                    sender: AccountId([1; 32]),
                    payload: boing_primitives::TransactionPayload::Transfer {
                        to: AccountId([2; 32]),
                        amount: 1,
                    },
                    access_list: boing_primitives::AccessList {
                        read: vec![],
                        write: vec![],
                    },
                },
                votes: Default::default(),
                rewarded: Default::default(),
                entered_height: 1,
            },
        );
        let params = NameRegistryParams::default();
        state
            .names_mut()
            .commit(&params, 1, AccountId([1; 32]), [7; 32])
            .unwrap();
        let root0 = state.state_root();
        assert_eq!(state.journal_len(), 0, "no journal without live checkpoints");

        let outer = state.checkpoint();
        state.get_mut(&AccountId([1; 32])).unwrap().balance = 1;
        state.insert(acct(2, 5));
        let inner = state.checkpoint();
        state.set_contract_code(AccountId([9; 32]), vec![9]);
        state.set_contract_code(AccountId([8; 32]), vec![8]);
        state.merge_contract_storage(AccountId([9; 32]), [1; 32], [3; 32]);
        state.merge_contract_storage(AccountId([9; 32]), [4; 32], [4; 32]);
        state.remove_qa_pending(&Hash([5; 32]));
        state
            .names_mut()
            .commit(&params, 2, AccountId([2; 32]), [8; 32])
            .unwrap();
        state
            .names_mut()
            .commit(&params, 2, AccountId([2; 32]), [9; 32])
            .unwrap();
        state.revert(inner);
        // Inner writes undone, outer writes kept.
        assert_eq!(state.get(&AccountId([1; 32])).unwrap().balance, 1);
        assert_eq!(state.get(&AccountId([2; 32])).unwrap().balance, 5);
        assert_eq!(state.get_contract_code(&AccountId([9; 32])), Some(&vec![1, 2, 3]));
        assert!(state.get_contract_code(&AccountId([8; 32])).is_none());
        assert_eq!(state.get_contract_storage(&AccountId([9; 32]), &[1; 32]), [2; 32]);
        assert_eq!(state.get_contract_storage(&AccountId([9; 32]), &[4; 32]), [0; 32]);
        assert!(state.qa_pending(&Hash([5; 32])).is_some());

        state.revert(outer);
        assert_eq!(state.state_root(), root0);
        assert!(state.get(&AccountId([2; 32])).is_none());
        assert_eq!(state.journal_len(), 0);
    }

    #[test]
    fn dropped_checkpoint_keeps_writes_and_clears_journal() {
        let mut state = StateStore::new();
        state.insert(acct(1, 100));
        {
            let _cp = state.checkpoint();
            state.get_mut(&AccountId([1; 32])).unwrap().balance = 7;
            assert!(state.journal_len() > 0);
        }
        // Next write with no live checkpoint drops the journal.
        state.insert(acct(3, 1));
        assert_eq!(state.journal_len(), 0);
        assert_eq!(state.get(&AccountId([1; 32])).unwrap().balance, 7);
        // A checkpoint taken later only undoes writes made after it.
        let cp = state.checkpoint();
        state.insert(acct(4, 1));
        state.revert(cp);
        assert_eq!(state.get(&AccountId([1; 32])).unwrap().balance, 7);
        assert!(state.get(&AccountId([4; 32])).is_none());
    }

    /// Registry snapshots are taken at most once per checkpoint level, not once per mutation.
    #[test]
    fn registry_snapshot_once_per_checkpoint_level() {
        let mut state = StateStore::new();
        let params = NameRegistryParams::default();
        let cp = state.checkpoint();
        for i in 0..50u8 {
            state
                .names_mut()
                .commit(&params, 1, AccountId([i; 32]), [i; 32])
                .unwrap();
        }
        assert_eq!(state.journal_len(), 1);
        let inner = state.checkpoint();
        state.names_mut().commit(&params, 1, AccountId([99; 32]), [99; 32]).unwrap();
        state.names_mut().commit(&params, 1, AccountId([98; 32]), [98; 32]).unwrap();
        assert_eq!(state.journal_len(), 2);
        state.revert(inner);
        let after_inner = state.names().commitment();
        state.revert(cp);
        assert!(state.names().is_empty());
        assert_ne!(after_inner, state.names().commitment());
    }

    /// Randomized: journaled revert equals a full-copy reference model.
    #[test]
    fn journaled_revert_matches_full_copy_model() {
        let mut seed: u64 = 0x9e3779b97f4a7c15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _round in 0..40 {
            let mut state = StateStore::new();
            for i in 0..8u8 {
                state.insert(acct(i, 1000));
                state.merge_contract_storage(AccountId([100; 32]), [i; 32], [i; 32]);
            }
            let before = state.state_root();
            let before_accounts = state.export_for_persistence();
            let mut cps = Vec::new();
            for _ in 0..30 {
                let r = next();
                let b = (r % 12) as u8;
                match (r >> 8) % 6 {
                    0 => state.insert(acct(b, (r >> 16) as u128 % 500)),
                    1 => {
                        if let Some(a) = state.get_mut(&AccountId([b; 32])) {
                            a.nonce += 1;
                        }
                    }
                    2 => state.merge_contract_storage(AccountId([100; 32]), [b; 32], [r as u8; 32]),
                    3 => state.set_contract_code(AccountId([b; 32]), vec![r as u8]),
                    4 => cps.push(state.checkpoint()),
                    _ => {
                        if cps.len() > 1 {
                            let cp = cps.pop().unwrap();
                            state.revert(cp);
                        }
                    }
                }
            }
            // Revert everything back to a checkpoint taken first.
            let mut state2 = state;
            let all: Vec<_> = cps.drain(..).collect();
            drop(all);
            let cp0 = state2.checkpoint();
            state2.insert(acct(200, 1));
            state2.revert(cp0);
            assert!(state2.get(&AccountId([200; 32])).is_none());
            // Separate full check: fresh store, checkpoint first, random writes, revert -> identical.
            let mut s3 = StateStore::load_from_persistence(
                before_accounts.0.clone(),
                before_accounts.1.clone(),
                before_accounts.2.clone(),
            );
            let cp = s3.checkpoint();
            for _ in 0..40 {
                let r = next();
                let b = (r % 12) as u8;
                match (r >> 8) % 5 {
                    0 => s3.insert(acct(b, (r >> 16) as u128 % 500)),
                    1 => {
                        if let Some(a) = s3.get_mut(&AccountId([b; 32])) {
                            a.balance = 0;
                        }
                    }
                    2 => s3.merge_contract_storage(AccountId([100; 32]), [b; 32], [r as u8; 32]),
                    3 => s3.set_contract_code(AccountId([b; 32]), vec![r as u8]),
                    _ => {
                        let inner = s3.checkpoint();
                        s3.insert(acct(b, 1));
                        if r & 1 == 0 {
                            s3.revert(inner);
                        }
                    }
                }
            }
            s3.revert(cp);
            assert_eq!(s3.state_root(), before);
        }
    }

    #[test]
    fn test_top_stakers() {
        let mut state = StateStore::new();
        let a = AccountId([1u8; 32]);
        let b = AccountId([2u8; 32]);
        let c = AccountId([3u8; 32]);
        state.insert(Account {
            id: a,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 100,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: b,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 500,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: c,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 200,
                ..Default::default()
            },
        });
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
                stake: 0,
                ..Default::default()
            },
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
                stake: 50,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: b,
            state: AccountState {
                balance: 25,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let agg = state.compute_native_aggregates();
        assert_eq!(agg.account_count, 2);
        assert_eq!(agg.total_balance, 125);
        assert_eq!(agg.total_stake, 50);
        assert_eq!(agg.total_native_held, 175);
    }

    #[test]
    fn redenominate_scales_every_native_amount_once_and_is_journaled() {
        use boing_primitives::{Account, AccountState};
        let mut st = StateStore::new();
        let a = AccountId([1; 32]);
        let b = AccountId([2; 32]);
        st.insert(Account { id: a, state: AccountState { balance: 20, stake: 3, pending_unbond: 4, nonce: 7, unbond_unlock_height: 99 } });
        st.insert(Account { id: b, state: AccountState { balance: 0, ..Default::default() } });
        let p = crate::OpenValidatorParams::default();
        st.open_validators_mut().register(&p, 0, b, AccountId([9; 32])).unwrap();
        let root_before = st.state_root();
        let cp = st.checkpoint();
        assert_eq!(st.redenominate(10), 2);
        let s = st.get(&a).unwrap();
        assert_eq!((s.balance, s.stake, s.pending_unbond, s.nonce, s.unbond_unlock_height), (200, 30, 40, 7, 99));
        assert_eq!(st.open_validators().records[&AccountId([9; 32])].bond, 1_000);
        assert_ne!(st.state_root(), root_before);
        st.revert(cp);
        assert_eq!(st.get(&a).unwrap().balance, 20, "journaled: revert undoes it");
        assert_eq!(st.open_validators().records[&AccountId([9; 32])].bond, 100);
        assert_eq!(st.state_root(), root_before);
    }
}
