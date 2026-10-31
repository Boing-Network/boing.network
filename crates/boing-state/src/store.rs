//! State store — Sparse Merkle tree for compact proofs.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use boing_primitives::{Account, AccountId, AccountState, Hash};

use crate::names::{NameRegistry, NameRegistryParams};
use crate::open_validators::{OpenValidatorParams, OpenValidatorRegistry};
use crate::qa_pending::QaPendingRecord;
use crate::sample_offers::SampleOfferRegistry;
use crate::sample_serve::SampleServeChallengeRegistry;
use crate::slash_appeals::SlashAppealRegistry;
use crate::sparse_merkle::{
    hash_account_leaf, hash_contract_code, SparseMerkleTree, DEEP_MAX_DEPTH, LEGACY_MAX_DEPTH,
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

/// Full state snapshot for persistence helpers (audit #61 item 32).
/// Prefer this over [`PersistenceExport`] alone so registries are not dropped on round-trip.
#[derive(Clone, Debug, Default)]
pub struct PersistenceBundle {
    pub accounts: Vec<(AccountId, AccountState)>,
    pub contract_code: Vec<(AccountId, Vec<u8>)>,
    pub contract_storage: Vec<ContractStorageEntry>,
    pub qa_pending: Vec<(Hash, QaPendingRecord)>,
    pub open_validators: OpenValidatorRegistry,
    pub names: NameRegistry,
}

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
    SampleServe(Box<SampleServeChallengeRegistry>),
    SlashAppeals(Box<SlashAppealRegistry>),
}

