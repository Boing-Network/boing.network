//! Disk persistence for chain and state.
//!
//! Persists blocks and state to the data directory so the node can resume
//! after restart without losing data.

use std::path::{Path, PathBuf};

use boing_primitives::{AccountId, AccountState, Block, ExecutionReceipt, Hash};
use boing_qa::pool_config::QaPoolGovernanceConfig;
use boing_qa::RuleRegistry;
use boing_state::{ContractStorageEntry, StateStore};

use crate::chain::ChainState;

const CHAIN_DIR: &str = "chain";
const BLOCKS_DIR: &str = "blocks";
const CHAIN_META_FILE: &str = "meta.bin";
const STATE_DIR: &str = "state";
const STATE_FILE: &str = "accounts.bin";
const QA_REGISTRY_FILE: &str = "qa_registry.json";
const QA_POOL_CONFIG_FILE: &str = "qa_pool_config.json";

/// Chain metadata stored on disk.
#[derive(serde::Serialize, serde::Deserialize)]
struct ChainMeta {
    height: u64,
    latest_hash: Hash,
}

/// Persisted state: accounts and contract data.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct PersistedState {
    accounts: Vec<(AccountId, AccountState)>,
    contract_code: Vec<(AccountId, Vec<u8>)>,
    contract_storage: Vec<ContractStorageEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Disk-backed persistence for chain and state.
pub struct Persistence {
    base: PathBuf,
}

impl Persistence {
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
        }
    }

    fn chain_dir(&self) -> PathBuf {
        self.base.join(CHAIN_DIR)
    }

    fn blocks_dir(&self) -> PathBuf {
        self.chain_dir().join(BLOCKS_DIR)
    }

    fn state_dir(&self) -> PathBuf {
        self.base.join(STATE_DIR)
    }

    /// Ensure required directories exist.
    pub fn ensure_dirs(&self) -> Result<(), PersistenceError> {
        std::fs::create_dir_all(self.blocks_dir())?;
        std::fs::create_dir_all(self.state_dir())?;
        Ok(())
    }

    /// Save a single block to disk (append).
    pub fn save_block(&self, block: &Block) -> Result<(), PersistenceError> {
        self.ensure_dirs()?;
        let path = self
            .blocks_dir()
            .join(format!("{}.bin", block.header.height));
        let bytes = bincode::serialize(block)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Save execution receipts for a block height (`chain/blocks/receipts_{height}.bin`).
    pub fn save_receipts(
        &self,
        height: u64,
        receipts: &[ExecutionReceipt],
    ) -> Result<(), PersistenceError> {
        self.ensure_dirs()?;
        let path = self.blocks_dir().join(format!("receipts_{}.bin", height));
        let bytes = bincode::serialize(receipts)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Load receipts for a height if present (older nodes may have no file).
    pub fn load_receipts_for_height(
        &self,
        height: u64,
    ) -> Result<Option<Vec<ExecutionReceipt>>, PersistenceError> {
        let path = self.blocks_dir().join(format!("receipts_{}.bin", height));
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path)?;
        let receipts: Vec<ExecutionReceipt> = bincode::deserialize(&bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        Ok(Some(receipts))
    }

    /// Save chain metadata (height, latest_hash).
    pub fn save_chain_meta(&self, height: u64, latest_hash: Hash) -> Result<(), PersistenceError> {
        self.ensure_dirs()?;
        let meta = ChainMeta {
            height,
            latest_hash,
        };
        let path = self.chain_dir().join(CHAIN_META_FILE);
        let bytes = bincode::serialize(&meta)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Save full state to disk.
    pub fn save_state(&self, state: &StateStore) -> Result<(), PersistenceError> {
        self.ensure_dirs()?;
        let (accounts, contract_code, contract_storage) = state.export_for_persistence();
        let persisted = PersistedState {
            accounts,
            contract_code,
            contract_storage,
        };
        let path = self.state_dir().join(STATE_FILE);
        let bytes = bincode::serialize(&persisted)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Load all blocks from disk and build ChainState.
    pub fn load_chain(&self) -> Result<Option<ChainState>, PersistenceError> {
        let meta_path = self.chain_dir().join(CHAIN_META_FILE);
        if !meta_path.exists() {
            return Ok(None);
        }
        let meta_bytes = std::fs::read(&meta_path)?;
        let meta: ChainMeta = bincode::deserialize(&meta_bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let blocks_dir = self.blocks_dir();
        if !blocks_dir.exists() {
            return Ok(None);
        }

        let genesis_path = blocks_dir.join("0.bin");
        if !genesis_path.exists() {
            return Ok(None);
        }

        let genesis_bytes = std::fs::read(&genesis_path)?;
        let genesis: Block = bincode::deserialize(&genesis_bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let chain = ChainState::from_genesis(genesis.clone());

        for h in 1..=meta.height {
            let block_path = blocks_dir.join(format!("{}.bin", h));
            if !block_path.exists() {
                return Err(PersistenceError::Serialization(format!(
                    "Missing block {h}"
                )));
            }
            let block_bytes = std::fs::read(&block_path)?;
            let block: Block = bincode::deserialize(&block_bytes)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            chain
                .append(block)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        }

        Ok(Some(chain))
    }

    /// Load state from disk.
    pub fn load_state(&self) -> Result<Option<StateStore>, PersistenceError> {
        let path = self.state_dir().join(STATE_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        let persisted: PersistedState = bincode::deserialize(&bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let state = StateStore::load_from_persistence(
            persisted.accounts,
            persisted.contract_code,
            persisted.contract_storage,
        );
        Ok(Some(state))
    }

    /// Check if persisted data exists (we can resume).
    pub fn has_persisted_data(&self) -> bool {
        self.chain_dir().join(CHAIN_META_FILE).exists()
    }

    fn qa_registry_path(&self) -> std::path::PathBuf {
        self.base.join(QA_REGISTRY_FILE)
    }

    fn qa_pool_config_path(&self) -> std::path::PathBuf {
        self.base.join(QA_POOL_CONFIG_FILE)
    }

    /// Save QA rule registry (JSON, governance shape). Same format as `qa_registry` proposal value.
    pub fn save_qa_registry(&self, registry: &RuleRegistry) -> Result<(), PersistenceError> {
        let path = self.qa_registry_path();
        let json = serde_json::to_vec_pretty(registry)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_qa_registry(&self) -> Result<Option<RuleRegistry>, PersistenceError> {
        let path = self.qa_registry_path();
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))
    }

    /// Save QA pool governance config (JSON). Same format as `qa_pool_config` proposal value.
    pub fn save_qa_pool_config(
        &self,
        config: &QaPoolGovernanceConfig,
    ) -> Result<(), PersistenceError> {
        let path = self.qa_pool_config_path();
        let json = serde_json::to_vec_pretty(config)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_qa_pool_config(&self) -> Result<Option<QaPoolGovernanceConfig>, PersistenceError> {
        let path = self.qa_pool_config_path();
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))
    }
}
