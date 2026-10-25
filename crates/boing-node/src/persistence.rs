//! Disk persistence for chain and state.
//!
//! Persists blocks and state to the data directory so the node can resume
//! after restart without losing data.

use std::path::{Path, PathBuf};

use boing_governance::SlashRegistry;
use boing_primitives::{
    decode_bls_dual_key_registry, encode_bls_dual_key_registry, AccountId, AccountState,
    Block, BlsDualKeyRegistry, ExecutionReceipt, Hash, PHASE27_BLS_DUAL_KEY_REGISTRY_FILE,
};
use boing_qa::pool_config::QaPoolGovernanceConfig;
use boing_qa::RuleRegistry;
use boing_state::{ContractStorageEntry, QaPendingRecord, StateStore};
use boing_tokenomics::NetworkFeePolicy;

use crate::chain::ChainState;

// --- Bounded deserialization of local persistence -------------------------------------------------
// A corrupt, truncated or tampered data dir must not make the node allocate unbounded memory at
// startup. Files are size-checked *before* being read, and bincode decoding runs with a byte limit
// (same fixint / trailing-bytes semantics as `bincode::serialize`/`deserialize`, so existing files
// load unchanged).

/// Cap for small per-height / metadata files (`receipts_<h>.bin`, `meta.bin`, `qa_pending.bin`).
pub const MAX_PERSIST_SMALL_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Cap for one persisted block file.
pub const MAX_PERSIST_BLOCK_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// Cap for registry sidecars (names, open validators, sample offers, slash appeals, ...).
pub const MAX_PERSIST_REGISTRY_FILE_BYTES: u64 = 512 * 1024 * 1024;
/// Default cap for the full state snapshot `accounts.bin` (override: `BOING_PERSIST_MAX_STATE_BYTES`).
pub const DEFAULT_MAX_PERSIST_STATE_BYTES: u64 = 32 * 1024 * 1024 * 1024;

fn max_state_file_bytes() -> u64 {
    std::env::var("BOING_PERSIST_MAX_STATE_BYTES")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_MAX_PERSIST_STATE_BYTES)
}

/// Read `path`, refusing files larger than `max` bytes (checked via metadata before reading).
pub fn read_file_limited(path: &Path, max: u64) -> Result<Vec<u8>, PersistenceError> {
    let len = std::fs::metadata(path)?.len();
    if len > max {
        return Err(PersistenceError::Serialization(format!(
            "{} is {len} bytes, over the {max}-byte persistence limit",
            path.display()
        )));
    }
    Ok(std::fs::read(path)?)
}

/// Decode bincode with a hard byte limit (see module note above).
pub fn decode_bincode_limited<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    max: u64,
) -> Result<T, PersistenceError> {
    use bincode::Options;
    if bytes.len() as u64 > max {
        return Err(PersistenceError::Serialization(format!(
            "payload is {} bytes, over the {max}-byte persistence limit",
            bytes.len()
        )));
    }
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .with_limit(max)
        .deserialize(bytes)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))
}

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
const SAMPLE_OFFERS_FILE: &str = "sample_offers_registry.bin";
const SAMPLE_SERVE_FILE: &str = "sample_serve_challenges.bin";
const SLASH_APPEALS_FILE: &str = "slash_appeals_registry.bin";
/// Phase 17+ on-disk envelope magic for `slash_appeals_registry.bin`.
const SLASH_APPEALS_MAGIC: &[u8; 4] = b"BSAR";
/// Phase 17 payload version (weighted ResolveVote, no ballot).
const SLASH_APPEALS_VERSION_V2: u32 = 2;
/// Phase 18 payload version (weighted ResolveVote + optional ballot).
const SLASH_APPEALS_VERSION_V3: u32 = 3;
/// Phase 20 payload version (electorate W snapshot + pin-set commitment).
const SLASH_APPEALS_VERSION_V4: u32 = 4;
/// Phase 21 payload version (resolve_deadline + Expired status).
const SLASH_APPEALS_VERSION_V5: u32 = 5;
/// Node-local jailing snapshots (not committed to the state root).
const OPEN_VALIDATOR_JAIL_STATS_FILE: &str = "open_validator_jail_stats.bin";
const SLASH_REGISTRY_FILE: &str = "slash_registry.json";
/// Phase 27 host-local BLS dual-key registry (not slash V5 / not state-root).
const BLS_DUAL_KEY_REGISTRY_FILE: &str = PHASE27_BLS_DUAL_KEY_REGISTRY_FILE;

