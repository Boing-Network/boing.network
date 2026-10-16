//! Network presets: the genesis and historical execution rules a node needs to replay an existing
//! network from genesis with the current binary.
//!
//! The public testnet (chain 6913) predates some rule changes. Its blocks must keep their committed
//! hashes and state roots, so instead of rewriting history the node runs height-gated rule *eras*
//! (see [`boing_execution::ExecutionHistory`]) and uses the network's genesis allocation, whatever
//! the local validator key is. Other chains (local dev, mainnet) have no preset and use the current
//! rules from genesis.

use boing_execution::{ExecutionHistory, FeeEra};
use boing_primitives::AccountId;
use boing_tokenomics::{FeeRemainder, NetworkFeePolicy};

use crate::faucet;
use crate::node::BoingNode;

/// Public Boing testnet chain id.
pub const BOING_TESTNET_CHAIN_ID: u64 = 6913;
/// Genesis proposer / bootstrap anchor of the public testnet.
pub const BOING_TESTNET_GENESIS_ACCOUNT: AccountId = AccountId([1u8; 32]);
/// Genesis balance of [`BOING_TESTNET_GENESIS_ACCOUNT`].
pub const BOING_TESTNET_GENESIS_BALANCE: u128 = 1_000_000;
/// First testnet height executed with treasury-first fees (heights 1..=26 used the legacy split).
pub const BOING_TESTNET_TREASURY_FEES_FROM: u64 = 27;
/// Testnet activation height of security fixes #2 (v2 chain-id signatures required for admission)
/// and #3 (failed txs pay the failed-tx fee). Below it the network runs the historical rules, so
/// replay from genesis stays exact.
pub const BOING_TESTNET_SECURITY_V2_FROM: u64 = 900;
/// Testnet activation height of security fixes #16 (blocks carry tx signatures and importers verify
/// them) and #14 (chain-bound validator registration proof). Below it history replays unchanged.
pub const BOING_TESTNET_SECURITY_V3_FROM: u64 = 920;

/// Genesis block proposer plus the accounts funded at height 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenesisSpec {
    pub proposer: AccountId,
    pub allocations: Vec<(AccountId, u128)>,
}

/// Everything a node needs to follow a known network from genesis.
#[derive(Clone, Debug)]
pub struct NetworkPreset {
    pub chain_id: u64,
    /// Bootstrap validators (open-mode anchors) used when no validator list is configured.
    pub anchors: Vec<AccountId>,
    pub genesis: GenesisSpec,
    pub history: ExecutionHistory,
}

/// Historical execution eras of the public testnet, found by replaying every committed block
/// (`tests/testnet_replay.rs`):
///
/// * heights 1..=26: legacy 70/20/10 fee split (proposer / treasury / burn sink) with the integer
///   remainder credited to the proposer;
/// * heights 27..: current rules (100% of fees to the protocol treasury).
///
/// Every other rule change since genesis (parallel-transfer safety check, batch txs, open validator
/// set at 400, names at 640) already replays identically with the current code.
pub fn boing_testnet_history() -> ExecutionHistory {
    ExecutionHistory {
        chain_id: BOING_TESTNET_CHAIN_ID,
        fee_eras: vec![FeeEra {
            from_height: 0,
            policy: NetworkFeePolicy::legacy_proposer_split(),
            remainder: FeeRemainder::Validators,
        }],
        current_from: BOING_TESTNET_TREASURY_FEES_FROM,
        failed_tx_fee_from: BOING_TESTNET_SECURITY_V2_FROM,
        tx_v2_from: BOING_TESTNET_SECURITY_V2_FROM,
        block_sigs_from: BOING_TESTNET_SECURITY_V3_FROM,
        registration_proof_v2_from: BOING_TESTNET_SECURITY_V3_FROM,
    }
}

/// Chain id from `BOING_CHAIN_ID` (None when unset: local dev, legacy signatures only).
pub fn configured_chain_id() -> Option<u64> {
    std::env::var("BOING_CHAIN_ID").ok()?.trim().parse().ok()
}

