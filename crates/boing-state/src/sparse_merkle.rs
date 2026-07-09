//! Sparse Merkle tree — compact state representation for Boing.
//!
//! Enables compact proofs and stateless client verification.
//! Upgradable to Verkle trees later.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use boing_primitives::{hasher, AccountId, AccountState, Hash};

const EMPTY_HASH: Hash = Hash::ZERO;

fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    let mut h = hasher();
    h.update(left.as_bytes());
    h.update(right.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    Hash(out)
}

/// Sparse Merkle tree over 32-byte keys → 32-byte leaf value hashes.
///
/// Used for the global account tree and per-contract storage trees.
#[derive(Default)]
pub struct SparseMerkleTree {
    leaves: HashMap<[u8; 32], [u8; 32]>,
    root_cache: Option<Hash>,
}

impl SparseMerkleTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a pre-hashed leaf value under `key`.
    pub fn insert_leaf_hash(&mut self, key: [u8; 32], value_hash: Hash) {
        self.leaves.insert(key, value_hash.0);
        self.root_cache = None;
    }

    /// Insert an account leaf with **no** code/storage commitment (tests / legacy).
    /// Production [`crate::StateStore::state_root`] uses [`hash_account_leaf`] instead.
    pub fn insert(&mut self, key: AccountId, value: &AccountState) {
        let value_hash = hash_account_leaf(value, &Hash::ZERO, &Hash::ZERO);
        self.insert_leaf_hash(key.0, value_hash);
    }

    pub fn get(&self, key: &AccountId) -> Option<&[u8; 32]> {
        self.leaves.get(&key.0)
    }

    pub fn get_raw(&self, key: &[u8; 32]) -> Option<&[u8; 32]> {
        self.leaves.get(key)
    }

    pub fn delete(&mut self, key: &AccountId) {
        self.leaves.remove(&key.0);
        self.root_cache = None;
    }

    pub fn delete_raw(&mut self, key: &[u8; 32]) {
        self.leaves.remove(key);
        self.root_cache = None;
    }

    /// BLAKE3 of a 32-byte storage word (storage SMT leaf value).
    pub fn hash_storage_value(value: &[u8; 32]) -> Hash {
        let mut h = hasher();
        h.update(value);
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        Hash(out)
    }

    /// Insert a contract storage slot into this tree (key = slot, leaf = BLAKE3(value)).
    pub fn insert_storage_slot(&mut self, slot: [u8; 32], value: &[u8; 32]) {
        self.insert_leaf_hash(slot, Self::hash_storage_value(value));
    }
}