/// Encode `SlashAppealRegistry` as Phase 21 V5 envelope: magic + u32 version + bincode.
pub fn encode_slash_appeals_v5(
    reg: &boing_state::SlashAppealRegistry,
) -> Result<Vec<u8>, PersistenceError> {
    let payload = bincode::serialize(reg)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let mut out = Vec::with_capacity(4 + 4 + payload.len());
    out.extend_from_slice(SLASH_APPEALS_MAGIC);
    out.extend_from_slice(&SLASH_APPEALS_VERSION_V5.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Encode a raw V4-shaped payload as Phase 20 V4 envelope (tests / migration fixtures).
pub fn encode_slash_appeals_v4_fixture(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 4 + payload.len());
    out.extend_from_slice(SLASH_APPEALS_MAGIC);
    out.extend_from_slice(&SLASH_APPEALS_VERSION_V4.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Encode a raw V3-shaped payload as Phase 18 V3 envelope (tests / migration fixtures).
pub fn encode_slash_appeals_v3_fixture(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 4 + payload.len());
    out.extend_from_slice(SLASH_APPEALS_MAGIC);
    out.extend_from_slice(&SLASH_APPEALS_VERSION_V3.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Encode as Phase 17 V2 envelope (tests / migration fixtures). Prefer [`encode_slash_appeals_v5`].
pub fn encode_slash_appeals_v2_fixture(
    payload: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 4 + payload.len());
    out.extend_from_slice(SLASH_APPEALS_MAGIC);
    out.extend_from_slice(&SLASH_APPEALS_VERSION_V2.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Decode slash-appeals registry: V5 envelope, else V4 (derive resolve_deadline), else V3
/// (legacy snapshot sentinel), else V2 (ballot=None), else legacy Phase 16 bool votes
/// (weight=0), else Phase 14/15 (empty votes). Corrupt → hard error (no wipe).
pub fn decode_slash_appeals(bytes: &[u8]) -> Result<boing_state::SlashAppealRegistry, PersistenceError> {
    if bytes.len() >= 8 && &bytes[..4] == SLASH_APPEALS_MAGIC {
        let ver = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if ver == SLASH_APPEALS_VERSION_V5 {
            return bincode::deserialize(&bytes[8..])
                .map_err(|e| PersistenceError::Serialization(e.to_string()));
        }
        if ver == SLASH_APPEALS_VERSION_V4 {
            return boing_state::SlashAppealRegistry::from_legacy_phase20_v4(&bytes[8..])
                .map_err(PersistenceError::Serialization);
        }
        if ver == SLASH_APPEALS_VERSION_V3 {
            return boing_state::SlashAppealRegistry::from_legacy_phase18_v3(&bytes[8..])
                .map_err(PersistenceError::Serialization);
        }
        if ver == SLASH_APPEALS_VERSION_V2 {
            return boing_state::SlashAppealRegistry::from_legacy_phase17_v2(&bytes[8..])
                .map_err(PersistenceError::Serialization);
        }
        return Err(PersistenceError::Serialization(format!(
            "unsupported slash_appeals_registry.bin version {ver}"
        )));
    }
    // Legacy bare bincode: try Phase 16 (bool votes) then Phase 14/15 (no votes).
    if let Ok(reg) = boing_state::SlashAppealRegistry::from_legacy_phase16(bytes) {
        return Ok(reg);
    }
    if let Ok(reg) = boing_state::SlashAppealRegistry::from_legacy_phase14(bytes) {
        return Ok(reg);
    }
    Err(PersistenceError::Serialization(
        "corrupt or unsupported slash_appeals_registry.bin (not V5, V4, V3, V2, Phase 16, or Phase 14/15)"
            .into(),
    ))
}

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
        let bytes = read_file_limited(&path, MAX_PERSIST_SMALL_FILE_BYTES)?;
        let receipts: Vec<ExecutionReceipt> =
            decode_bincode_limited(&bytes, MAX_PERSIST_SMALL_FILE_BYTES)?;
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
        let sample_offers_path = self.state_dir().join(SAMPLE_OFFERS_FILE);
        if state.sample_offers().is_empty() {
            if sample_offers_path.exists() {
                std::fs::remove_file(&sample_offers_path)?;
            }
        } else {
            let bytes = bincode::serialize(state.sample_offers())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            let tmp = sample_offers_path.with_extension("bin.tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(tmp, &sample_offers_path)?;
        }
        let sample_serve_path = self.state_dir().join(SAMPLE_SERVE_FILE);
        if state.sample_serve().is_empty() {
            if sample_serve_path.exists() {
                std::fs::remove_file(&sample_serve_path)?;
            }
        } else {
            let bytes = bincode::serialize(state.sample_serve())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            let tmp = sample_serve_path.with_extension("bin.tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(tmp, &sample_serve_path)?;
        }
        let slash_appeals_path = self.state_dir().join(SLASH_APPEALS_FILE);
        if state.slash_appeals().is_empty() {
            if slash_appeals_path.exists() {
                std::fs::remove_file(&slash_appeals_path)?;
            }
        } else {
            let bytes = encode_slash_appeals_v5(state.slash_appeals())?;
            let tmp = slash_appeals_path.with_extension("bin.tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(tmp, &slash_appeals_path)?;
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
        let meta_bytes = read_file_limited(&meta_path, MAX_PERSIST_SMALL_FILE_BYTES)?;
        let meta: ChainMeta = decode_bincode_limited(&meta_bytes, MAX_PERSIST_SMALL_FILE_BYTES)?;

        let blocks_dir = self.blocks_dir();
        if !blocks_dir.exists() {
            return Ok(None);
        }

        let genesis_path = blocks_dir.join("0.bin");
        if !genesis_path.exists() {
            return Ok(None);
        }

        let genesis_bytes = read_file_limited(&genesis_path, MAX_PERSIST_BLOCK_FILE_BYTES)?;
        let genesis: Block = Block::from_bincode(&genesis_bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let chain = ChainState::from_genesis(genesis.clone());

        for h in 1..=meta.height {
            let block_path = blocks_dir.join(format!("{}.bin", h));
            if !block_path.exists() {
                return Err(PersistenceError::Serialization(format!(
                    "Missing block {h}"
                )));
            }
            let block_bytes = read_file_limited(&block_path, MAX_PERSIST_BLOCK_FILE_BYTES)?;
            let block: Block = Block::from_bincode(&block_bytes)
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
        let max_state = max_state_file_bytes();
        let bytes = read_file_limited(&path, max_state)?;
        let persisted: PersistedState = decode_bincode_limited(&bytes, max_state)?;

        let mut state = StateStore::load_from_persistence(
            persisted.accounts,
            persisted.contract_code,
            persisted.contract_storage,
        );
        let ov_path = self.state_dir().join(OPEN_VALIDATORS_FILE);
        if ov_path.exists() {
            let ov_bytes = read_file_limited(&ov_path, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            let mut reg: boing_state::OpenValidatorRegistry =
                decode_bincode_limited(&ov_bytes, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            let js_path = self.state_dir().join(OPEN_VALIDATOR_JAIL_STATS_FILE);
            if js_path.exists() {
                // Optional, node-local: a missing/corrupt file only loses the display snapshot.
                if let Ok(b) = read_file_limited(&js_path, MAX_PERSIST_SMALL_FILE_BYTES) {
                    reg.last_jail =
                        decode_bincode_limited(&b, MAX_PERSIST_SMALL_FILE_BYTES).unwrap_or_default();
                }
            }
            state.load_open_validators(reg);
        }
        let names_path = self.state_dir().join(NAMES_FILE);
        if names_path.exists() {
            let bytes = read_file_limited(&names_path, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            let reg: boing_state::NameRegistry =
                decode_bincode_limited(&bytes, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            state.load_names(reg);
        }
        let sample_offers_path = self.state_dir().join(SAMPLE_OFFERS_FILE);
        if sample_offers_path.exists() {
            let bytes = read_file_limited(&sample_offers_path, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            let reg: boing_state::SampleOfferRegistry =
                decode_bincode_limited(&bytes, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            state.load_sample_offers(reg);
        }
        let sample_serve_path = self.state_dir().join(SAMPLE_SERVE_FILE);
        if sample_serve_path.exists() {
            let bytes = read_file_limited(&sample_serve_path, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            let reg: boing_state::SampleServeChallengeRegistry =
                decode_bincode_limited(&bytes, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            state.load_sample_serve(reg);
        }
        let slash_appeals_path = self.state_dir().join(SLASH_APPEALS_FILE);
        if slash_appeals_path.exists() {
            let bytes = read_file_limited(&slash_appeals_path, MAX_PERSIST_REGISTRY_FILE_BYTES)?;
            let reg = decode_slash_appeals(&bytes)?;
            state.load_slash_appeals(reg);
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
        let bytes = read_file_limited(&path, MAX_PERSIST_SMALL_FILE_BYTES)?;
        let items: Vec<(Hash, QaPendingRecord)> =
            decode_bincode_limited(&bytes, MAX_PERSIST_SMALL_FILE_BYTES)?;
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

    fn bls_dual_key_registry_path(&self) -> std::path::PathBuf {
        self.base.join(BLS_DUAL_KEY_REGISTRY_FILE)
    }

    /// Save Phase 27 host-local BLS dual-key registry (magic + version + bincode).
    pub fn save_bls_dual_key_registry(
        &self,
        registry: &BlsDualKeyRegistry,
    ) -> Result<(), PersistenceError> {
        let path = self.bls_dual_key_registry_path();
        let bytes = encode_bls_dual_key_registry(registry)
            .map_err(PersistenceError::Serialization)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn load_bls_dual_key_registry(
        &self,
    ) -> Result<Option<BlsDualKeyRegistry>, PersistenceError> {
        let path = self.bls_dual_key_registry_path();
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        decode_bls_dual_key_registry(&bytes)
            .map(Some)
            .map_err(PersistenceError::Serialization)
    }
}

#[cfg(test)]
mod slash_appeals_envelope_tests {
    use super::*;
    use boing_primitives::{AccountId, PHASE14_MAX_APPEAL_EVIDENCE_BYTES};
    use boing_state::{ConsensusAppealStatus, ConsensusSlashReason, SlashAppealRegistry};
    use serde::Serialize;
    use std::collections::BTreeMap;

    fn acct(b: u8) -> AccountId {
        AccountId([b; 32])
    }

    #[test]
    fn v5_roundtrip() {
        let mut reg = SlashAppealRegistry::new();
        reg.record_slash_with_id(
            1,
            acct(1),
            100,
            ConsensusSlashReason::BlobFailedServe,
            1,
            100,
        )
        .unwrap();
        let aid = reg
            .submit_appeal(
                1,
                b"ev".to_vec(),
                1,
                PHASE14_MAX_APPEAL_EVIDENCE_BYTES,
                300,
                [0xABu8; 32],
            )
            .unwrap();
        let sig = boing_primitives::SlashAppealBallotSignature([0x42u8; 64]);
        reg.cast_resolve_vote(aid, acct(2), true, 50, 100, Some(sig))
            .unwrap();
        let bytes = encode_slash_appeals_v5(&reg).unwrap();
        assert_eq!(&bytes[..4], b"BSAR");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 5);
        let back = decode_slash_appeals(&bytes).unwrap();
        assert_eq!(back.get_appeal(aid).unwrap().votes.len(), 1);
        let v = back.get_appeal(aid).unwrap().votes.get(&acct(2)).unwrap();
        assert_eq!(v.weight, 50);
        assert_eq!(v.ballot_signature, Some(sig));
        assert_eq!(back.get_appeal(aid).unwrap().electorate_w_snapshot, 300);
        assert_eq!(back.get_appeal(aid).unwrap().pin_set_commitment, [0xABu8; 32]);
        assert_eq!(
            back.get_appeal(aid).unwrap().resolve_deadline,
            boing_primitives::phase21_resolve_deadline(1)
        );
    }

    #[test]
    fn load_v4_migrates_resolve_deadline_from_slash() {
        #[derive(Serialize)]
        struct V4Vote {
            approved: bool,
            weight: u128,
            ballot_signature: Option<boing_primitives::SlashAppealBallotSignature>,
        }
        #[derive(Serialize)]
        struct V4Appeal {
            id: u64,
            slash_id: u64,
            evidence: Vec<u8>,
            status: ConsensusAppealStatus,
            votes: BTreeMap<AccountId, V4Vote>,
            electorate_w_snapshot: u128,
            pin_set_commitment: [u8; 32],
        }
        #[derive(Serialize)]
        struct V4Reg {
            slashes: BTreeMap<u64, boing_state::ConsensusSlashRecord>,
            appeals: BTreeMap<u64, V4Appeal>,
            next_appeal_id: u64,
        }
        let mut slashes = BTreeMap::new();
        slashes.insert(
            1,
            boing_state::ConsensusSlashRecord {
                id: 1,
                account: acct(1),
                amount: 100,
                reason: ConsensusSlashReason::BlobFailedServe,
                block_height: 1,
                appeal_deadline: 555,
                reversed: false,
            },
        );
        let mut votes = BTreeMap::new();
        votes.insert(
            acct(2),
            V4Vote {
                approved: true,
                weight: 50,
                ballot_signature: None,
            },
        );
        let mut appeals = BTreeMap::new();
        appeals.insert(
            1,
            V4Appeal {
                id: 1,
                slash_id: 1,
                evidence: b"ev".to_vec(),
                status: ConsensusAppealStatus::Pending,
                votes,
                electorate_w_snapshot: 300,
                pin_set_commitment: [0xCDu8; 32],
            },
        );
        let payload = bincode::serialize(&V4Reg {
            slashes,
            appeals,
            next_appeal_id: 2,
        })
        .unwrap();
        let bytes = encode_slash_appeals_v4_fixture(&payload);
        let reg = decode_slash_appeals(&bytes).unwrap();
        let a = reg.get_appeal(1).unwrap();
        assert_eq!(a.resolve_deadline, 555);
        assert_eq!(a.electorate_w_snapshot, 300);
        assert_eq!(a.pin_set_commitment, [0xCDu8; 32]);
        // Next save is V5.
        let saved = encode_slash_appeals_v5(&reg).unwrap();
        assert_eq!(u32::from_le_bytes(saved[4..8].try_into().unwrap()), 5);
    }

    #[test]
    fn load_v3_migrates_legacy_snapshot_sentinel() {
        #[derive(Serialize)]
        struct V3Vote {
            approved: bool,
            weight: u128,
            ballot_signature: Option<boing_primitives::SlashAppealBallotSignature>,
        }
        #[derive(Serialize)]
        struct V3Appeal {
            id: u64,
            slash_id: u64,
            evidence: Vec<u8>,
            status: ConsensusAppealStatus,
            votes: BTreeMap<AccountId, V3Vote>,
        }
        #[derive(Serialize)]
        struct V3Reg {
            slashes: BTreeMap<u64, boing_state::ConsensusSlashRecord>,
            appeals: BTreeMap<u64, V3Appeal>,
            next_appeal_id: u64,
        }
        let mut slashes = BTreeMap::new();
        slashes.insert(
            1,
            boing_state::ConsensusSlashRecord {
                id: 1,
                account: acct(1),
                amount: 100,
                reason: ConsensusSlashReason::BlobFailedServe,
                block_height: 1,
                appeal_deadline: 100,
                reversed: false,
            },
        );
        let mut votes = BTreeMap::new();
        votes.insert(
            acct(2),
            V3Vote {
                approved: true,
                weight: 50,
                ballot_signature: None,
            },
        );
        let mut appeals = BTreeMap::new();
        appeals.insert(
            1,
            V3Appeal {
                id: 1,
                slash_id: 1,
                evidence: b"ev".to_vec(),
                status: ConsensusAppealStatus::Pending,
                votes,
            },
        );
        let payload = bincode::serialize(&V3Reg {
            slashes,
            appeals,
            next_appeal_id: 2,
        })
        .unwrap();
        let bytes = encode_slash_appeals_v3_fixture(&payload);
        let reg = decode_slash_appeals(&bytes).unwrap();
        let a = reg.get_appeal(1).unwrap();
        assert_eq!(
            a.electorate_w_snapshot,
            boing_primitives::PHASE20_LEGACY_ELECTORATE_W_SENTINEL
        );
        assert_eq!(
            a.pin_set_commitment,
            boing_primitives::PHASE20_LEGACY_PIN_SET_COMMITMENT
        );
        assert_eq!(a.votes.get(&acct(2)).unwrap().weight, 50);
        assert_eq!(a.resolve_deadline, 100);
        // Next save is V5.
        let saved = encode_slash_appeals_v5(&reg).unwrap();
        assert_eq!(u32::from_le_bytes(saved[4..8].try_into().unwrap()), 5);
    }

    #[test]
    fn load_v2_migrates_ballot_none() {
        #[derive(Serialize)]
        struct V2Vote {
            approved: bool,
            weight: u128,
        }
        #[derive(Serialize)]
        struct V2Appeal {
            id: u64,
            slash_id: u64,
            evidence: Vec<u8>,
            status: ConsensusAppealStatus,
            votes: BTreeMap<AccountId, V2Vote>,
        }
        #[derive(Serialize)]
        struct V2Reg {
            slashes: BTreeMap<u64, boing_state::ConsensusSlashRecord>,
            appeals: BTreeMap<u64, V2Appeal>,
            next_appeal_id: u64,
        }
        let mut slashes = BTreeMap::new();
        slashes.insert(
            1,
            boing_state::ConsensusSlashRecord {
                id: 1,
                account: acct(1),
                amount: 100,
                reason: ConsensusSlashReason::BlobFailedServe,
                block_height: 1,
                appeal_deadline: 100,
                reversed: false,
            },
        );
        let mut votes = BTreeMap::new();
        votes.insert(
            acct(2),
            V2Vote {
                approved: true,
                weight: 50,
            },
        );
        let mut appeals = BTreeMap::new();
        appeals.insert(
            1,
            V2Appeal {
                id: 1,
                slash_id: 1,
                evidence: b"ev".to_vec(),
                status: ConsensusAppealStatus::Pending,
                votes,
            },
        );
        let payload = bincode::serialize(&V2Reg {
            slashes,
            appeals,
            next_appeal_id: 2,
        })
        .unwrap();
        let bytes = encode_slash_appeals_v2_fixture(&payload);
        let reg = decode_slash_appeals(&bytes).unwrap();
        let v = reg.get_appeal(1).unwrap().votes.get(&acct(2)).unwrap();
        assert!(v.approved);
        assert_eq!(v.weight, 50);
        assert!(v.ballot_signature.is_none());
    }

    #[test]
    fn load_legacy_phase16_bool_votes() {
        #[derive(Serialize)]
        struct P16Appeal {
            id: u64,
            slash_id: u64,
            evidence: Vec<u8>,
            status: ConsensusAppealStatus,
            votes: BTreeMap<AccountId, bool>,
        }
        #[derive(Serialize)]
        struct P16Reg {
            slashes: BTreeMap<u64, boing_state::ConsensusSlashRecord>,
            appeals: BTreeMap<u64, P16Appeal>,
            next_appeal_id: u64,
        }
        let mut slashes = BTreeMap::new();
        slashes.insert(
            1,
            boing_state::ConsensusSlashRecord {
                id: 1,
                account: acct(1),
                amount: 10,
                reason: ConsensusSlashReason::BlobFailedServe,
                block_height: 1,
                appeal_deadline: 100,
                reversed: false,
            },
        );
        let mut votes = BTreeMap::new();
        votes.insert(acct(9), true);
        let mut appeals = BTreeMap::new();
        appeals.insert(
            1,
            P16Appeal {
                id: 1,
                slash_id: 1,
                evidence: vec![],
                status: ConsensusAppealStatus::Pending,
                votes,
            },
        );
        let legacy = bincode::serialize(&P16Reg {
            slashes,
            appeals,
            next_appeal_id: 2,
        })
        .unwrap();
        let reg = decode_slash_appeals(&legacy).unwrap();
        let v = reg.get_appeal(1).unwrap().votes.get(&acct(9)).unwrap();
        assert!(v.approved);
        assert_eq!(v.weight, 0);
    }

    #[test]
    fn load_legacy_phase14_no_votes() {
        #[derive(Serialize)]
        struct P14Appeal {
            id: u64,
            slash_id: u64,
            evidence: Vec<u8>,
            status: ConsensusAppealStatus,
        }
        #[derive(Serialize)]
        struct P14Reg {
            slashes: BTreeMap<u64, boing_state::ConsensusSlashRecord>,
            appeals: BTreeMap<u64, P14Appeal>,
            next_appeal_id: u64,
        }
        let mut slashes = BTreeMap::new();
        slashes.insert(
            2,
            boing_state::ConsensusSlashRecord {
                id: 2,
                account: acct(2),
                amount: 5,
                reason: ConsensusSlashReason::Equivocation,
                block_height: 1,
                appeal_deadline: 50,
                reversed: false,
            },
        );
        let mut appeals = BTreeMap::new();
        appeals.insert(
            1,
            P14Appeal {
                id: 1,
                slash_id: 2,
                evidence: b"x".to_vec(),
                status: ConsensusAppealStatus::Pending,
            },
        );
        let legacy = bincode::serialize(&P14Reg {
            slashes,
            appeals,
            next_appeal_id: 2,
        })
        .unwrap();
        let reg = decode_slash_appeals(&legacy).unwrap();
        assert!(reg.get_appeal(1).unwrap().votes.is_empty());
    }

    #[test]
    fn corrupt_file_errors_no_wipe() {
        let err = decode_slash_appeals(b"not-a-registry").unwrap_err();
        assert!(matches!(err, PersistenceError::Serialization(_)));
    }
}

#[cfg(test)]
mod bounded_decode_tests {
    use super::*;

    #[test]
    fn limited_decode_matches_plain_bincode_and_rejects_oversize() {
        let v: Vec<(u64, String)> = vec![(1, "a".into()), (2, "bb".into())];
        let bytes = bincode::serialize(&v).unwrap();
        let plain: Vec<(u64, String)> = bincode::deserialize(&bytes).unwrap();
        let lim: Vec<(u64, String)> = decode_bincode_limited(&bytes, 1024).unwrap();
        assert_eq!(plain, lim);
        // trailing bytes still tolerated like plain bincode
        let mut with_tail = bytes.clone();
        with_tail.extend_from_slice(&[0, 1, 2]);
        assert!(decode_bincode_limited::<Vec<(u64, String)>>(&with_tail, 1024).is_ok());
        // over the cap
        assert!(decode_bincode_limited::<Vec<(u64, String)>>(&bytes, 4).is_err());
    }

    #[test]
    fn lying_length_prefix_is_rejected_without_huge_allocation() {
        // Vec<u8> claiming 4 GiB of payload but only a few bytes present.
        let mut bytes = (4u64 * 1024 * 1024 * 1024).to_le_bytes().to_vec();
        bytes.extend_from_slice(&[1, 2, 3]);
        assert!(decode_bincode_limited::<Vec<u8>>(&bytes, 1024 * 1024).is_err());
    }

    #[test]
    fn read_file_limited_checks_size_first() {
        let p = std::env::temp_dir().join(format!("boing-persist-limit-{}.bin", std::process::id()));
        std::fs::write(&p, vec![7u8; 2048]).unwrap();
        assert_eq!(read_file_limited(&p, 4096).unwrap().len(), 2048);
        let err = read_file_limited(&p, 1024).unwrap_err();
        assert!(err.to_string().contains("persistence limit"), "{err}");
        let _ = std::fs::remove_file(p);
    }
}