/// First height at which v2 (chain-id) signatures are required: `BOING_TX_V2_FROM`, else the
/// network history (testnet: [`BOING_TESTNET_SECURITY_V2_FROM`]), else 0 (new chains / mainnet:
/// from genesis).
pub fn tx_v2_from(history: Option<&ExecutionHistory>) -> u64 {
    if let Some(h) = std::env::var("BOING_TX_V2_FROM").ok().and_then(|s| s.trim().parse().ok()) {
        return h;
    }
    history.map(|h| h.tx_v2_from).unwrap_or(0)
}

/// First height whose blocks must carry verified tx signatures: `BOING_BLOCK_SIGS_FROM`, else the
/// network history (testnet: [`BOING_TESTNET_SECURITY_V3_FROM`]), else 0 (new chains: genesis).
pub fn block_sigs_from(history: Option<&ExecutionHistory>) -> u64 {
    if let Some(h) = std::env::var("BOING_BLOCK_SIGS_FROM").ok().and_then(|s| s.trim().parse().ok()) {
        return h;
    }
    history.map(|h| h.block_sigs_from).unwrap_or(0)
}

/// Chain id used for signature checks: the installed network history's, else `BOING_CHAIN_ID`.
pub fn effective_chain_id(history: Option<&ExecutionHistory>) -> Option<u64> {
    history.map(|h| h.chain_id).filter(|c| *c != 0).or_else(configured_chain_id)
}

/// Signature policy for a tx that would land in block `next_height`.
pub fn tx_sig_policy(
    chain_id: Option<u64>,
    history: Option<&ExecutionHistory>,
    next_height: u64,
) -> boing_primitives::TxSigPolicy {
    match chain_id {
        None => boing_primitives::TxSigPolicy::default(),
        Some(c) => boing_primitives::TxSigPolicy {
            chain_id: Some(c),
            require_v2: next_height >= tx_v2_from(history),
        },
    }
}

/// Public testnet genesis: the bootstrap anchor (1,000,000 BOING) and the deterministic testnet
/// faucet account (10,000,000 BOING; the first blocks spend from it).
pub fn boing_testnet_genesis() -> GenesisSpec {
    GenesisSpec {
        proposer: BOING_TESTNET_GENESIS_ACCOUNT,
        allocations: vec![
            (BOING_TESTNET_GENESIS_ACCOUNT, BOING_TESTNET_GENESIS_BALANCE),
            (faucet::testnet_faucet_account_id(), faucet::FAUCET_INITIAL_BALANCE),
        ],
    }
}

pub fn boing_testnet_preset() -> NetworkPreset {
    NetworkPreset {
        chain_id: BOING_TESTNET_CHAIN_ID,
        anchors: vec![BOING_TESTNET_GENESIS_ACCOUNT],
        genesis: boing_testnet_genesis(),
        history: boing_testnet_history(),
    }
}

/// Preset for `chain_id`, if it is a known network.
pub fn preset_for_chain(chain_id: u64) -> Option<NetworkPreset> {
    (chain_id == BOING_TESTNET_CHAIN_ID).then(boing_testnet_preset)
}

/// Resolve from env: `BOING_CHAIN_ID` selects the preset; `BOING_NETWORK_PRESET=none` disables it
/// (e.g. a private chain that reuses chain id 6913).
pub fn preset_from_env() -> Option<NetworkPreset> {
    if matches!(
        std::env::var("BOING_NETWORK_PRESET").ok().as_deref().map(str::trim),
        Some("none") | Some("off") | Some("0")
    ) {
        return None;
    }
    let chain_id: u64 = std::env::var("BOING_CHAIN_ID").ok()?.trim().parse().ok()?;
    preset_for_chain(chain_id)
}

/// Apply a preset to a freshly constructed node: installs the historical execution eras and, if the
/// node is still at height 0, the network genesis (block 0 + allocations). Never touches a node that
/// already has blocks beyond genesis except for the (replay-only) execution eras.
/// Returns true when the genesis was (re)written.
pub fn apply_preset(node: &mut BoingNode, preset: &NetworkPreset) -> bool {
    node.executor.set_execution_history(Some(preset.history.clone()));
    node.apply_genesis_spec(&preset.genesis)
}
