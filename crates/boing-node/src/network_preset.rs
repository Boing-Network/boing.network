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
/// Boing mainnet chain id (no preset ships for it; operators configure it via `BOING_CHAIN_ID`).
pub const BOING_MAINNET_CHAIN_ID: u64 = 6914;
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
/// Testnet activation height of the 2026-10-17 security-review fixes #6 (sender-keyed name
/// commitments), #9 (no parallel transfer batch touching the fee recipient), #10 (open-validator
/// registry cap) and #11 (a jailed validator cannot exit before its jail ends). Chosen with a wide
/// margin above the tip at deploy time (~941; blocks are produced on demand).
pub const BOING_TESTNET_SECURITY_V4_FROM: u64 = 1_000;
/// Testnet height at which the native ledger switches from whole BOING to base units (18 decimals,
/// approved 2026-10-17): at the start of this block every balance, stake, pending unbond and validator
/// bond is multiplied by 10^18 once; from it fees use `GAS_PRICE_BASE_UNITS` and whole-BOING prices
/// (bond, name rent, faucet) are scaled. Chosen 50 blocks after the v4 gate, ~108 above the tip at
/// deploy time.
pub const BOING_TESTNET_BASE_UNITS_FROM: u64 = 1_050;
/// Testnet activation height of the native-value rules (approved 2026-10-17): contract calls may carry
/// native BOING (`ContractCallWithValue`, batch value steps) and contracts can read `CALLVALUE`,
/// `SELFBALANCE` / `BALANCE` and pay out with `NATIVE_TRANSFER`. Chosen ~80 blocks above the tip at
/// deploy time (~1067; blocks are produced on demand). Validators must run testnet-v0.1.18 before it.
pub const BOING_TESTNET_NATIVE_VALUE_FROM: u64 = 1_150;

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
        security_v4_from: BOING_TESTNET_SECURITY_V4_FROM,
        base_units_from: BOING_TESTNET_BASE_UNITS_FROM,
        native_value_from: BOING_TESTNET_NATIVE_VALUE_FROM,
        // Honor reduced execution_gas_fee_bps only after operators opt in via env/CLI
        // (`BOING_MICRO_FEES_ACTIVATION_HEIGHT`); until then keep full historical gas for replay.
        // `u64::MAX` = never via history alone (policy BPS ignored for reduction until env sets a height,
        // or a future preset pins a concrete height). Overridden by env when set.
        micro_fees_from: u64::MAX,
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
    if let Some(h) = std::env::var("BOING_TX_V2_FROM")
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        return h;
    }
    history.map(|h| h.tx_v2_from).unwrap_or(0)
}

/// First height whose blocks must carry verified tx signatures: `BOING_BLOCK_SIGS_FROM`, else the
/// network history (testnet: [`BOING_TESTNET_SECURITY_V3_FROM`]), else 0 (new chains: genesis).
pub fn block_sigs_from(history: Option<&ExecutionHistory>) -> u64 {
    if let Some(h) = std::env::var("BOING_BLOCK_SIGS_FROM")
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        return h;
    }
    history.map(|h| h.block_sigs_from).unwrap_or(0)
}

/// Startup guards for mainnet mode (chain id [`BOING_MAINNET_CHAIN_ID`]).
///
/// * `BOING_OPERATOR_RPC_TOKEN` must be set (non-blank): without it every operator-gated method is open
///   to anyone who can reach the RPC port.
/// * The testnet faucet must be off: its signing key is derived from a public constant, so anyone can
///   spend the faucet account's funds.
///
/// Non-mainnet chain ids (testnet, devnets, unset) are unchanged. Returns a human-readable error.
pub fn check_mainnet_guards(
    chain_id: Option<u64>,
    operator_token_set: bool,
    faucet_enabled: bool,
) -> Result<(), String> {
    if chain_id != Some(BOING_MAINNET_CHAIN_ID) {
        return Ok(());
    }
    if !operator_token_set {
        return Err(format!(
            "refusing to start in mainnet mode (chain id {BOING_MAINNET_CHAIN_ID}) without BOING_OPERATOR_RPC_TOKEN: \
             operator RPC methods would be unauthenticated. Set a long random token, or use a different BOING_CHAIN_ID for non-mainnet networks"
        ));
    }
    if faucet_enabled {
        return Err(format!(
            "refusing to start with --faucet-enable on mainnet (chain id {BOING_MAINNET_CHAIN_ID}): the faucet signing key is a public constant"
        ));
    }
    Ok(())
}

/// Chain id used for signature checks: the installed network history's, else `BOING_CHAIN_ID`.
pub fn effective_chain_id(history: Option<&ExecutionHistory>) -> Option<u64> {
    history
        .map(|h| h.chain_id)
        .filter(|c| *c != 0)
        .or_else(configured_chain_id)
}

/// Sign a node-originated tx (faucet): v2 for `chain_id` when known (accepted before and after the v2
/// activation), else the legacy format (local dev without a chain id).
pub fn sign_for_chain(
    tx: boing_primitives::Transaction,
    key: &ed25519_dalek::SigningKey,
    chain_id: Option<u64>,
) -> boing_primitives::SignedTransaction {
    match chain_id {
        Some(c) => boing_primitives::SignedTransaction::new_for_chain(tx, key, c),
        None => boing_primitives::SignedTransaction::new_legacy(tx, key),
    }
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
            (
                faucet::testnet_faucet_account_id(),
                faucet::FAUCET_INITIAL_BALANCE,
            ),
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
        std::env::var("BOING_NETWORK_PRESET")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("none") | Some("off") | Some("0")
    ) {
        return None;
    }
    let chain_id: u64 = std::env::var("BOING_CHAIN_ID").ok()?.trim().parse().ok()?;
    preset_for_chain(chain_id)
}