/// Account leaf commitment included in the global state SMT:
/// `BLAKE3(balance_le || nonce_le || stake_le || code_hash || storage_root)`.
///
/// - `code_hash`: [`Hash::ZERO`] if the account has no bytecode; otherwise `BLAKE3(bytecode)`.
/// - `storage_root`: root of the per-contract storage SMT, or [`Hash::ZERO`] if empty.
pub fn hash_account_leaf(v: &AccountState, code_hash: &Hash, storage_root: &Hash) -> Hash {
    let mut h = hasher();
    h.update(&v.balance.to_le_bytes());
    h.update(&v.nonce.to_le_bytes());
    h.update(&v.stake.to_le_bytes());
    h.update(&v.pending_unbond.to_le_bytes());
    h.update(&v.unbond_unlock_height.to_le_bytes());
    h.update(code_hash.as_bytes());
    h.update(storage_root.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    Hash(out)
}

/// `BLAKE3(bytecode)` for deployed code; [`Hash::ZERO`] when `bytecode` is empty / absent.
pub fn hash_contract_code(bytecode: &[u8]) -> Hash {
    if bytecode.is_empty() {
        return Hash::ZERO;
    }
    let mut h = hasher();
    h.update(bytecode);
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    Hash(out)
}

impl SparseMerkleTree {

    /// Compute root hash. Cached for repeated calls.
    pub fn root(&mut self) -> Hash {
        if let Some(r) = self.root_cache {
            return r;
        }
        let root = self.compute_root_impl(self.leaves.iter().map(|(k, v)| (*k, Hash(*v))));
        self.root_cache = Some(root);
        root
    }

    fn compute_root_impl(&self, entries: impl Iterator<Item = ([u8; 32], Hash)>) -> Hash {
        let mut sorted: Vec<_> = entries.collect();
        if sorted.is_empty() {
            return EMPTY_HASH;
        }
        sorted.sort_by(|a, b| a.0.as_ref().cmp(b.0.as_ref()));
        Self::build_root(&sorted, 0, 64, 0)
    }

    fn build_root(entries: &[([u8; 32], Hash)], depth: u32, max_depth: u32, _bit_prefix: u64) -> Hash {
        if entries.is_empty() {
            return EMPTY_HASH;
        }
        if depth >= max_depth || entries.len() == 1 {
            let (k, v) = &entries[0];
            let mut h = hasher();
            h.update(k);
            h.update(v.as_bytes());
            let mut out = [0u8; 32];
            out.copy_from_slice(h.finalize().as_bytes());
            return Hash(out);
        }
        let shift = (max_depth - 1).saturating_sub(depth).min(63);
        let mid = 1u64 << shift;
        let (left_entries, right_entries): (Vec<_>, Vec<_>) = entries
            .iter()
            .partition(|(k, _)| Self::path_bit(k, depth) == 0);
        let left_hash = if left_entries.is_empty() {
            EMPTY_HASH
        } else {
            Self::build_root(&left_entries, depth + 1, max_depth, _bit_prefix)
        };
        let right_hash = if right_entries.is_empty() {
            EMPTY_HASH
        } else {
            Self::build_root(&right_entries, depth + 1, max_depth, _bit_prefix | mid)
        };
        hash_pair(&left_hash, &right_hash)
    }

    fn path_bit(key: &[u8; 32], depth: u32) -> u8 {
        let byte_idx = (depth / 8) as usize;
        let bit_idx = 7 - (depth % 8);
        if byte_idx >= 32 {
            return 0;
        }
        (key[byte_idx] >> bit_idx) & 1
    }

    /// Generate a Merkle proof for the given key.
    pub fn prove(&mut self, key: &AccountId) -> Option<MerkleProof> {
        let value_hash = *self.get(key)?;
        let root = self.root();
        let mut entries: Vec<_> = self
            .leaves
            .iter()
            .map(|(k, v)| (*k, Hash(*v)))
            .collect();
        if entries.is_empty() {
            return None;
        }
        entries.sort_by(|a, b| a.0.as_ref().cmp(b.0.as_ref()));
        let siblings = Self::collect_proof_path(&entries, key.0, 0, 64);
        Some(MerkleProof {
            key: key.0,
            value_hash: Hash(value_hash),
            siblings,
            root,
        })
    }

    /// Collect sibling hashes along path from root to leaf for `key`.
    fn collect_proof_path(
        entries: &[([u8; 32], Hash)],
        key: [u8; 32],
        depth: u32,
        max_depth: u32,
    ) -> Vec<ProofStep> {
        if entries.is_empty() || depth >= max_depth {
            return vec![];
        }
        if entries.len() == 1 {
            return vec![];
        }
        let (left_entries, right_entries): (Vec<_>, Vec<_>) =
            entries.iter().cloned().partition(|(k, _)| Self::path_bit(k, depth) == 0);
        let bit = Self::path_bit(&key, depth);
        let (my_entries, sibling_entries) = if bit == 0 {
            (left_entries, right_entries)
        } else {
            (right_entries, left_entries)
        };
        let sibling_hash = if sibling_entries.is_empty() {
            EMPTY_HASH
        } else {
            Self::hash_subtree_impl(&sibling_entries, depth + 1, max_depth)
        };
        let mut path = vec![ProofStep {
            sibling_hash,
            path_bit: bit,
        }];
        path.extend(Self::collect_proof_path(&my_entries, key, depth + 1, max_depth));
        path
    }

    fn hash_subtree_impl(
        entries: &[([u8; 32], Hash)],
        depth: u32,
        max_depth: u32,
    ) -> Hash {
        if entries.is_empty() {
            return EMPTY_HASH;
        }
        if depth >= max_depth || entries.len() == 1 {
            let (k, v) = &entries[0];
            let mut h = hasher();
            h.update(k);
            h.update(v.as_bytes());
            let mut out = [0u8; 32];
            out.copy_from_slice(h.finalize().as_bytes());
            return Hash(out);
        }
        let (left_entries, right_entries): (Vec<_>, Vec<_>) =
            entries.iter().cloned().partition(|(k, _)| Self::path_bit(k, depth) == 0);
        let left_hash = Self::hash_subtree_impl(&left_entries, depth + 1, max_depth);
        let right_hash = Self::hash_subtree_impl(&right_entries, depth + 1, max_depth);
        hash_pair(&left_hash, &right_hash)
    }
}

/// Single step in a Merkle proof: sibling hash and direction (path_bit).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofStep {
    pub sibling_hash: Hash,
    /// 0 = we went left, sibling is right; 1 = we went right, sibling is left.
    pub path_bit: u8,
}

