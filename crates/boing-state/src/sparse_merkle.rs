//! Sparse Merkle tree — compact state representation for Boing.
//!
//! Enables compact proofs and stateless client verification.
//! Upgradable to Verkle trees later.

use std::collections::HashMap;

use boing_primitives::{hasher, AccountId, AccountState, Hash};
use serde::{Deserialize, Serialize};

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
///
/// The root is maintained **incrementally** by an arena-backed path-compressed binary trie
/// ([`Trie`]): an insert/delete re-hashes only the `O(depth)` nodes on the touched path, and
/// cloning the tree is a flat memcpy of the arena. The committed root is byte-for-byte identical
/// to the original full-rebuild algorithm ([`SparseMerkleTree::reference_root`]), which is kept as
/// the oracle for differential tests.
#[derive(Clone)]
pub struct SparseMerkleTree {
    leaves: HashMap<[u8; 32], [u8; 32]>,
    trie: Trie,
    max_depth: u32,
}

impl Default for SparseMerkleTree {
    fn default() -> Self {
        Self::with_max_depth(LEGACY_MAX_DEPTH)
    }
}

/// Legacy trie depth: keys are split on their first 64 bits (consensus-defined; see
/// [`SparseMerkleTree::reference_root`]). Entries that share all 64 bits are bucketed and only the
/// smallest key of a bucket is committed. This is the depth of the account tree and, until the
/// storage-tree v2 activation, of every per-contract storage tree.
pub const LEGACY_MAX_DEPTH: u32 = 64;
/// Full-key depth (storage-tree v2): every bit of a 32-byte key routes, so two distinct keys never
/// share a bucket and every slot is committed. Single-entry subtrees still hash as `H(k||v)`, so
/// the cost is proportional to the occupied path, not to 256.
pub const DEEP_MAX_DEPTH: u32 = 256;
const NIL: u32 = u32::MAX;

fn leaf_hash(key: &[u8; 32], val: &[u8; 32]) -> [u8; 32] {
    let mut h = hasher();
    h.update(key);
    h.update(val);
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    out
}

#[derive(Clone, Debug)]
enum Node {
    Leaf {
        key: [u8; 32],
        val: [u8; 32],
    },
    /// Two or more entries sharing the first 64 key bits (only the smallest key is committed).
    Bucket(Vec<([u8; 32], [u8; 32])>),
    /// Subtree with at least two entries. `hash` is a cache cleared on any change below.
    Branch {
        left: u32,
        right: u32,
        hash: Option<[u8; 32]>,
    },
    Free,
}

/// Arena trie. Invariants: a subtree holding exactly one entry is a single `Leaf` (placed as high as
/// possible, like the original "single entry => H(k||v)" rule), a `Branch` always has >= 2 entries
/// below it, a `Bucket` has >= 2 entries and only exists at depth 64.
#[derive(Clone, Debug)]
struct Trie {
    nodes: Vec<Node>,
    free: Vec<u32>,
    root: u32,
    max_depth: u32,
}

impl Default for Trie {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            free: Vec::new(),
            root: NIL,
            max_depth: LEGACY_MAX_DEPTH,
        }
    }
}

impl Trie {
    fn alloc(&mut self, n: Node) -> u32 {
        if let Some(i) = self.free.pop() {
            self.nodes[i as usize] = n;
            i
        } else {
            self.nodes.push(n);
            (self.nodes.len() - 1) as u32
        }
    }

    fn release(&mut self, i: u32) {
        self.nodes[i as usize] = Node::Free;
        self.free.push(i);
    }

    fn insert(&mut self, key: [u8; 32], val: [u8; 32]) {
        self.root = self.ins(self.root, 0, key, val);
    }

    fn remove(&mut self, key: &[u8; 32]) {
        self.root = self.rem(self.root, 0, key);
    }