/// Resolve micro-fee activation height: CLI, else `BOING_MICRO_FEES_ACTIVATION_HEIGHT`, else keep
/// the history preset value (`u64::MAX` on public testnet until operators opt in).
pub fn resolve_micro_fees_from(
    cli: Option<u64>,
    history: &mut boing_execution::ExecutionHistory,
) -> Result<(), String> {
    let from_env = match std::env::var("BOING_MICRO_FEES_ACTIVATION_HEIGHT") {
        Ok(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(
                    s.parse::<u64>()
                        .map_err(|_| format!("invalid BOING_MICRO_FEES_ACTIVATION_HEIGHT: {s}"))?,
                )
            }
        }
        Err(std::env::VarError::NotPresent) => None,
        Err(e) => return Err(e.to_string()),
    };
    if let Some(h) = cli.or(from_env) {
        history.micro_fees_from = h;
    }
    Ok(())
}

/// Apply a preset to a freshly constructed node: installs the historical execution eras and, if the
/// node is still at height 0, the network genesis (block 0 + allocations). Never touches a node that
/// already has blocks beyond genesis except for the (replay-only) execution eras.
/// Returns true when the genesis was (re)written.
pub fn apply_preset(node: &mut BoingNode, preset: &NetworkPreset) -> bool {
    node.executor
        .set_execution_history(Some(preset.history.clone()));
    node.apply_genesis_spec(&preset.genesis)
}

#[cfg(test)]
mod sign_for_chain_tests {
    use super::*;

    /// Node-signed txs (faucet) must pass the node's own admission policy after the v2 activation.
    #[test]
    fn mainnet_guards() {
        // mainnet: token required, faucet forbidden
        assert!(check_mainnet_guards(Some(6914), false, false)
            .unwrap_err()
            .contains("BOING_OPERATOR_RPC_TOKEN"));
        assert!(check_mainnet_guards(Some(6914), true, true)
            .unwrap_err()
            .contains("faucet"));
        assert!(check_mainnet_guards(Some(6914), false, true).is_err());
        assert!(check_mainnet_guards(Some(6914), true, false).is_ok());
        // testnet / devnet / unset behave as before (open operator RPC, faucet allowed)
        for id in [Some(BOING_TESTNET_CHAIN_ID), Some(1), Some(0), None] {
            assert!(check_mainnet_guards(id, false, true).is_ok());
            assert!(check_mainnet_guards(id, false, false).is_ok());
        }
    }

    #[test]
    fn faucet_signature_passes_policy_after_activation() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[3u8; 32]);
        let me = AccountId(key.verifying_key().to_bytes());
        let tx = boing_primitives::Transaction {
            nonce: 0,
            sender: me,
            payload: boing_primitives::TransactionPayload::Transfer {
                to: AccountId([4; 32]),
                amount: 1,
            },
            access_list: boing_primitives::AccessList::default(),
        };
        let required = boing_primitives::TxSigPolicy {
            chain_id: Some(6913),
            require_v2: true,
        };
        assert!(sign_for_chain(tx.clone(), &key, Some(6913))
            .verify_with(required)
            .is_ok());
        assert!(sign_for_chain(tx.clone(), &key, None)
            .verify_with(required)
            .is_err());
        assert!(sign_for_chain(tx, &key, None)
            .verify_with(boing_primitives::TxSigPolicy::default())
            .is_ok());
    }
}

#[cfg(test)]
mod preset_scope_tests {
    use super::*;

    /// Security review #13: the testnet replay preset (historical eras + fixed genesis) applies to chain
    /// 6913 only. Any other chain id (local dev, a future mainnet id) gets no preset and therefore the
    /// current rules from genesis and its own genesis.
    #[test]
    fn preset_only_for_testnet_chain_id() {
        assert!(preset_for_chain(BOING_TESTNET_CHAIN_ID).is_some());
        for other in [0u64, 1, 6912, 6914, 69_130, u64::MAX] {
            assert!(
                preset_for_chain(other).is_none(),
                "chain {other} must not get the testnet preset"
            );
        }
    }

    /// Activation heights only ever move forward, and every gated rule is on from v4 at the latest.
    #[test]
    fn testnet_gates_are_ordered() {
        let h = boing_testnet_history();
        assert_eq!(h.chain_id, BOING_TESTNET_CHAIN_ID);
        assert!(BOING_TESTNET_TREASURY_FEES_FROM < BOING_TESTNET_SECURITY_V2_FROM);
        assert!(BOING_TESTNET_SECURITY_V2_FROM < BOING_TESTNET_SECURITY_V3_FROM);
        assert!(BOING_TESTNET_SECURITY_V3_FROM < BOING_TESTNET_SECURITY_V4_FROM);
        assert_eq!(h.security_v4_from, BOING_TESTNET_SECURITY_V4_FROM);
        assert!(!h.security_v4_active(BOING_TESTNET_SECURITY_V4_FROM - 1));
        assert!(h.security_v4_active(BOING_TESTNET_SECURITY_V4_FROM));
    }
}