/// Merkle proof for a key-value pair. Verifiable against root.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProof {
    pub key: [u8; 32],
    pub value_hash: Hash,
    pub siblings: Vec<ProofStep>,
    pub root: Hash,
}

impl MerkleProof {
    /// Verify this proof produces the expected root.
    pub fn verify(&self) -> bool {
        let computed = Self::compute_root_from_proof(&self.key, &self.value_hash, &self.siblings);
        computed == self.root
    }

    fn compute_root_from_proof(
        key: &[u8; 32],
        value_hash: &Hash,
        steps: &[ProofStep],
    ) -> Hash {
        let mut h = hasher();
        h.update(key);
        h.update(value_hash.as_bytes());
        let mut current = [0u8; 32];
        current.copy_from_slice(h.finalize().as_bytes());
        let mut current_hash = Hash(current);

        for step in steps {
            let (left, right) = if step.path_bit == 0 {
                (current_hash, step.sibling_hash)
            } else {
                (step.sibling_hash, current_hash)
            };
            current_hash = hash_pair(&left, &right);
        }
        current_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_root() {
        let mut tree = SparseMerkleTree::new();
        assert_eq!(tree.root(), Hash::ZERO);
    }

    #[test]
    fn test_insert_root_deterministic() {
        let mut tree = SparseMerkleTree::new();
        let id = AccountId([1u8; 32]);
        tree.insert(id, &AccountState { balance: 100, nonce: 0, stake: 0, ..Default::default() });
        let r1 = tree.root();
        let r2 = tree.root();
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_insert_different_values_different_roots() {
        let mut t1 = SparseMerkleTree::new();
        let mut t2 = SparseMerkleTree::new();
        let id = AccountId([1u8; 32]);
        t1.insert(id, &AccountState { balance: 100, nonce: 0, stake: 0, ..Default::default() });
        t2.insert(id, &AccountState { balance: 200, nonce: 0, stake: 0, ..Default::default() });
        assert_ne!(t1.root(), t2.root());
    }

    #[test]
    fn test_get() {
        let mut tree = SparseMerkleTree::new();
        let id = AccountId([1u8; 32]);
        tree.insert(id, &AccountState { balance: 42, nonce: 1, stake: 0, ..Default::default() });
        assert!(tree.get(&id).is_some());
        assert!(tree.get(&AccountId([2u8; 32])).is_none());
    }

    #[test]
    fn test_prove_and_verify() {
        let mut tree = SparseMerkleTree::new();
        // Keys differing in first bit: simple 2-leaf tree
        let mut k1 = [0u8; 32];
        k1[0] = 0;
        let mut k2 = [0u8; 32];
        k2[0] = 128;
        let id1 = AccountId(k1);
        let id2 = AccountId(k2);
        tree.insert(id1, &AccountState { balance: 100, nonce: 0, stake: 0, ..Default::default() });
        tree.insert(id2, &AccountState { balance: 200, nonce: 1, stake: 0, ..Default::default() });
        assert!(tree.prove(&id1).unwrap().verify());
        assert!(tree.prove(&id2).unwrap().verify());
    }

    #[test]
    fn test_delete() {
        let mut tree = SparseMerkleTree::new();
        let id = AccountId([1u8; 32]);
        tree.insert(id, &AccountState { balance: 100, nonce: 0, stake: 0, ..Default::default() });
        let root_with = tree.root();
        tree.delete(&id);
        assert!(tree.get(&id).is_none());
        assert_eq!(tree.root(), Hash::ZERO);
        assert_ne!(root_with, Hash::ZERO);
    }
}