    fn ins(&mut self, idx: u32, d: u32, key: [u8; 32], val: [u8; 32]) -> u32 {
        if idx == NIL {
            return self.alloc(Node::Leaf { key, val });
        }
        let max_depth = self.max_depth;
        match &mut self.nodes[idx as usize] {
            Node::Leaf { key: k0, val: v0 } => {
                if *k0 == key {
                    *v0 = val;
                    return idx;
                }
                let (k0, v0) = (*k0, *v0);
                if d >= max_depth {
                    let mut v = vec![(k0, v0), (key, val)];
                    v.sort_by_key(|a| a.0);
                    self.nodes[idx as usize] = Node::Bucket(v);
                } else {
                    self.nodes[idx as usize] = Node::Branch {
                        left: NIL,
                        right: NIL,
                        hash: None,
                    };
                    self.ins(idx, d, k0, v0);
                    self.ins(idx, d, key, val);
                }
                idx
            }
            Node::Bucket(v) => {
                match v.binary_search_by(|e| e.0.cmp(&key)) {
                    Ok(i) => v[i].1 = val,
                    Err(i) => v.insert(i, (key, val)),
                }
                idx
            }
            Node::Branch { left, right, hash } => {
                *hash = None;
                let go_left = SparseMerkleTree::path_bit(&key, d) == 0;
                let child = if go_left { *left } else { *right };
                let nc = self.ins(child, d + 1, key, val);
                if let Node::Branch { left, right, .. } = &mut self.nodes[idx as usize] {
                    if go_left {
                        *left = nc;
                    } else {
                        *right = nc;
                    }
                }
                idx
            }
            Node::Free => {
                debug_assert!(false, "trie points at a freed node");
                self.alloc(Node::Leaf { key, val })
            }
        }
    }

    fn rem(&mut self, idx: u32, d: u32, key: &[u8; 32]) -> u32 {
        if idx == NIL {
            return NIL;
        }
        match &mut self.nodes[idx as usize] {
            Node::Leaf { key: k0, .. } => {
                if k0 == key {
                    self.release(idx);
                    NIL
                } else {
                    idx
                }
            }
            Node::Bucket(v) => {
                if let Ok(i) = v.binary_search_by(|e| e.0.cmp(key)) {
                    v.remove(i);
                    if v.len() == 1 {
                        let (k, val) = v[0];
                        self.nodes[idx as usize] = Node::Leaf { key: k, val };
                    }
                }
                idx
            }
            Node::Branch { left, right, hash } => {
                *hash = None;
                let go_left = SparseMerkleTree::path_bit(key, d) == 0;
                let child = if go_left { *left } else { *right };
                let nc = self.rem(child, d + 1, key);
                let (l, r) = match &mut self.nodes[idx as usize] {
                    Node::Branch { left, right, .. } => {
                        if go_left {
                            *left = nc;
                        } else {
                            *right = nc;
                        }
                        (*left, *right)
                    }
                    _ => return idx,
                };
                if l == NIL && r == NIL {
                    self.release(idx);
                    return NIL;
                }
                if l == NIL || r == NIL {
                    let only = if l == NIL { r } else { l };
                    // A lone entry hashes as H(k||v) at any depth: lift it into the parent slot.
                    if matches!(self.nodes[only as usize], Node::Leaf { .. }) {
                        self.release(idx);
                        return only;
                    }
                }
                idx
            }
            Node::Free => idx,
        }
    }

    /// Sibling hashes from the root down to `key`'s leaf (stops at the first single-entry subtree
    /// or bucket), reusing cached node hashes: O(depth) after the root has been computed.
    fn proof_path(&mut self, key: &[u8; 32]) -> Vec<ProofStep> {
        let mut out = Vec::new();
        let mut idx = self.root;
        let mut d = 0u32;
        while idx != NIL {
            let (l, r) = match &self.nodes[idx as usize] {
                Node::Branch { left, right, .. } => (*left, *right),
                _ => break,
            };
            let bit = SparseMerkleTree::path_bit(key, d);
            let (mine, sibling) = if bit == 0 { (l, r) } else { (r, l) };
            let sibling_hash = Hash(self.hash_of(sibling));
            out.push(ProofStep {
                sibling_hash,
                path_bit: bit,
            });
            idx = mine;
            d += 1;
        }
        out
    }

