//! Replays recorded public-testnet blocks (chain 6913) from genesis with the current code and checks
//! every block hash, receipts root and state root (via the normal catch-up import path). Guards
//! against changing historical execution rules: a rule change that alters committed roots must be
//! added as a height-gated era in `network_preset` instead.
//!
//! Fixture: `tests/fixtures/testnet_blocks.jsonl` (one `boing_getBlockByHeight(h, true)` result per
//! line, heights 0..=671). Override with `BOING_REPLAY_BLOCKS=<path>` to check a fresh dump of the
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
            let block: Block = serde_json::from_value(serde_json::json!({
                "header": v["header"], "transactions": v["transactions"],
            }))
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
