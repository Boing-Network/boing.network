//! Disk persistence for chain and state.
//!
//! Persists blocks and state to the data directory so the node can resume
//! after restart without losing data.

use std::path::{Path, PathBuf};

use boing_governance::SlashRegistry;
use boing_primitives::{AccountId, AccountState, Block, ExecutionReceipt, Hash};
use boing_qa::pool_config::QaPoolGovernanceConfig;
use boing_qa::RuleRegistry;
use boing_state::{ContractStorageEntry, QaPendingRecord, StateStore};
use boing_tokenomics::NetworkFeePolicy;

use crate::chain::ChainState;

const CHAIN_DIR: &str = "chain";
const BLOCKS_DIR: &str = "blocks";
const CHAIN_META_FILE: &str = "meta.bin";
const STATE_DIR: &str = "state";
const STATE_FILE: &str = "accounts.bin";
const QA_REGISTRY_FILE: &str = "qa_registry.json";
const QA_POOL_CONFIG_FILE: &str = "qa_pool_config.json";
const NETWORK_FEE_CONFIG_FILE: &str = "network_fee_config.json";
const QA_PENDING_FILE: &str = "qa_pending.bin";
const OPEN_VALIDATORS_FILE: &str = "open_validators.bin";
const NAMES_FILE: &str = "names.bin";
/// Node-local jailing snapshots (not committed to the state root).
const OPEN_VALIDATOR_JAIL_STATS_FILE: &str = "open_validator_jail_stats.bin";
const SLASH_REGISTRY_FILE: &str = "slash_registry.json";

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

    /// Write height-0 genesis if `chain/blocks/0.bin` is missing.
    ///
    /// Production nodes used to persist only produced/imported blocks (height ≥ 1).
    /// [`Self::load_chain`] requires `0.bin`; without this backfill a restart silently
    /// starts a new chain while later blocks remain on disk.
    pub fn save_genesis_if_missing(&self, genesis: &Block) -> Result<bool, PersistenceError> {
        if genesis.header.height != 0 {
            return Err(PersistenceError::Serialization(
                "save_genesis_if_missing requires a height-0 block".into(),
            ));
        }
        self.ensure_dirs()?;
        let path = self.blocks_dir().join("0.bin");
        if path.exists() {
            return Ok(false);
        }
        self.save_block(genesis)?;
        Ok(true)
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
        let pending_path = self.state_dir().join(QA_PENDING_FILE);
        let pending_bytes = bincode::serialize(&state.export_qa_pending())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(pending_path, pending_bytes)?;
        let ov_path = self.state_dir().join(OPEN_VALIDATORS_FILE);
        if state.open_validators().is_empty() {
            if ov_path.exists() {
                std::fs::remove_file(&ov_path)?;
            }
        } else {
            let ov_bytes = bincode::serialize(state.open_validators())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            let tmp = ov_path.with_extension("bin.tmp");
            std::fs::write(&tmp, ov_bytes)?;
            std::fs::rename(tmp, &ov_path)?;
        }
        let names_path = self.state_dir().join(NAMES_FILE);
        if state.names().is_empty() {
            if names_path.exists() {
                std::fs::remove_file(&names_path)?;
            }
        } else {
            let bytes = bincode::serialize(state.names())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            let tmp = names_path.with_extension("bin.tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(tmp, &names_path)?;
        }
        let js_path = self.state_dir().join(OPEN_VALIDATOR_JAIL_STATS_FILE);
        if state.open_validators().last_jail.is_empty() {
            if js_path.exists() {
                std::fs::remove_file(&js_path)?;
            }
        } else {
            let bytes = bincode::serialize(&state.open_validators().last_jail)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            let tmp = js_path.with_extension("bin.tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(tmp, &js_path)?;
        }
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

        let mut state = StateStore::load_from_persistence(
            persisted.accounts,
            persisted.contract_code,
            persisted.contract_storage,
        );
        let ov_path = self.state_dir().join(OPEN_VALIDATORS_FILE);
        if ov_path.exists() {
            let ov_bytes = std::fs::read(&ov_path)?;
            let mut reg: boing_state::OpenValidatorRegistry = bincode::deserialize(&ov_bytes)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            let js_path = self.state_dir().join(OPEN_VALIDATOR_JAIL_STATS_FILE);
            if js_path.exists() {
                // Optional, node-local: a missing/corrupt file only loses the display snapshot.
                if let Ok(m) = std::fs::read(&js_path).map(|b| bincode::deserialize(&b)) {
                    reg.last_jail = m.unwrap_or_default();
                }
            }
            state.load_open_validators(reg);
        }
        let names_path = self.state_dir().join(NAMES_FILE);
        if names_path.exists() {
            let bytes = std::fs::read(&names_path)?;
            let reg: boing_state::NameRegistry = bincode::deserialize(&bytes)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            state.load_names(reg);
        }
        Ok(Some(state))
    }

    fn stake_validator_set_path(&self) -> PathBuf {
        self.chain_dir().join("stake_validator_set.json")
    }

    /// Persist the stake-derived validator set chosen at epoch boundary `height`.
    pub fn save_stake_validator_set(
        &self,
        height: u64,
        set: &[AccountId],
    ) -> Result<(), PersistenceError> {
        self.ensure_dirs()?;
        let v = serde_json::json!({
            "height": height,
            "validators": set.iter().map(|a| hex::encode(a.0)).collect::<Vec<_>>(),
        });
        let bytes = serde_json::to_vec_pretty(&v)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        let tmp = self.stake_validator_set_path().with_extension("json.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(tmp, self.stake_validator_set_path())?;
        Ok(())
    }

    /// Load `(boundary_height, validators)` saved by [`Self::save_stake_validator_set`].
    pub fn load_stake_validator_set(
        &self,
    ) -> Result<Option<(u64, Vec<AccountId>)>, PersistenceError> {
        let path = self.stake_validator_set_path();
        if !path.exists() {
            return Ok(None);
        }
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        let bad = || PersistenceError::Serialization("bad stake_validator_set.json".into());
        let height = v
            .get("height")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(bad)?;
        let mut out = Vec::new();
        for s in v
            .get("validators")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(bad)?
        {
            let b = hex::decode(s.as_str().ok_or_else(bad)?).map_err(|_| bad())?;
            let arr: [u8; 32] = b.try_into().map_err(|_| bad())?;
            out.push(AccountId(arr));
        }
        Ok(Some((height, out)))
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

    fn network_fee_config_path(&self) -> std::path::PathBuf {
        self.base.join(NETWORK_FEE_CONFIG_FILE)
    }

    pub fn save_network_fee_config(
        &self,
        policy: &NetworkFeePolicy,
    ) -> Result<(), PersistenceError> {
        let json = serde_json::to_vec_pretty(policy)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(self.network_fee_config_path(), json)?;
        Ok(())
    }

    pub fn load_network_fee_config(&self) -> Result<Option<NetworkFeePolicy>, PersistenceError> {
        let path = self.network_fee_config_path();
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))
    }

    pub fn load_qa_pending(
        &self,
    ) -> Result<Option<Vec<(Hash, QaPendingRecord)>>, PersistenceError> {
        let path = self.state_dir().join(QA_PENDING_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path)?;
        let items: Vec<(Hash, QaPendingRecord)> = bincode::deserialize(&bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        Ok(Some(items))
    }

    fn slash_registry_path(&self) -> std::path::PathBuf {
        self.base.join(SLASH_REGISTRY_FILE)
    }

    /// Save slash / appeal registry (JSON).
    pub fn save_slash_registry(&self, registry: &SlashRegistry) -> Result<(), PersistenceError> {
        let path = self.slash_registry_path();
        let json = serde_json::to_vec_pretty(registry)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_slash_registry(&self) -> Result<Option<SlashRegistry>, PersistenceError> {
        let path = self.slash_registry_path();
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))
    }
}
