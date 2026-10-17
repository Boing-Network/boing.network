//! Replays recorded public-testnet blocks (chain 6913) from genesis with the current code and checks
//! every block hash, receipts root and state root (via the normal catch-up import path). Guards
//! against changing historical execution rules: a rule change that alters committed roots must be
//! added as a height-gated era in `network_preset` instead.
//!
//! Fixture: `tests/fixtures/testnet_blocks.jsonl` (one `boing_getBlockByHeight(h, true)` result per
//! line, heights 0..=941: history through the security #2/#3 (900) and #16/#14 (920) activations, recorded
//! 2026-10-17 before the v4 gate at 1000). Override with `BOING_REPLAY_BLOCKS=<path>` to check a fresh dump of the
//! live chain.

use std::io::{BufRead, BufReader};

use boing_node::network_preset::{self, BOING_TESTNET_GENESIS_ACCOUNT};
use boing_node::node::BoingNode;
use boing_primitives::{AccountId, Block};

const TESTNET_OPEN_ACTIVATION: u64 = 400;
const TESTNET_NAME_ACTIVATION: u64 = 640;

fn fixture_path() -> std::path::PathBuf {
    std::env::var("BOING_REPLAY_BLOCKS").map(Into::into).unwrap_or_else(|_| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testnet_blocks.jsonl")
    })
}