/// State store with Sparse Merkle tree for state_root.
#[derive(Default)]
pub struct StateStore {
    accounts: HashMap<AccountId, AccountState>,
    tree: SparseMerkleTree,
    /// Contract bytecode by account.
    contract_code: HashMap<AccountId, Vec<u8>>,
    /// Contract storage: (contract, key) -> value.
    contract_storage: HashMap<(AccountId, [u8; 32]), [u8; 32]>,
    /// Per-contract storage SMTs, kept in sync incrementally once `root_synced`.
    storage_trees: HashMap<AccountId, SparseMerkleTree>,
    /// Storage-tree v2 activation height (node config, not committed; `None` = never, the default).
    /// From this block height every per-contract storage tree routes on all 256 key bits instead of
    /// the first 64, so keys that share an 8-byte prefix are all committed. See
    /// `docs/STORAGE-TREE-V2-ACTIVATION.md`.
    storage_tree_v2_from: Option<u64>,
    /// True while the per-contract storage trees are built at full depth (derived from
    /// `storage_tree_v2_from` and the height passed to [`StateStore::state_root_at_height`]).
    storage_tree_deep: bool,
    /// Account-tree v3 activation height (node config, not committed; `None` = never, the default).
    /// From this block height the global account SMT (accounts, QA records, registries) routes on
    /// all 256 key bits instead of the first 64, so account ids that share an 8-byte prefix are all
    /// committed (audit #61 item 30). See `docs/ACCOUNT-TREE-V3-ACTIVATION.md`.
    account_tree_v3_from: Option<u64>,
    /// True while the account tree is built at full depth (derived like `storage_tree_deep`).
    account_tree_deep: bool,
    /// True once `tree`/`storage_trees` mirror the committed state up to the `dirty_*` sets below.
    /// Cleared by bulk replacements (e.g. `load_qa_pending`); the next `state_root` rebuilds fully.
    root_synced: bool,
    /// Accounts whose leaf must be recomputed at the next `state_root` (only tracked when synced).
    dirty_accounts: HashSet<AccountId>,
    /// Contract storage slots changed since the last `state_root`.
    dirty_slots: HashSet<(AccountId, [u8; 32])>,
    /// QA-pending subjects changed since the last `state_root`.
    dirty_qa: HashSet<Hash>,
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
    /// Sample-serve challenge registry (committed once non-empty). Phase 10.
    sample_serve: SampleServeChallengeRegistry,
    /// Consensus slash-appeal registry (committed once non-empty). Phase 14.
    slash_appeals: SlashAppealRegistry,
    /// Host-supplied consensus validator set for ResolveSlashAppeal (not committed).
    slash_appeal_resolvers: Option<Vec<AccountId>>,
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
    sample_serve_snap_pos: Option<usize>,
    slash_appeals_snap_pos: Option<usize>,
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
        self.sample_serve_snap_pos = None;
        self.slash_appeals_snap_pos = None;
        false
    }

    fn mark_account(&mut self, id: &AccountId) {
        if self.root_synced {
            self.dirty_accounts.insert(*id);
        }
    }

    fn mark_slot(&mut self, contract: AccountId, key: [u8; 32]) {
        if self.root_synced {
            self.dirty_slots.insert((contract, key));
            self.dirty_accounts.insert(contract);
        }
    }

    fn mark_qa(&mut self, subject: &Hash) {
        if self.root_synced {
            self.dirty_qa.insert(*subject);
        }
    }

    fn record_account(&mut self, id: &AccountId) {
        self.mark_account(id);
        if self.journaling() {
            let prev = self.accounts.get(id).cloned();
            self.journal.push(Undo::Account(*id, prev));
        }
    }

    fn record_storage(&mut self, contract: AccountId, key: [u8; 32]) {
        self.mark_slot(contract, key);
        if self.journaling() {
            let prev = self.contract_storage.get(&(contract, key)).copied();
            self.journal.push(Undo::Storage((contract, key), prev));
        }
    }

    fn record_qa(&mut self, subject: &Hash) {
        self.mark_qa(subject);
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
        // The leaf is refreshed by the next `state_root()` (dirty tracking in `record_account`).
    }

    pub fn set_contract_code(&mut self, account: AccountId, bytecode: Vec<u8>) {
        self.mark_account(&account);
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

    /// Routing depth of the per-contract storage trees under the current activation mode.
    fn storage_depth(&self) -> u32 {
        if self.storage_tree_deep {
            DEEP_MAX_DEPTH
        } else {
            LEGACY_MAX_DEPTH
        }
    }

    fn account_depth(&self) -> u32 {
        if self.account_tree_deep {
            DEEP_MAX_DEPTH
        } else {
            LEGACY_MAX_DEPTH
        }
    }

    /// Configure the account-tree v3 activation height (`None` = off). Consensus-critical: every
    /// node must use the same value, and it must be in the future when set on a live chain.
    pub fn set_account_tree_v3_activation(&mut self, from: Option<u64>) {
        self.account_tree_v3_from = from;
    }

    /// The configured account-tree v3 activation height, if any.
    pub fn account_tree_v3_activation(&self) -> Option<u64> {
        self.account_tree_v3_from
    }

    /// Whether the account tree is currently built at full (v3) depth.
    pub fn account_tree_v3_active(&self) -> bool {
        self.account_tree_deep
    }

    /// Configure the storage-tree v2 activation height (`None` = off). Consensus-critical: every
    /// node must use the same value, and it must be in the future when set on a live chain.
    pub fn set_storage_tree_v2_activation(&mut self, from: Option<u64>) {
        self.storage_tree_v2_from = from;
    }

    /// The configured storage-tree v2 activation height, if any.
    pub fn storage_tree_v2_activation(&self) -> Option<u64> {
        self.storage_tree_v2_from
    }

    /// Whether the storage trees are currently built at full (v2) depth.
    pub fn storage_tree_v2_active(&self) -> bool {
        self.storage_tree_deep
    }

    /// [`StateStore::state_root`] for the block at `height`: switches the storage trees to v2 (or
    /// back, on a rewind below the activation) when `height` crosses the configured activation.
    /// With no activation configured this is exactly `state_root()`.
    pub fn state_root_at_height(&mut self, height: u64) -> Hash {
        let want = self.storage_tree_v2_from.is_some_and(|from| height >= from);
        if want != self.storage_tree_deep {
            self.storage_tree_deep = want;
            self.root_synced = false; // full rebuild at the new depth (one-time, at activation)
        }
        let want = self.account_tree_v3_from.is_some_and(|from| height >= from);
        if want != self.account_tree_deep {
            self.account_tree_deep = want;
            self.root_synced = false;
        }
        self.state_root()
    }

    /// Per-contract storage SMT root (`Hash::ZERO` if the contract has no slots).
    pub fn storage_root_of(&self, contract: &AccountId) -> Hash {
        let mut tree = SparseMerkleTree::with_max_depth(self.storage_depth());
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

    fn account_leaf(&self, id: &AccountId, storage_root: &Hash) -> Hash {
        let state = self.accounts.get(id).cloned().unwrap_or(AccountState {
            balance: 0,
            nonce: 0,
            stake: 0,
            ..Default::default()
        });
        hash_account_leaf(&state, &self.code_hash_of(id), storage_root)
    }

    /// Commitments of the singleton registries (`None` = registry empty, so no leaf).
    fn registry_leaves(&self) -> [([u8; 32], Option<Hash>); 5] {
        [
            (
                OpenValidatorRegistry::smt_key(),
                (!self.open_validators.is_empty()).then(|| Hash(self.open_validators.commitment())),
            ),
            (
                NameRegistry::smt_key(),
                (!self.names.is_empty()).then(|| Hash(self.names.commitment())),
            ),
            (
                SampleOfferRegistry::smt_key(),
                (!self.sample_offers.is_empty()).then(|| Hash(self.sample_offers.commitment())),
            ),
            (
                SampleServeChallengeRegistry::smt_key(),
                (!self.sample_serve.is_empty()).then(|| Hash(self.sample_serve.commitment())),
            ),
            (
                SlashAppealRegistry::smt_key(),
                (!self.slash_appeals.is_empty()).then(|| Hash(self.slash_appeals.commitment())),
            ),
        ]
    }

    /// Builds the account SMT and per-contract storage SMTs from scratch.
    fn build_full(&self) -> (SparseMerkleTree, HashMap<AccountId, SparseMerkleTree>) {
        // One pass groups storage by contract (a per-account storage scan would be O(accounts x slots)).
        let depth = self.storage_depth();
        let mut storage_trees: HashMap<AccountId, SparseMerkleTree> = HashMap::new();
        for ((c, key), value) in &self.contract_storage {
            storage_trees
                .entry(*c)
                .or_insert_with(|| SparseMerkleTree::with_max_depth(depth))
                .insert_storage_slot(*key, value);
        }
        let mut ids: HashSet<AccountId> = self.accounts.keys().copied().collect();
        ids.extend(self.contract_code.keys().copied());
        ids.extend(storage_trees.keys().copied());
        let mut tree = SparseMerkleTree::with_max_depth(self.account_depth());
        for id in ids {
            let storage_root = storage_trees
                .get_mut(&id)
                .map(|t| t.root())
                .unwrap_or(Hash::ZERO);
            tree.insert_leaf_hash(id.0, self.account_leaf(&id, &storage_root));
        }
        for (subject, rec) in &self.qa_pending {
            tree.insert_leaf_hash(QaPendingRecord::smt_key(subject), rec.commitment_hash());
        }
        for (key, leaf) in self.registry_leaves() {
            if let Some(h) = leaf {
                tree.insert_leaf_hash(key, h);
            }
        }
        (tree, storage_trees)
    }

    /// State root computed from scratch with the original full-rebuild algorithm, without touching
    /// the incremental caches. Slow (O(state)); the oracle for tests and audit tooling.
    pub fn state_root_full_rebuild(&self) -> Hash {
        let (mut tree, mut storage_trees) = self.build_full();
        // Re-derive the account leaves with reference storage roots so the oracle shares no cached
        // hashing with the incremental path.
        for (id, st) in storage_trees.iter_mut() {
            let reference = st.reference_root();
            tree.insert_leaf_hash(id.0, self.account_leaf(id, &reference));
        }
        tree.reference_root()
    }

    /// Compute the state root from the Sparse Merkle tree. Each leaf commits to
    /// balance/nonce/stake/unbond **and** code_hash + storage_root; accounts that only have code or
    /// storage (no balance entry yet) are included.
    ///
    /// Incremental: after the first call only accounts / storage slots / QA records written since the
    /// previous call are re-hashed (O(changed x depth)); the root is identical to a full rebuild
    /// ([`StateStore::state_root_full_rebuild`]).
    pub fn state_root(&mut self) -> Hash {
        if !self.root_synced {
            let (tree, storage_trees) = self.build_full();
            self.tree = tree;
            self.storage_trees = storage_trees;
            self.root_synced = true;
            self.dirty_accounts.clear();
            self.dirty_slots.clear();
            self.dirty_qa.clear();
        } else {
            let depth = self.storage_depth();
            for (c, key) in std::mem::take(&mut self.dirty_slots) {
                match self.contract_storage.get(&(c, key)) {
                    Some(v) => self
                        .storage_trees
                        .entry(c)
                        .or_insert_with(|| SparseMerkleTree::with_max_depth(depth))
                        .insert_storage_slot(key, v),
                    None => {
                        if let Some(t) = self.storage_trees.get_mut(&c) {
                            t.delete_raw(&key);
                            if t.is_empty() {
                                self.storage_trees.remove(&c);
                            }
                        }
                    }
                }
            }
            for id in std::mem::take(&mut self.dirty_accounts) {
                let storage_root = self.storage_trees.get_mut(&id).map(|t| t.root());
                let exists = self.accounts.contains_key(&id)
                    || self.contract_code.contains_key(&id)
                    || storage_root.is_some();
                if exists {
                    let leaf = self.account_leaf(&id, &storage_root.unwrap_or(Hash::ZERO));
                    self.tree.insert_leaf_hash(id.0, leaf);
                } else {
                    self.tree.delete_raw(&id.0);
                }
            }
            for subject in std::mem::take(&mut self.dirty_qa) {
                let key = QaPendingRecord::smt_key(&subject);
                match self.qa_pending.get(&subject) {
                    Some(rec) => self.tree.insert_leaf_hash(key, rec.commitment_hash()),
                    None => self.tree.delete_raw(&key),
                }
            }
            // Registries are small and mutated through `*_mut()` borrows: refresh their leaves always.
            for (key, leaf) in self.registry_leaves() {
                match leaf {
                    Some(h) => self.tree.insert_leaf_hash(key, h),
                    None => self.tree.delete_raw(&key),
                }
            }
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

    /// Snapshot current state for simulation / block import (includes contract code and storage).
    ///
    /// A flat clone: the incremental SMT caches are carried over, so a snapshot of a store whose
    /// root was already computed produces its next `state_root` in O(changed x depth). The journal
    /// and live checkpoints are not shared with the snapshot.
    pub fn snapshot(&self) -> StateStore {
        StateStore {
            accounts: self.accounts.clone(),
            tree: self.tree.clone(),
            contract_code: self.contract_code.clone(),
            contract_storage: self.contract_storage.clone(),
            storage_trees: self.storage_trees.clone(),
            storage_tree_v2_from: self.storage_tree_v2_from,
            storage_tree_deep: self.storage_tree_deep,
            account_tree_v3_from: self.account_tree_v3_from,
            account_tree_deep: self.account_tree_deep,
            root_synced: self.root_synced,
            dirty_accounts: self.dirty_accounts.clone(),
            dirty_slots: self.dirty_slots.clone(),
            dirty_qa: self.dirty_qa.clone(),
            qa_pending: self.qa_pending.clone(),
            open_validators: self.open_validators.clone(),
            open_validator_params: self.open_validator_params.clone(),
            names: self.names.clone(),
            name_registry_params: self.name_registry_params.clone(),
            sample_offers: self.sample_offers.clone(),
            sample_serve: self.sample_serve.clone(),
            slash_appeals: self.slash_appeals.clone(),
            slash_appeal_resolvers: self.slash_appeal_resolvers.clone(),
            journal: Vec::new(),
            cp_token: Arc::new(()),
            last_cp_pos: 0,
            ov_snap_pos: None,
            names_snap_pos: None,
            sample_offers_snap_pos: None,
            sample_serve_snap_pos: None,
            slash_appeals_snap_pos: None,
        }
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
                    self.mark_account(&id);
                    self.accounts.insert(id, st);
                }
                Undo::Account(id, None) => {
                    self.mark_account(&id);
                    self.accounts.remove(&id);
                }
                Undo::Code(id, Some(code)) => {
                    self.mark_account(&id);
                    self.contract_code.insert(id, code);
                }
                Undo::Code(id, None) => {
                    self.mark_account(&id);
                    self.contract_code.remove(&id);
                }
                Undo::Storage(k, Some(v)) => {
                    self.mark_slot(k.0, k.1);
                    self.contract_storage.insert(k, v);
                }
                Undo::Storage(k, None) => {
                    self.mark_slot(k.0, k.1);
                    self.contract_storage.remove(&k);
                }
                Undo::QaPending(k, Some(r)) => {
                    self.mark_qa(&k);
                    self.qa_pending.insert(k, r);
                }
                Undo::QaPending(k, None) => {
                    self.mark_qa(&k);
                    self.qa_pending.remove(&k);
                }
                Undo::OpenValidators(reg) => self.open_validators = *reg,
                Undo::Names(reg) => self.names = *reg,
                Undo::SampleOffers(reg) => self.sample_offers = *reg,
                Undo::SampleServe(reg) => self.sample_serve = *reg,
                Undo::SlashAppeals(reg) => self.slash_appeals = *reg,
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
        if matches!(self.sample_serve_snap_pos, Some(p) if p >= cp.pos) {
            self.sample_serve_snap_pos = None;
        }
        if matches!(self.slash_appeals_snap_pos, Some(p) if p >= cp.pos) {
            self.slash_appeals_snap_pos = None;
        }
    }

    /// Generate Merkle proof for an account. Ensures tree is synced (includes code/storage).
    pub fn prove_account(&mut self, id: &AccountId) -> Option<crate::MerkleProof> {
        self.state_root(); // sync tree with accounts + code/storage commitments
        self.tree.prove(id)
    }

    /// Read-only account proof: builds an ephemeral tree so RPC can avoid taking the node write
    /// lock (audit #61 item 20).
    pub fn prove_account_readonly(&self, id: &AccountId) -> Option<crate::MerkleProof> {
        let (mut tree, _) = self.build_full();
        tree.prove(id)
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
        // Fast path: the incremental per-contract tree is current (no pending slot writes), so clone
        // just this contract's arena instead of re-inserting every slot of the contract.
        if self.root_synced && self.dirty_slots.is_empty() {
            if let Some(t) = self.storage_trees.get(contract) {
                let mut tree = t.clone();
                return tree.prove(&AccountId(*slot));
            }
        }
        let mut tree = SparseMerkleTree::with_max_depth(self.storage_depth());
        for ((c, key), value) in &self.contract_storage {
            if c == contract {
                tree.insert_storage_slot(*key, value);
            }
        }
        tree.prove(&AccountId(*slot))
    }

    /// Export accounts / code / storage for the primary `accounts.bin` snapshot.
    ///
    /// Does **not** include registries — use [`Self::export_persistence_bundle`] for a full
    /// round-trip (audit #61 item 32).
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

    /// Export accounts, code, storage, and singleton registries in one bundle.
    pub fn export_persistence_bundle(&self) -> PersistenceBundle {
        let (accounts, contract_code, contract_storage) = self.export_for_persistence();
        PersistenceBundle {
            accounts,
            contract_code,
            contract_storage,
            qa_pending: self.export_qa_pending(),
            open_validators: self.open_validators.clone(),
            names: self.names.clone(),
        }
    }

    /// Load state from persisted data (accounts / code / storage only).
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

    /// Load a full [`PersistenceBundle`], restoring registries that affect the state root.
    pub fn load_persistence_bundle(bundle: PersistenceBundle) -> Self {
        let mut state = Self::load_from_persistence(
            bundle.accounts,
            bundle.contract_code,
            bundle.contract_storage,
        );
        state.load_qa_pending(bundle.qa_pending);
        state.load_open_validators(bundle.open_validators);
        state.load_names(bundle.names);
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
            let subjects: Vec<Hash> = self
                .qa_pending
                .keys()
                .chain(items.iter().map(|(k, _)| k))
                .copied()
                .collect();
            for s in subjects {
                self.record_qa(&s);
            }
        }
        self.qa_pending = items.into_iter().collect();
        // Bulk replacement: let the next `state_root` rebuild from scratch.
        self.root_synced = false;
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

    /// Sample-serve challenge registry (read).
    pub fn sample_serve(&self) -> &SampleServeChallengeRegistry {
        &self.sample_serve
    }

    pub fn sample_serve_mut(&mut self) -> &mut SampleServeChallengeRegistry {
        if self.journaling() && self.snapshot_needed(self.sample_serve_snap_pos) {
            self.sample_serve_snap_pos = Some(self.journal.len());
            self.journal
                .push(Undo::SampleServe(Box::new(self.sample_serve.clone())));
        }
        &mut self.sample_serve
    }

    /// Replace the sample-serve registry (persistence load).
    pub fn load_sample_serve(&mut self, reg: SampleServeChallengeRegistry) {
        *self.sample_serve_mut() = reg;
    }

    /// Consensus slash-appeal registry (read). Phase 14.
    pub fn slash_appeals(&self) -> &SlashAppealRegistry {
        &self.slash_appeals
    }

    pub fn slash_appeals_mut(&mut self) -> &mut SlashAppealRegistry {
        if self.journaling() && self.snapshot_needed(self.slash_appeals_snap_pos) {
            self.slash_appeals_snap_pos = Some(self.journal.len());
            self.journal
                .push(Undo::SlashAppeals(Box::new(self.slash_appeals.clone())));
        }
        &mut self.slash_appeals
    }

    /// Replace the slash-appeal registry (persistence load).
    pub fn load_slash_appeals(&mut self, reg: SlashAppealRegistry) {
        *self.slash_appeals_mut() = reg;
    }

    /// Host-pinned consensus validators allowed to ResolveSlashAppeal (not committed).
    pub fn slash_appeal_resolvers(&self) -> Option<&[AccountId]> {
        self.slash_appeal_resolvers.as_deref()
    }

    pub fn set_slash_appeal_resolvers(&mut self, set: Option<Vec<AccountId>>) {
        self.slash_appeal_resolvers = set;
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
        // Total order (stake desc, then account id asc) so equal stakes resolve identically on every
        // node (HashMap iteration order is per-process random), and a partial selection so the cost
        // is O(accounts + n log n) instead of a full sort.
        let order = |a: &(&AccountId, &AccountState), b: &(&AccountId, &AccountState)| {
            b.1.stake.cmp(&a.1.stake).then_with(|| a.0 .0.cmp(&b.0 .0))
        };
        let mut accounts: Vec<_> = self.accounts.iter().collect();
        if n == 0 {
            return Vec::new();
        }
        if n < accounts.len() {
            accounts.select_nth_unstable_by(n - 1, order);
            accounts.truncate(n);
        }
        accounts.sort_unstable_by(order);
        accounts.into_iter().map(|(id, _)| *id).collect()
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
        assert_eq!(
            state.journal_len(),
            0,
            "no journal without live checkpoints"
        );

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
        assert_eq!(
            state.get_contract_code(&AccountId([9; 32])),
            Some(&vec![1, 2, 3])
        );
        assert!(state.get_contract_code(&AccountId([8; 32])).is_none());
        assert_eq!(
            state.get_contract_storage(&AccountId([9; 32]), &[1; 32]),
            [2; 32]
        );
        assert_eq!(
            state.get_contract_storage(&AccountId([9; 32]), &[4; 32]),
            [0; 32]
        );
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
        state
            .names_mut()
            .commit(&params, 1, AccountId([99; 32]), [99; 32])
            .unwrap();
        state
            .names_mut()
            .commit(&params, 1, AccountId([98; 32]), [98; 32])
            .unwrap();
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
            let all: Vec<_> = std::mem::take(&mut cps);
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
        st.insert(Account {
            id: a,
            state: AccountState {
                balance: 20,
                stake: 3,
                pending_unbond: 4,
                nonce: 7,
                unbond_unlock_height: 99,
            },
        });
        st.insert(Account {
            id: b,
            state: AccountState {
                balance: 0,
                ..Default::default()
            },
        });
        let p = crate::OpenValidatorParams::default();
        st.open_validators_mut()
            .register(&p, 0, b, AccountId([9; 32]))
            .unwrap();
        let root_before = st.state_root();
        let cp = st.checkpoint();
        assert_eq!(st.redenominate(10), 2);
        let s = st.get(&a).unwrap();
        assert_eq!(
            (
                s.balance,
                s.stake,
                s.pending_unbond,
                s.nonce,
                s.unbond_unlock_height
            ),
            (200, 30, 40, 7, 99)
        );
        assert_eq!(
            st.open_validators().records[&AccountId([9; 32])].bond,
            1_000
        );
        assert_ne!(st.state_root(), root_before);
        st.revert(cp);
        assert_eq!(
            st.get(&a).unwrap().balance,
            20,
            "journaled: revert undoes it"
        );
        assert_eq!(st.open_validators().records[&AccountId([9; 32])].bond, 100);
        assert_eq!(st.state_root(), root_before);
    }

    /// Differential: the incremental `state_root` (dirty tracking + persistent trees, carried through
    /// nested checkpoints / reverts / snapshots) must equal the from-scratch reference at every step.
    #[test]
    fn incremental_state_root_matches_full_rebuild_under_random_ops() {
        struct Rng(u64);
        impl Rng {
            fn next(&mut self) -> u64 {
                self.0 ^= self.0 >> 12;
                self.0 ^= self.0 << 25;
                self.0 ^= self.0 >> 27;
                self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
            }
        }
        let qa_rec = |n: u8| QaPendingRecord {
            deployer: AccountId([n; 32]),
            deploy_nonce: n as u64,
            tx: boing_primitives::Transaction {
                nonce: n as u64,
                sender: AccountId([n; 32]),
                payload: boing_primitives::TransactionPayload::Transfer {
                    to: AccountId([2; 32]),
                    amount: n as u128,
                },
                access_list: boing_primitives::AccessList {
                    read: vec![],
                    write: vec![],
                },
            },
            votes: Default::default(),
            rewarded: Default::default(),
            entered_height: n as u64,
        };
        for seed in 1..=25u64 {
            let mut r = Rng(0xA076_1D64_78BD_642F ^ seed.wrapping_mul(0xE703_7ED1_A0B4_28DB));
            let mut st = StateStore::new();
            let mut cps: Vec<StateCheckpoint> = Vec::new();
            for step in 0..300 {
                let id = AccountId([(r.next() % 8) as u8 + 1; 32]);
                match r.next() % 14 {
                    0 | 1 => st.insert(acct(id.0[0], (r.next() % 1000) as u128)),
                    2 => {
                        if let Some(a) = st.get_mut(&id) {
                            a.nonce += 1;
                            a.balance = a.balance.wrapping_add(7);
                        }
                    }
                    3 => st
                        .set_contract_code(id, vec![(r.next() % 4) as u8; (r.next() % 5) as usize]),
                    4..=6 => {
                        // small-integer style slots (shared 64-bit prefix) and random slots
                        let mut key = [0u8; 32];
                        if r.next().is_multiple_of(2) {
                            key[31] = (r.next() % 6) as u8;
                        } else {
                            key[..8].copy_from_slice(&r.next().to_le_bytes());
                        }
                        st.merge_contract_storage(id, key, [(r.next() % 3) as u8; 32]);
                    }
                    7 => st.insert_qa_pending(
                        Hash([(r.next() % 4) as u8; 32]),
                        qa_rec((r.next() % 5) as u8),
                    ),
                    8 => {
                        st.remove_qa_pending(&Hash([(r.next() % 4) as u8; 32]));
                    }
                    9 => {
                        let params = NameRegistryParams::default();
                        let _ =
                            st.names_mut()
                                .commit(&params, step, id, [(r.next() % 3) as u8; 32]);
                    }
                    10 => cps.push(st.checkpoint()),
                    11 => {
                        if let Some(cp) = cps.pop() {
                            st.revert(cp);
                        }
                    }
                    12 => {
                        // a snapshot must carry the incremental caches and then evolve independently
                        let mut snap = st.snapshot();
                        snap.insert(acct(200, 5));
                        assert_eq!(
                            snap.state_root(),
                            snap.state_root_full_rebuild(),
                            "seed {seed} step {step} snap"
                        );
                        assert_eq!(
                            st.state_root(),
                            st.state_root_full_rebuild(),
                            "seed {seed} step {step} orig"
                        );
                    }
                    _ => {
                        assert_eq!(
                            st.state_root(),
                            st.state_root_full_rebuild(),
                            "seed {seed} step {step}"
                        );
                    }
                }
            }
            assert_eq!(
                st.state_root(),
                st.state_root_full_rebuild(),
                "seed {seed} final"
            );
            // Unwind every open checkpoint (innermost first), checking after each.
            while let Some(cp) = cps.pop() {
                st.revert(cp);
                assert_eq!(
                    st.state_root(),
                    st.state_root_full_rebuild(),
                    "seed {seed} unwind"
                );
            }
            // Persistence round trip lands on the same root.
            let (a, c, s) = st.export_for_persistence();
            let mut loaded = StateStore::load_from_persistence(a, c, s);
            loaded.load_qa_pending(st.export_qa_pending());
            *loaded.names_mut() = st.names().clone();
            assert_eq!(
                loaded.state_root(),
                st.state_root(),
                "seed {seed} persistence"
            );
        }
    }
}
