//! Chain state — tracks latest block and height.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use boing_primitives::{AccountId, Block, Hash};

/// Default in-memory window of recent blocks (older heights remain on disk via persistence).
/// `0` keeps every block in RAM (previous behaviour).
pub const DEFAULT_IN_MEMORY_BLOCK_WINDOW: u64 = 100_000;

/// Chain head — latest committed block.
#[derive(Clone, Debug)]
pub struct ChainState {
    inner: Arc<RwLock<ChainStateInner>>,
}

#[derive(Clone, Debug)]
struct ChainStateInner {
    pub height: u64,
    pub latest_hash: Hash,
    pub latest_block: Option<Block>,
    /// Block index: height -> block, hash -> block (for RPC queries).
    blocks_by_height: HashMap<u64, Block>,
    blocks_by_hash: HashMap<Hash, Block>,
    /// Keep at most this many recent heights in RAM (`0` = unbounded).
    memory_window: u64,
}

impl ChainState {
    pub fn genesis(proposer: AccountId) -> Block {
        Block {
            header: boing_primitives::BlockHeader {
                parent_hash: Hash::ZERO,
                height: 0,
                timestamp: 0,
                proposer,
                tx_root: Hash::ZERO,
                receipts_root: Hash::ZERO,
                state_root: Hash::ZERO,
                blobs_root: None,
            },
            transactions: vec![],
            signatures: vec![],
            priority_tips: vec![],
        }
    }

    pub fn new(genesis_proposer: AccountId) -> Self {
        let genesis = Self::genesis(genesis_proposer);
        Self::from_genesis(genesis)
    }

    pub fn from_genesis(genesis: Block) -> Self {
        Self::from_genesis_with_window(genesis, DEFAULT_IN_MEMORY_BLOCK_WINDOW)
    }

    pub fn from_genesis_with_window(genesis: Block, memory_window: u64) -> Self {
        let hash = genesis.hash();
        let mut blocks_by_height = HashMap::new();
        blocks_by_height.insert(0, genesis.clone());
        let mut blocks_by_hash = HashMap::new();
        blocks_by_hash.insert(hash, genesis.clone());
        Self {
            inner: Arc::new(RwLock::new(ChainStateInner {
                height: 0,
                latest_hash: hash,
                latest_block: Some(genesis),
                blocks_by_height,
                blocks_by_hash,
                memory_window,
            })),
        }
    }

    /// Cap how many recent heights stay resident (`0` = keep all). Genesis (height 0) is always kept.
    pub fn set_memory_window(&self, window: u64) {
        let mut inner = self.inner.write().unwrap();
        inner.memory_window = window;
        Self::prune_locked(&mut inner);
    }

    pub fn height(&self) -> u64 {
        self.inner.read().unwrap().height
    }

    /// Tip block timestamp (0 if tip is missing — should not happen after genesis).
    pub fn tip_timestamp(&self) -> u64 {
        self.inner
            .read()
            .unwrap()
            .latest_block
            .as_ref()
            .map(|b| b.header.timestamp)
            .unwrap_or(0)
    }

    pub fn latest_hash(&self) -> Hash {
        self.inner.read().unwrap().latest_hash
    }

    pub fn parent_hash(&self) -> Hash {
        self.inner.read().unwrap().latest_hash
    }

    /// Append a committed block. Returns error if block doesn't chain.
    pub fn append(&self, block: Block) -> Result<(), ChainError> {
        let mut inner = self.inner.write().unwrap();
        if block.header.parent_hash != inner.latest_hash {
            return Err(ChainError::BlockNotChained);
        }
        if block.header.height != inner.height + 1 {
            return Err(ChainError::InvalidHeight);
        }
        let hash = block.hash();
        inner
            .blocks_by_height
            .insert(block.header.height, block.clone());
        inner.blocks_by_hash.insert(hash, block.clone());
        inner.height = block.header.height;
        inner.latest_hash = hash;
        inner.latest_block = Some(block);
        Self::prune_locked(&mut inner);
        Ok(())
    }

    fn prune_locked(inner: &mut ChainStateInner) {
        let window = inner.memory_window;
        if window == 0 {
            return;
        }
        let tip = inner.height;
        let first_keep = tip.saturating_sub(window.saturating_sub(1));
        let drop_heights: Vec<u64> = inner
            .blocks_by_height
            .keys()
            .copied()
            .filter(|h| *h != 0 && *h < first_keep)
            .collect();
        for h in drop_heights {
            if let Some(b) = inner.blocks_by_height.remove(&h) {
                inner.blocks_by_hash.remove(&b.hash());
            }
        }
    }

    /// Number of blocks currently held in RAM (including genesis).
    pub fn cached_block_count(&self) -> usize {
        self.inner.read().unwrap().blocks_by_height.len()
    }

    /// Get block by height, if present.
    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        self.inner
            .read()
            .unwrap()
            .blocks_by_height
            .get(&height)
            .cloned()
    }

    /// Run `f` on the block at `height` without cloning it (read-only; holds the chain read lock for
    /// the duration of `f`, so keep it short and never call back into [`ChainState`] from `f`).
    pub fn with_block_by_height<R>(&self, height: u64, f: impl FnOnce(&Block) -> R) -> Option<R> {
        self.inner
            .read()
            .unwrap()
            .blocks_by_height
            .get(&height)
            .map(f)
    }

    /// Get block by hash, if present.
    pub fn get_block_by_hash(&self, hash: &Hash) -> Option<Block> {
        self.inner.read().unwrap().blocks_by_hash.get(hash).cloned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("Block does not chain to latest")]
    BlockNotChained,
    #[error("Invalid block height")]
    InvalidHeight,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_window_keeps_genesis_and_recent_tip() {
        let proposer = AccountId([1u8; 32]);
        let chain = ChainState::from_genesis_with_window(ChainState::genesis(proposer), 3);
        let mut parent = chain.latest_hash();
        for h in 1..=5u64 {
            let block = Block {
                header: boing_primitives::BlockHeader {
                    parent_hash: parent,
                    height: h,
                    timestamp: h,
                    proposer,
                    tx_root: Hash::ZERO,
                    receipts_root: Hash::ZERO,
                    state_root: Hash::ZERO,
                    blobs_root: None,
                },
                transactions: vec![],
                signatures: vec![],
                priority_tips: vec![],
            };
            parent = block.hash();
            chain.append(block).unwrap();
        }
        assert!(chain.get_block_by_height(0).is_some());
        assert!(chain.get_block_by_height(1).is_none());
        assert!(chain.get_block_by_height(2).is_none());
        assert!(chain.get_block_by_height(3).is_some());
        assert!(chain.get_block_by_height(5).is_some());
        assert!(chain.cached_block_count() <= 4); // genesis + window
    }
}
