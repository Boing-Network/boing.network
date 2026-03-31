//! Chain state — tracks latest block and height.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use boing_primitives::{AccountId, Block, Hash};

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
            },
            transactions: vec![],
        }
    }

    pub fn new(genesis_proposer: AccountId) -> Self {
        let genesis = Self::genesis(genesis_proposer);
        Self::from_genesis(genesis)
    }

    pub fn from_genesis(genesis: Block) -> Self {
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
            })),
        }
    }

    pub fn height(&self) -> u64 {
        self.inner.read().unwrap().height
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
        Ok(())
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