/// `"amount":"123"` -> `"amount":123` (only all-digit strings), so `u128` fields decode.
fn unquote_amounts(json: &str) -> String {
    const KEY: &str = "\"amount\":\"";
    let mut out = String::with_capacity(json.len());
    let mut rest = json;
    while let Some(i) = rest.find(KEY) {
        let after = &rest[i + KEY.len()..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after.as_bytes().get(digits) == Some(&b'"') {
            out.push_str(&rest[..i]);
            out.push_str("\"amount\":");
            out.push_str(&after[..digits]);
            rest = &after[digits + 1..];
        } else {
            out.push_str(&rest[..i + KEY.len()]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn load_blocks() -> Vec<(Block, String)> {
    let path = fixture_path();
    let f = std::fs::File::open(&path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    BufReader::new(f)
        .lines()
        .map(|l| l.unwrap())
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, line)| {
            let v: serde_json::Value = serde_json::from_str(&line).unwrap();
            let mut enc = serde_json::json!({
                "header": v["header"], "transactions": v["transactions"],
            });
            if v["signatures"].is_array() {
                enc["signatures"] = v["signatures"].clone();
            }
            // The RPC prints u128 amounts above u64 as decimal strings; decode them as numbers.
            let block: Block = serde_json::from_str(&unquote_amounts(&enc.to_string()))
            .unwrap_or_else(|e| panic!("line {i}: decode block: {e}"));
            let hash = v["hash"].as_str().unwrap().trim_start_matches("0x").to_string();
            (block, hash)
        })
        .collect()
}

/// A testnet node configured as the public nodes are (open set at 400, names at 640), with
/// `local` as its own identity (a validator joining with its own key, or a plain follower).
fn testnet_node(local: AccountId, with_preset: bool) -> BoingNode {
    let mut n = BoingNode::with_validators(vec![BOING_TESTNET_GENESIS_ACCOUNT], local, None);
    if with_preset {
        network_preset::apply_preset(&mut n, &network_preset::boing_testnet_preset());
    }
    let mut open = boing_state::OpenValidatorParams::default();
    open.activation_height = TESTNET_OPEN_ACTIVATION;
    open.anchors = vec![BOING_TESTNET_GENESIS_ACCOUNT];
    n.enable_open_validator_set(open);
    n.enable_name_registry(boing_state::NameRegistryParams {
        activation_height: TESTNET_NAME_ACTIVATION,
        ..Default::default()
    });
    n
}

/// Replays all blocks; returns the first failing height (with the error) or the number imported.
fn replay(node: &mut BoingNode, blocks: &[(Block, String)]) -> Result<u64, (u64, String)> {
    let mut imported = 0;
    for (block, want_hash) in blocks {
        let h = block.header.height;
        assert_eq!(&hex::encode(block.hash().0), want_hash, "height {h}: block hash round-trip");
        if h == 0 {
            if node.chain.latest_hash() != block.hash() {
                return Err((0, "genesis hash differs".into()));
            }
            continue;
        }
        node.import_network_block_catchup(block).map_err(|e| (h, e.to_string()))?;
        imported += 1;
    }
    Ok(imported)
}

#[test]
fn replay_recorded_testnet_blocks_from_genesis() {
    let blocks = load_blocks();
    let tip = blocks.last().unwrap().0.header.height;
    let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    let n = replay(&mut node, &blocks).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    assert_eq!(n, tip);
    assert_eq!(node.chain.height(), tip);
    assert_eq!(node.chain.latest_hash(), blocks.last().unwrap().0.hash());
}

/// A node started with its own validator key (open-mode candidate) must build the same genesis.
#[test]
fn replay_with_own_validator_identity() {
    let blocks = load_blocks();
    let mut node = testnet_node(AccountId([7u8; 32]), true);
    let n = replay(&mut node, &blocks).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    assert_eq!(n, blocks.last().unwrap().0.header.height);
}

/// Without the preset the current rules alone cannot reproduce early blocks (documents why the
/// historical fee era exists).
#[test]
fn replay_without_preset_fails_in_legacy_fee_era() {
    let blocks = load_blocks();
    let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, false);
    let err = replay(&mut node, &blocks).expect_err("current rules must not match legacy blocks");
    assert!(err.0 < network_preset::BOING_TESTNET_TREASURY_FEES_FROM, "failed at {}", err.0);
}

/// Security fixes #2/#3 activate at a fixed testnet height. Every recorded block below it must still
/// replay with the historical rules (checked by the replays above); this pins the gates themselves.
#[test]
fn security_v2_gates_follow_the_activation_height() {
    let h = network_preset::BOING_TESTNET_SECURITY_V2_FROM;
    let hist = network_preset::boing_testnet_history();
    let node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    assert!(!node.executor.failed_tx_fee_active(h - 1));
    assert!(node.executor.failed_tx_fee_active(h));
    let p = |next| network_preset::tx_sig_policy(Some(6913), Some(&hist), next);
    assert!(!p(h - 1).require_v2, "either signature format before activation");
    assert!(p(h).require_v2, "v2 required from activation");
    assert_eq!(p(h).chain_id, Some(6913));
    // Without a chain id (local dev) nothing is required.
    assert!(!network_preset::tx_sig_policy(None, Some(&hist), h + 5).require_v2);
}

/// Security fixes #16 (blocks carry verified tx signatures) and #14 (chain-bound registration proof)
/// activate at a later fixed height. Recorded blocks below it carry no signatures and blocks from it
/// carry one per tx, so the replays above prove both sides of the gate import unchanged.
#[test]
fn security_v3_gates_follow_the_activation_height() {
    let h = network_preset::BOING_TESTNET_SECURITY_V3_FROM;
    assert!(h > network_preset::BOING_TESTNET_SECURITY_V2_FROM);
    let hist = network_preset::boing_testnet_history();
    assert_eq!(network_preset::block_sigs_from(Some(&hist)), h);
    assert_eq!(hist.registration_proof_v2_from, h);
    assert_eq!(network_preset::effective_chain_id(Some(&hist)), Some(6913));
    // Admission/simulation use the same registration proof rule as block execution.
    let node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    assert_eq!(node.executor.registration_proof_rule(), (Some(6913), h));
    assert_eq!(node.admission_vm().registration_proof_rule(), (Some(6913), h));
    let blocks = load_blocks();
    assert!(blocks.last().unwrap().0.header.height >= h, "fixture covers the activation");
    // Producers running the #16 binary attach signatures from 905; they are optional below the gate
    // (verified when present) and mandatory from it.
    for (b, _) in &blocks {
        if b.header.height >= h || !b.signatures.is_empty() {
            assert_eq!(b.signatures.len(), b.transactions.len(), "height {}", b.header.height);
        }
    }
    assert!(blocks.iter().any(|(b, _)| b.header.height < h && b.signatures.is_empty() && !b.transactions.is_empty()));
}

/// The 2026-10-17 fixes (#6 name commitments, #9 parallel fee accounting, #10 registry cap, #11 jail
/// exit) activate at v4. The fixture is entirely below it, so the replays above prove the new code
/// reproduces history; this pins the gate in the executor and the admission VM.
#[test]
fn security_v4_gates_follow_the_activation_height() {
    let h = network_preset::BOING_TESTNET_SECURITY_V4_FROM;
    let node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    assert!(!node.executor.security_v4_active(h - 1));
    assert!(node.executor.security_v4_active(h));
    assert_eq!(node.executor.vm().security_v4_from(), h);
    assert_eq!(node.admission_vm().security_v4_from(), h);
    let blocks = load_blocks();
    assert!(blocks.last().unwrap().0.header.height < h, "fixture predates v4");
}

/// A recorded block given a bogus signature list must
/// be rejected even before activation (signatures, when present, are always verified), while the
/// same block as recorded imports.
#[test]
fn bogus_signatures_on_recorded_blocks_are_rejected() {
    let blocks = load_blocks();
    let (idx, _) = blocks
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, (b, _))| !b.transactions.is_empty())
        .expect("fixture has a block with transactions");
    let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    replay(&mut node, &blocks[..idx]).unwrap();
    let mut bogus = blocks[idx].0.clone();
    bogus.signatures = vec![boing_primitives::Signature([0u8; 64]); bogus.transactions.len()];
    assert!(node.import_network_block_catchup(&bogus).is_err());
    node.import_network_block_catchup(&blocks[idx].0).expect("recorded block imports");
}

/// Replays the recorded blocks twice in independent nodes and compares every state root along the
/// way (determinism across nodes, including blocks with failed receipts after activation when the
/// fixture covers them).
#[test]
fn replay_is_deterministic_across_nodes() {
    let blocks = load_blocks();
    let mut a = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    let mut b = testnet_node(AccountId([7u8; 32]), true);
    for (block, _) in blocks.iter().skip(1) {
        a.import_network_block_catchup(block).unwrap();
        b.import_network_block_catchup(block).unwrap();
        assert_eq!(a.chain.latest_hash(), b.chain.latest_hash());
    }
}