    fn hash_of(&mut self, idx: u32) -> [u8; 32] {
        if idx == NIL {
            return [0u8; 32];
        }
        match &self.nodes[idx as usize] {
            Node::Leaf { key, val } => leaf_hash(key, val),
            Node::Bucket(v) => leaf_hash(&v[0].0, &v[0].1),
            Node::Branch { hash: Some(h), .. } => *h,
            Node::Branch {
                left,
                right,
                hash: None,
            } => {
                let (l, r) = (*left, *right);
                let lh = self.hash_of(l);
                let rh = self.hash_of(r);
                let h = hash_pair(&Hash(lh), &Hash(rh)).0;
                if let Node::Branch { hash, .. } = &mut self.nodes[idx as usize] {
                    *hash = Some(h);
                }
                h
            }
            Node::Free => [0u8; 32],
        }
    }
}

impl SparseMerkleTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty tree that routes on the first `max_depth` key bits ([`LEGACY_MAX_DEPTH`] or
    /// [`DEEP_MAX_DEPTH`]).
    pub fn with_max_depth(max_depth: u32) -> Self {
        let mut trie = Trie::default();
        trie.max_depth = max_depth;
        Self {
            leaves: HashMap::new(),
            trie,
            max_depth,
        }
    }

    /// Routing depth of this tree.
    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    /// Insert a pre-hashed leaf value under `key`.
    pub fn insert_leaf_hash(&mut self, key: [u8; 32], value_hash: Hash) {
        if self.leaves.get(&key) == Some(&value_hash.0) {
            return; // unchanged: keep every cached node hash
        }
        self.leaves.insert(key, value_hash.0);
        self.trie.insert(key, value_hash.0);
    }

    /// Number of committed leaves.
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
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
        self.delete_raw(&key.0);
    }

    pub fn delete_raw(&mut self, key: &[u8; 32]) {
        if self.leaves.remove(key).is_some() {
            self.trie.remove(key);
        }
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
    /// Root hash. Incremental: only nodes dirtied since the last call are re-hashed.
    pub fn root(&mut self) -> Hash {
        let r = self.trie.root;
        Hash(self.trie.hash_of(r))
    }

    /// Root computed with the original full-rebuild algorithm (sort + recursive partition). This is
    /// the consensus definition the incremental trie must match exactly; used as a test oracle and
    /// by `StateStore::state_root_full_rebuild`.
    pub fn reference_root(&self) -> Hash {
        self.compute_root_impl(self.leaves.iter().map(|(k, v)| (*k, Hash(*v))))
    }

    fn compute_root_impl(&self, entries: impl Iterator<Item = ([u8; 32], Hash)>) -> Hash {
        let mut sorted: Vec<_> = entries.collect();
        if sorted.is_empty() {
            return EMPTY_HASH;
        }
        sorted.sort_by(|a, b| a.0.as_ref().cmp(b.0.as_ref()));
        Self::build_root(&sorted, 0, self.max_depth, 0)
    }

    fn build_root(
        entries: &[([u8; 32], Hash)],
        depth: u32,
        max_depth: u32,
        _bit_prefix: u64,
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
        let siblings = self.trie.proof_path(&key.0);
        Some(MerkleProof {
            key: key.0,
            value_hash: Hash(value_hash),
            siblings,
            root,
        })
    }

    /// Original sort-and-partition proof builder (O(n log n) per proof); the oracle for
    /// [`SparseMerkleTree::prove`] in differential tests.
    #[cfg(test)]
    fn prove_reference(&self, key: &AccountId) -> Option<MerkleProof> {
        let value_hash = *self.get(key)?;
        let mut entries: Vec<_> = self.leaves.iter().map(|(k, v)| (*k, Hash(*v))).collect();
        entries.sort_by(|a, b| a.0.as_ref().cmp(b.0.as_ref()));
        let siblings = Self::collect_proof_path(&entries, key.0, 0, self.max_depth);
        Some(MerkleProof {
            key: key.0,
            value_hash: Hash(value_hash),
            siblings,
            root: self.reference_root(),
        })
    }

    /// Collect sibling hashes along path from root to leaf for `key`.
    #[cfg(test)]
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
        let (left_entries, right_entries): (Vec<_>, Vec<_>) = entries
            .iter()
            .cloned()
            .partition(|(k, _)| Self::path_bit(k, depth) == 0);
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
        path.extend(Self::collect_proof_path(
            &my_entries,
            key,
            depth + 1,
            max_depth,
        ));
        path
    }

    #[cfg(test)]
    fn hash_subtree_impl(entries: &[([u8; 32], Hash)], depth: u32, max_depth: u32) -> Hash {
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
        let (left_entries, right_entries): (Vec<_>, Vec<_>) = entries
            .iter()
            .cloned()
            .partition(|(k, _)| Self::path_bit(k, depth) == 0);
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
    /// Verify this proof is internally consistent with the root embedded in the proof.
    ///
    /// Callers must also check that root against a **trusted** value (e.g. a block header
    /// state root) via [`Self::verify_against`] (audit #61 item 33).
    pub fn verify(&self) -> bool {
        self.verify_against(&self.root)
    }

    /// Verify the proof path and that the computed root equals `trusted_root`.
    pub fn verify_against(&self, trusted_root: &Hash) -> bool {
        let computed = Self::compute_root_from_proof(&self.key, &self.value_hash, &self.siblings);
        computed == *trusted_root && self.root == *trusted_root
    }

    fn compute_root_from_proof(key: &[u8; 32], value_hash: &Hash, steps: &[ProofStep]) -> Hash {
        let mut h = hasher();
        h.update(key);
        h.update(value_hash.as_bytes());
        let mut current = [0u8; 32];
        current.copy_from_slice(h.finalize().as_bytes());
        let mut current_hash = Hash(current);

        // `steps` run root -> leaf (the order `prove` emits); fold from the leaf upward.
        for step in steps.iter().rev() {
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
        tree.insert(
            id,
            &AccountState {
                balance: 100,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        );
        let r1 = tree.root();
        let r2 = tree.root();
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_insert_different_values_different_roots() {
        let mut t1 = SparseMerkleTree::new();
        let mut t2 = SparseMerkleTree::new();
        let id = AccountId([1u8; 32]);
        t1.insert(
            id,
            &AccountState {
                balance: 100,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        );
        t2.insert(
            id,
            &AccountState {
                balance: 200,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        );
        assert_ne!(t1.root(), t2.root());
    }

    #[test]
    fn test_get() {
        let mut tree = SparseMerkleTree::new();
        let id = AccountId([1u8; 32]);
        tree.insert(
            id,
            &AccountState {
                balance: 42,
                nonce: 1,
                stake: 0,
                ..Default::default()
            },
        );
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
        tree.insert(
            id1,
            &AccountState {
                balance: 100,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        );
        tree.insert(
            id2,
            &AccountState {
                balance: 200,
                nonce: 1,
                stake: 0,
                ..Default::default()
            },
        );
        assert!(tree.prove(&id1).unwrap().verify());
        assert!(tree.prove(&id2).unwrap().verify());
    }

    #[test]
    fn test_delete() {
        let mut tree = SparseMerkleTree::new();
        let id = AccountId([1u8; 32]);
        tree.insert(
            id,
            &AccountState {
                balance: 100,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        );
        let root_with = tree.root();
        tree.delete(&id);
        assert!(tree.get(&id).is_none());
        assert_eq!(tree.root(), Hash::ZERO);
        assert_ne!(root_with, Hash::ZERO);
    }

    // ---- Differential tests: incremental trie vs. the original full-rebuild algorithm ----

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            // xorshift64*
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
    }

    /// Keys engineered to stress the trie: random, long shared prefixes (deep chains), and keys that
    /// share all 64 routing bits (buckets), plus a tiny key space so updates/deletes hit existing keys.
    fn gen_key(r: &mut Rng) -> [u8; 32] {
        let mut k = [0u8; 32];
        match r.next() % 4 {
            0 => {
                for b in k.iter_mut() {
                    *b = r.next() as u8;
                }
            }
            1 => {
                // shared 40-bit prefix, differ afterwards
                k[..5].copy_from_slice(&[0xAB, 0xCD, 0xEF, 0x01, 0x23]);
                for b in k[5..].iter_mut() {
                    *b = r.next() as u8;
                }
            }
            2 => {
                // identical first 8 bytes (all 64 routing bits): bucket case, like small-integer slots
                k[24..].copy_from_slice(&(r.next() % 6).to_be_bytes());
            }
            _ => {
                k[0] = (r.next() % 4) as u8;
                k[7] = (r.next() % 3) as u8;
                k[31] = (r.next() % 3) as u8;
            }
        }
        k
    }

    #[test]
    fn incremental_root_matches_reference_under_random_ops() {
        for seed in 1..=40u64 {
            let mut r = Rng(0x9E37_79B9_7F4A_7C15 ^ seed.wrapping_mul(0xD1B5_4A32_D192_ED03));
            let mut t = SparseMerkleTree::new();
            for step in 0..400 {
                let k = gen_key(&mut r);
                if r.next().is_multiple_of(3) {
                    t.delete_raw(&k);
                } else {
                    let mut v = [0u8; 32];
                    v[..8].copy_from_slice(&(r.next() % 5).to_le_bytes());
                    t.insert_leaf_hash(k, Hash(v));
                }
                // Check often early on (small trees) and sparsely later.
                if step < 60 || step % 7 == 0 {
                    assert_eq!(t.root(), t.reference_root(), "seed {seed} step {step}");
                }
            }
            assert_eq!(t.root(), t.reference_root(), "seed {seed} final");
            // Cloned tree has the same root and diverges independently.
            let mut c = t.clone();
            assert_eq!(c.root(), t.root());
            c.insert_leaf_hash([0xEE; 32], Hash([1u8; 32]));
            assert_ne!(c.root(), t.root());
            assert_eq!(c.root(), c.reference_root());
            assert_eq!(t.root(), t.reference_root());
            // Drain everything: root returns to empty.
            let keys: Vec<[u8; 32]> = t.leaves.keys().copied().collect();
            for k in keys {
                t.delete_raw(&k);
                assert_eq!(t.root(), t.reference_root());
            }
            assert_eq!(t.root(), Hash::ZERO);
        }
    }

    #[test]
    fn trie_proofs_match_reference_and_verify() {
        for (depth, seed) in [(LEGACY_MAX_DEPTH, 3u64), (DEEP_MAX_DEPTH, 4u64)] {
            let mut r = Rng(0xABCD_EF01_2345_6789 ^ seed);
            let mut t = SparseMerkleTree::with_max_depth(depth);
            let mut keys = Vec::new();
            for i in 0..300 {
                let k = gen_key(&mut r);
                t.insert_leaf_hash(k, Hash([(i % 5) as u8 + 1; 32]));
                keys.push(k);
            }
            for k in keys.iter().take(60) {
                t.delete_raw(k);
            }
            let root = t.reference_root();
            assert_eq!(t.root(), root);
            for k in keys.iter().skip(60) {
                let id = AccountId(*k);
                if t.get(&id).is_none() {
                    continue; // duplicate key that an earlier delete removed
                }
                let fast = t.prove(&id).expect("present");
                let slow = t.prove_reference(&id).expect("present");
                assert_eq!(fast.siblings, slow.siblings, "depth {depth}");
                assert_eq!(fast.root, root);
                if depth == DEEP_MAX_DEPTH {
                    assert!(fast.verify(), "deep proofs always verify");
                }
            }
        }
    }

    #[test]
    fn insertion_order_does_not_change_root() {
        let mut r = Rng(42);
        let entries: Vec<([u8; 32], Hash)> = (0..200)
            .map(|i| (gen_key(&mut r), Hash([(i % 7) as u8; 32])))
            .collect();
        let mut a = SparseMerkleTree::new();
        for (k, v) in &entries {
            a.insert_leaf_hash(*k, *v);
        }
        let mut b = SparseMerkleTree::new();
        for (k, v) in entries.iter().rev() {
            b.insert_leaf_hash(*k, *v);
        }
        // Duplicate keys with different values make "last write wins" order-dependent; compare to
        // the reference of each tree instead of each other.
        assert_eq!(a.root(), a.reference_root());
        assert_eq!(b.root(), b.reference_root());
    }

    /// Deep (256-bit) trees commit every key, including ones that share their first 64 bits, and
    /// match the reference algorithm run at depth 256.
    #[test]
    fn deep_tree_commits_keys_sharing_64_bit_prefix() {
        let mut k1 = [0u8; 32];
        let mut k2 = [0u8; 32];
        k1[31] = 1;
        k2[31] = 2;
        let mut t = SparseMerkleTree::with_max_depth(DEEP_MAX_DEPTH);
        t.insert_leaf_hash(k1, Hash([7u8; 32]));
        t.insert_leaf_hash(k2, Hash([9u8; 32]));
        let two = t.root();
        t.insert_leaf_hash(k2, Hash([10u8; 32]));
        assert_ne!(
            t.root(),
            two,
            "second key's value IS committed at depth 256"
        );
        assert_eq!(t.root(), t.reference_root());
        t.delete_raw(&k2);
        let mut only = SparseMerkleTree::with_max_depth(DEEP_MAX_DEPTH);
        only.insert_leaf_hash(k1, Hash([7u8; 32]));
        assert_eq!(
            t.root(),
            only.root(),
            "delete restores the single-leaf root"
        );
        // A single entry hashes as H(k||v) at any depth, so the legacy and deep roots agree.
        let mut legacy = SparseMerkleTree::new();
        legacy.insert_leaf_hash(k1, Hash([7u8; 32]));
        assert_eq!(legacy.root(), only.root());
    }

    #[test]
    fn deep_incremental_root_matches_reference_under_random_ops() {
        for seed in 1..=20u64 {
            let mut r = Rng(0x1234_5678_9ABC_DEF1 ^ seed.wrapping_mul(0xD1B5_4A32_D192_ED03));
            let mut t = SparseMerkleTree::with_max_depth(DEEP_MAX_DEPTH);
            for step in 0..300 {
                let k = gen_key(&mut r);
                if r.next().is_multiple_of(3) {
                    t.delete_raw(&k);
                } else {
                    let mut v = [0u8; 32];
                    v[..8].copy_from_slice(&(r.next() % 5).to_le_bytes());
                    t.insert_leaf_hash(k, Hash(v));
                }
                if step < 60 || step % 7 == 0 {
                    assert_eq!(t.root(), t.reference_root(), "seed {seed} step {step}");
                }
            }
            let keys: Vec<[u8; 32]> = t.leaves.keys().copied().collect();
            for k in keys {
                t.delete_raw(&k);
                assert_eq!(t.root(), t.reference_root());
            }
            assert_eq!(t.root(), Hash::ZERO);
        }
    }

    /// Documents a consensus quirk (see docs/audit): keys that share their first 64 bits collapse to
    /// the smallest key, so the other entries are NOT committed by the root.
    #[test]
    fn keys_sharing_64_bit_prefix_commit_only_smallest_key() {
        let mut k1 = [0u8; 32];
        let mut k2 = [0u8; 32];
        k1[31] = 1;
        k2[31] = 2;
        let mut t = SparseMerkleTree::new();
        t.insert_leaf_hash(k1, Hash([7u8; 32]));
        let one = t.root();
        t.insert_leaf_hash(k2, Hash([9u8; 32]));
        let two = t.root();
        assert_ne!(
            two, one,
            "the second key only adds the (empty-sibling) path hashes"
        );
        // ...but its VALUE is not committed: changing it leaves the root untouched (known limitation).
        t.insert_leaf_hash(k2, Hash([10u8; 32]));
        assert_eq!(t.root(), two);
        assert_eq!(t.root(), t.reference_root());
        // Changing the smallest key's value does change the root.
        t.insert_leaf_hash(k1, Hash([8u8; 32]));
        assert_ne!(t.root(), two);
    }
}
