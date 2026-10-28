//! Replays recorded public-testnet blocks (chain 6913) from genesis with the current code and checks
//! every block hash, receipts root and state root (via the normal catch-up import path). Guards
//! against changing historical execution rules: a rule change that alters committed roots must be
//! added as a height-gated era in `network_preset` instead.
//!
//! Fixture: `tests/fixtures/testnet_blocks.jsonl` (one `boing_getBlockByHeight(h, true)` result per
//! line, heights 0..=941: history through the security #2/#3 (900) and #16/#14 (920) activations, recorded
//! 2026-10-17 before the v4 gate at 1000). Override with `BOING_REPLAY_BLOCKS=<path>` to check a fresh dump of the
//! live chain. `replay_then_cross_redenomination_locally` continues past the fixture tip across a local
//! 18-decimals migration height. `tests/fixtures/testnet_blocks_after_942.jsonl` continues the recorded
//! chain from 942 through the live v4 gate (1000) and the live 18-decimals switch (1050), recorded
//! 2026-10-17 at tip 1052; `replay_live_chain_across_v4_and_redenomination` replays both files.
//! `tests/fixtures/testnet_blocks_1053_to_1310.jsonl` continues it to 1310, across the micro-fee switch
//! at 1188 and the media-blob gates (1200, 1306), recorded 2026-10-28 (live tip 1337). Blocks
//! 1311..=1317 carry up to ~80 MiB of incompressible `PutBlob` data each (~750 MB of JSON) and are not
//! committed; `replay_full_live_chain_to_the_tip` replays a full dump to the tip when
//! `BOING_REPLAY_FULL_CHAIN=<path>` is set (see docs/TESTNET-VALIDATOR-GUIDE.md).

use std::io::{BufRead, BufReader};

use boing_node::network_preset::{self, BOING_TESTNET_GENESIS_ACCOUNT};
use boing_node::node::BoingNode;
use boing_primitives::{
    receipts_root, tx_root, AccessList, AccountId, Block, BlockHeader, SignedTransaction,
    Transaction, TransactionPayload, BASE_UNITS_PER_BOING,
};

const TESTNET_OPEN_ACTIVATION: u64 = 400;
const TESTNET_NAME_ACTIVATION: u64 = 640;

fn fixture_path() -> std::path::PathBuf {
    std::env::var("BOING_REPLAY_BLOCKS")
        .map(Into::into)
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/testnet_blocks.jsonl")
        })
}

/// `"amount":"123"` -> `"amount":123` (only all-digit strings), so `u128` fields decode. The same for
/// `"value"` (native value of `ContractCallWithValue`, from 1150).
fn unquote_amounts(json: &str) -> String {
    let mut out = json.to_string();
    for key in ["amount", "value"] {
        out = unquote_key(&out, key);
    }
    out
}

fn unquote_key(json: &str, name: &str) -> String {
    let key = format!("\"{name}\":\"");
    let mut out = String::with_capacity(json.len());
    let mut rest = json;
    while let Some(i) = rest.find(&key) {
        let after = &rest[i + key.len()..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after.as_bytes().get(digits) == Some(&b'"') {
            out.push_str(&rest[..i]);
            out.push_str(&format!("\"{name}\":"));
            out.push_str(&after[..digits]);
            rest = &after[digits + 1..];
        } else {
            out.push_str(&rest[..i + key.len()]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn load_blocks() -> Vec<(Block, String)> {
    load_blocks_from(&fixture_path())
}

fn load_blocks_from(path: &std::path::Path) -> Vec<(Block, String)> {
    let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
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
            let hash = v["hash"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x")
                .to_string();
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
        assert_eq!(
            &hex::encode(block.hash().0),
            want_hash,
            "height {h}: block hash round-trip"
        );
        if h == 0 {
            if node.chain.latest_hash() != block.hash() {
                return Err((0, "genesis hash differs".into()));
            }
            continue;
        }
        node.import_network_block_catchup(block)
            .map_err(|e| (h, e.to_string()))?;
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
    assert!(
        err.0 < network_preset::BOING_TESTNET_TREASURY_FEES_FROM,
        "failed at {}",
        err.0
    );
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
    assert!(
        !p(h - 1).require_v2,
        "either signature format before activation"
    );
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
    assert_eq!(
        node.admission_vm().registration_proof_rule(),
        (Some(6913), h)
    );
    let blocks = load_blocks();
    assert!(
        blocks.last().unwrap().0.header.height >= h,
        "fixture covers the activation"
    );
    // Producers running the #16 binary attach signatures from 905; they are optional below the gate
    // (verified when present) and mandatory from it.
    for (b, _) in &blocks {
        if b.header.height >= h || !b.signatures.is_empty() {
            assert_eq!(
                b.signatures.len(),
                b.transactions.len(),
                "height {}",
                b.header.height
            );
        }
    }
    assert!(blocks.iter().any(|(b, _)| b.header.height < h
        && b.signatures.is_empty()
        && !b.transactions.is_empty()));
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
    assert!(
        blocks.last().unwrap().0.header.height < h,
        "fixture predates v4"
    );
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
    node.import_network_block_catchup(&blocks[idx].0)
        .expect("recorded block imports");
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

/// 18 decimals (mainnet plan step 1): the testnet ledger switches to base units at a fixed height after
/// the v4 gate. The fixture is below it (whole BOING), so the replays above prove the new code reproduces
/// history; this pins the gate in the executor, the VM and the admission VM.
#[test]
fn base_units_gate_follows_the_activation_height() {
    let h = network_preset::BOING_TESTNET_BASE_UNITS_FROM;
    assert!(
        h > network_preset::BOING_TESTNET_SECURITY_V4_FROM,
        "after the v4 gate"
    );
    let node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    use boing_primitives::Denomination::*;
    assert_eq!(node.executor.denomination_at(h - 1), WholeBoing);
    assert_eq!(node.executor.denomination_at(h), BaseUnits);
    assert_eq!(node.executor.redenomination_height(), Some(h));
    assert_eq!(node.executor.vm().base_units_from(), h);
    assert_eq!(node.admission_vm().base_units_from(), h);
    let blocks = load_blocks();
    assert!(
        blocks.last().unwrap().0.header.height < h
            || blocks.iter().any(|(b, _)| b.header.height == h)
    );
}

/// Builds the next block on `leader` with the leader's own executor (same rules as an importer).
fn craft_next(leader: &BoingNode, txs: Vec<SignedTransaction>) -> Block {
    craft_next_with(leader, txs, true)
}

/// [`craft_next`], optionally allowing failed receipts (a proposer may include a tx that fails).
fn craft_next_with(leader: &BoingNode, txs: Vec<SignedTransaction>, all_succeed: bool) -> Block {
    let height = leader.chain.height() + 1;
    let proposer = leader.consensus.leader(height);
    let ts = leader
        .chain
        .get_block_by_height(height - 1)
        .unwrap()
        .header
        .timestamp
        + 1;
    let mut state = leader.state.snapshot();
    let plain: Vec<Transaction> = txs.iter().map(|s| s.tx.clone()).collect();
    let (_g, receipts) = leader
        .executor
        .execute_block(height, ts, &plain, &mut state, proposer)
        .unwrap();
    if all_succeed {
        assert!(receipts.iter().all(|r| r.success), "{receipts:?}");
    }
    let reward = leader.executor.block_emission(height);
    if reward > 0 {
        match state.get_mut(&proposer) {
            Some(a) => a.balance += reward,
            None => state.insert(boing_primitives::Account {
                id: proposer,
                state: boing_primitives::AccountState {
                    balance: reward,
                    ..Default::default()
                },
            }),
        }
    }
    state.apply_open_validator_block_hook(height, proposer);
    Block {
        header: BlockHeader {
            parent_hash: leader.chain.latest_hash(),
            height,
            timestamp: ts,
            proposer,
            tx_root: tx_root(&plain),
            receipts_root: receipts_root(&receipts),
            state_root: state.state_root(),
            blobs_root: None,
        },
        signatures: txs.iter().map(|s| s.signature).collect(),
        priority_tips: {
            let tips: Vec<u128> = txs.iter().map(|s| s.priority_tip).collect();
            if tips.iter().any(|&t| t != 0) {
                tips
            } else {
                vec![]
            }
        },
        transactions: plain,
    }
}

/// Replay across H: replays the recorded chain, then continues it past a local migration height
/// (tip + 2) with faucet transfers. Two independent nodes import every block (identical roots), the
/// ledger is multiplied by 10^18 exactly once at H (chain-wide sums before/after), amounts and fees
/// switch unit with it, and the block after H does not multiply again.
#[test]
fn replay_then_cross_redenomination_locally() {
    // Admission reads the chain id from the environment, as on the public nodes (all nodes in this
    // binary are chain 6913).
    std::env::set_var("BOING_CHAIN_ID", "6913");
    let blocks = load_blocks();
    let tip = blocks.last().unwrap().0.header.height;
    let h = tip + 2;
    let mut a = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    let mut b = testnet_node(AccountId([7u8; 32]), true);
    replay(&mut a, &blocks).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    replay(&mut b, &blocks).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    let mut hist = network_preset::boing_testnet_history();
    assert!(h < hist.security_v4_from && h < hist.base_units_from);
    hist.base_units_from = h;
    a.executor.set_execution_history(Some(hist.clone()));
    b.executor.set_execution_history(Some(hist));

    let key = boing_node::faucet::testnet_faucet_signing_key();
    let faucet = boing_node::faucet::testnet_faucet_account_id();
    let to = AccountId([0xD1; 32]);
    let send = |n: &BoingNode, amount: u128| {
        let nonce = n.state.get(&faucet).unwrap().nonce;
        let tx = Transaction {
            nonce,
            sender: faucet,
            payload: TransactionPayload::Transfer { to, amount },
            access_list: AccessList::new(vec![faucet, to], vec![faucet, to]),
        };
        SignedTransaction::new_for_chain(tx, &key, 6913)
    };
    let step = |a: &mut BoingNode, b: &mut BoingNode, amount: u128| {
        let blk = craft_next(a, vec![send(a, amount)]);
        a.import_network_block_catchup(&blk)
            .expect("leader imports");
        b.import_network_block_catchup(&blk)
            .expect("follower imports");
        assert_eq!(a.chain.latest_hash(), b.chain.latest_hash());
        assert_eq!(a.state.state_root(), b.state.state_root());
    };

    // H - 1: whole BOING (20, the faucet amount).
    step(&mut a, &mut b, 20);
    assert_eq!(a.chain.height(), h - 1);
    let before = a.state.compute_native_aggregates();
    let faucet_before = a.state.get(&faucet).unwrap().balance;
    assert_eq!(a.state.get(&to).unwrap().balance, 20);
    assert!(faucet_before < 10_000_000, "whole units before H");

    // Admission for the block at H sees the migrated state: 20 BOING in base units is far above the
    // whole-unit balance, yet is admitted because the next block executes after the migration.
    a.submit_transaction(send(&a, 20 * BASE_UNITS_PER_BOING))
        .expect("admission at next == H uses the migrated state");

    // H: the migration runs first, then the block's tx in base units (the faucet's 20 BOING).
    step(&mut a, &mut b, 20 * BASE_UNITS_PER_BOING);
    let after = a.state.compute_native_aggregates();
    assert_eq!(
        after.total_balance,
        before.total_balance * BASE_UNITS_PER_BOING + a.executor.block_emission(h),
        "every balance x10^18 once, plus the base-unit emission (fees only move value)"
    );
    assert_eq!(after.total_stake, before.total_stake * BASE_UNITS_PER_BOING);
    assert_eq!(a.state.get(&to).unwrap().balance, 40 * BASE_UNITS_PER_BOING);
    let fee_h =
        a.executor
            .network_fee_at(21_000, &TransactionPayload::Transfer { to, amount: 1 }, h);
    assert_eq!(
        a.state.get(&faucet).unwrap().balance,
        faucet_before * BASE_UNITS_PER_BOING - 20 * BASE_UNITS_PER_BOING - fee_h
    );
    for r in a.state.open_validators().records.values() {
        assert_eq!(r.bond % BASE_UNITS_PER_BOING, 0, "bonds scaled");
        assert!(r.bond >= BASE_UNITS_PER_BOING);
    }

    // H + 1: no second multiplication.
    let mid = a.state.compute_native_aggregates();
    step(&mut a, &mut b, 1);
    let end = a.state.compute_native_aggregates();
    assert_eq!(
        end.total_balance,
        mid.total_balance + a.executor.block_emission(h + 1)
    );
    assert_eq!(
        a.state.get(&to).unwrap().balance,
        40 * BASE_UNITS_PER_BOING + 1
    );
}

/// The recorded public chain past the main fixture: the v4 gate (1000) and the live 18-decimals
/// switch (1050). Every recorded hash must reproduce; at 1050 the ledger is multiplied by 10^18
/// exactly once (chain-wide sum), and a faucet recipient funded before the switch (20 whole BOING at
/// block 1003) holds 20 * 10^18 base units afterwards.
#[test]
fn replay_live_chain_across_v4_and_redenomination() {
    std::env::set_var("BOING_CHAIN_ID", "6913");
    let main = load_blocks();
    let ext = load_blocks_from(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/testnet_blocks_after_942.jsonl"),
    );
    let h = network_preset::BOING_TESTNET_BASE_UNITS_FROM;
    assert_eq!(
        ext[0].0.header.height,
        main.last().unwrap().0.header.height + 1,
        "contiguous"
    );
    assert!(ext.last().unwrap().0.header.height > h, "covers the switch");
    let split = ext.iter().position(|(b, _)| b.header.height == h).unwrap();
    let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    replay(&mut node, &main).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    replay(&mut node, &ext[..split]).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    assert_eq!(node.chain.height(), h - 1);
    let mut id = [0u8; 32];
    id.copy_from_slice(
        &hex::decode("9bd09b6daa581894b7319af1de1dce5e874cc19d9e07bda462e99471cd828332").unwrap(),
    );
    let recipient = AccountId(id);
    assert_eq!(
        node.state.get(&recipient).unwrap().balance,
        20,
        "whole BOING before the switch"
    );
    let before = node.state.compute_native_aggregates();
    replay(&mut node, &ext[split..split + 1]).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    let after = node.state.compute_native_aggregates();
    assert_eq!(
        after.total_balance,
        before.total_balance * BASE_UNITS_PER_BOING + node.executor.block_emission(h),
        "x10^18 once at the switch, plus the base-unit emission"
    );
    assert_eq!(after.total_stake, before.total_stake * BASE_UNITS_PER_BOING);
    replay(&mut node, &ext[split + 1..]).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    assert_eq!(
        node.state.get(&recipient).unwrap().balance,
        20 * BASE_UNITS_PER_BOING
    );
    assert_eq!(node.chain.latest_hash(), ext.last().unwrap().0.hash());
}

/// Native value pins: the testnet activates it at 1150, after the 18-decimals switch, in the VM, the
/// admission VM and the executor.
#[test]
fn native_value_gate_follows_the_activation_height() {
    let h = network_preset::BOING_TESTNET_NATIVE_VALUE_FROM;
    assert_eq!(h, 1_150);
    assert!(h > network_preset::BOING_TESTNET_BASE_UNITS_FROM);
    let node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    assert!(!node.executor.native_value_active(h - 1));
    assert!(node.executor.native_value_active(h));
    assert_eq!(node.executor.vm().native_value_from(), h);
    assert_eq!(node.admission_vm().native_value_from(), h);
}

/// Replay across H for native value: replays the whole recorded chain (genesis..=1052, through the
/// v4 gate and the 18-decimals switch), then continues it on two independent nodes past a local
/// native-value height (tip + 2). Before H, admission rejects value calls and deploys using the new
/// opcodes, and a value call a proposer includes anyway fails identically on both nodes (no value
/// moves). From H, wBOING deploys, wraps 1 BOING with a value call and unwraps it with a contract
/// payout; both nodes agree on every block hash and state root.
#[test]
fn replay_live_chain_then_cross_native_value_locally() {
    use boing_execution::{
        encode_wboing_deposit_calldata, encode_wboing_withdraw_calldata, wrapped_boing_bytecode,
    };
    std::env::set_var("BOING_CHAIN_ID", "6913");
    let main = load_blocks();
    let ext = load_blocks_from(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/testnet_blocks_after_942.jsonl"),
    );
    let mut a = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    let mut b = testnet_node(AccountId([7u8; 32]), true);
    for n in [&mut a, &mut b] {
        replay(n, &main).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
        replay(n, &ext).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
        assert_eq!(n.chain.latest_hash(), ext.last().unwrap().0.hash());
    }
    let tip = a.chain.height();
    let h = tip + 2;
    let mut hist = network_preset::boing_testnet_history();
    assert!(
        h < hist.native_value_from,
        "the recorded chain is below the live gate"
    );
    hist.native_value_from = h;
    a.executor.set_execution_history(Some(hist.clone()));
    b.executor.set_execution_history(Some(hist));

    let key = boing_node::faucet::testnet_faucet_signing_key();
    let faucet = boing_node::faucet::testnet_faucet_account_id();
    let sign = |n: &BoingNode, payload: TransactionPayload, offset: u64| {
        let nonce = n.state.get(&faucet).unwrap().nonce + offset;
        let mut tx = Transaction {
            nonce,
            sender: faucet,
            payload,
            access_list: AccessList::default(),
        };
        tx.access_list = tx.suggested_parallel_access_list();
        SignedTransaction::new_for_chain(tx, &key, 6913)
    };
    let deploy = TransactionPayload::ContractDeployWithPurposeAndMetadata {
        bytecode: wrapped_boing_bytecode(),
        purpose_category: "token".into(),
        description_hash: None,
        asset_name: Some("Wrapped BOING".into()),
        asset_symbol: Some("wBOING".into()),
        create2_salt: None,
    };
    let some_contract = boing_primitives::AccountId(
        hex::decode("6676f971ade3982406efa3e2cdb0477923f8f2a15624c4b6ad3a432817a75e93")
            .unwrap()
            .try_into()
            .unwrap(),
    ); // testnet USDC (a contract)
    let value_call = |to: AccountId| TransactionPayload::ContractCallWithValue {
        contract: to,
        calldata: vec![],
        value: BASE_UNITS_PER_BOING,
    };
    let import = |a: &mut BoingNode, b: &mut BoingNode, blk: &Block| {
        a.import_network_block_catchup(blk).expect("leader imports");
        b.import_network_block_catchup(blk)
            .expect("follower imports");
        assert_eq!(a.chain.latest_hash(), b.chain.latest_hash());
        assert_eq!(a.state.state_root(), b.state.state_root());
    };

    // H - 1: admission rejects value calls and the new opcodes; a proposer that includes a value
    // call anyway gets a failed receipt on every node and nothing moves but the failed-tx fee.
    let e = a
        .submit_transaction(sign(&a, value_call(some_contract), 0))
        .unwrap_err();
    assert!(e.to_string().contains("not active"), "{e}");
    let e = a
        .submit_transaction(sign(&a, deploy.clone(), 0))
        .unwrap_err();
    assert!(e.to_string().contains("INVALID_OPCODE"), "{e}");
    let usdc_before = a.state.get(&some_contract).map(|s| s.balance).unwrap_or(0);
    let blk = craft_next_with(&a, vec![sign(&a, value_call(some_contract), 0)], false);
    import(&mut a, &mut b, &blk);
    assert_eq!(a.chain.height(), h - 1);
    let r = a.chain.get_block_by_height(h - 1).unwrap();
    assert_eq!(r.transactions.len(), 1);
    assert_eq!(
        a.state.get(&some_contract).map(|s| s.balance).unwrap_or(0),
        usdc_before
    );

    // H: wBOING deploys (new opcodes pass QA), admission now takes value calls.
    let wboing = boing_primitives::nonce_derived_contract_address(
        &faucet,
        a.state.get(&faucet).unwrap().nonce,
    );
    let blk = craft_next(&a, vec![sign(&a, deploy, 0)]);
    import(&mut a, &mut b, &blk);
    assert!(a.state.get_contract_code(&wboing).is_some());
    let wrap = TransactionPayload::ContractCallWithValue {
        contract: wboing,
        calldata: encode_wboing_deposit_calldata(),
        value: BASE_UNITS_PER_BOING,
    };
    a.submit_transaction(sign(&a, wrap.clone(), 0))
        .expect("admission from H");

    // H + 1: wrap 1 BOING; H + 2: unwrap it (contract payout).
    let f0 = a.state.get(&faucet).unwrap().balance;
    let blk = craft_next(&a, vec![sign(&a, wrap, 0)]);
    import(&mut a, &mut b, &blk);
    assert_eq!(a.state.get(&wboing).unwrap().balance, BASE_UNITS_PER_BOING);
    let f1 = a.state.get(&faucet).unwrap().balance;
    assert!(f0 - f1 > BASE_UNITS_PER_BOING, "value plus fee");
    let unwrap = TransactionPayload::ContractCall {
        contract: wboing,
        calldata: encode_wboing_withdraw_calldata(BASE_UNITS_PER_BOING),
    };
    let blk = craft_next(&a, vec![sign(&a, unwrap, 0)]);
    import(&mut a, &mut b, &blk);
    assert_eq!(a.state.get(&wboing).unwrap().balance, 0);
    let f2 = a.state.get(&faucet).unwrap().balance;
    assert!(
        f2 > f1 && f2 - f1 < BASE_UNITS_PER_BOING,
        "1 BOING back minus the fee"
    );
}

// ---------------------------------------------------------------------------------------------
// Micro fees (1188) and media-blob gates (1200, 1306): the preset alone must reproduce the live
// chain, with no env vars and no --network-fee-config.
// ---------------------------------------------------------------------------------------------

const MICRO_FEES_FROM: u64 = 1_188;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Fixtures concatenated: heights 0..=1310 (the committed part of the live chain).
fn load_chain_to_1310() -> Vec<(Block, String)> {
    let mut all = load_blocks_from(&fixture("testnet_blocks.jsonl"));
    for name in [
        "testnet_blocks_after_942.jsonl",
        "testnet_blocks_1053_to_1310.jsonl",
    ] {
        let part = load_blocks_from(&fixture(name));
        assert_eq!(
            part[0].0.header.height,
            all.last().unwrap().0.header.height + 1,
            "{name} must continue the previous fixture"
        );
        all.extend(part);
    }
    assert_eq!(all.last().unwrap().0.header.height, 1_310);
    all
}

fn first_failure(blocks: &[(Block, String)], node: &mut BoingNode) -> Option<u64> {
    replay(node, blocks).err().map(|(h, _)| h)
}

/// Pins the testnet preset values (consensus: never change once a network runs them).
#[test]
fn testnet_preset_carries_micro_fees_and_media_blob_gates() {
    let p = network_preset::boing_testnet_preset();
    assert_eq!(p.history.micro_fees_from, MICRO_FEES_FROM);
    assert_eq!(p.media_blobs_from, Some(1_200));
    assert_eq!(p.media_blobs_phase7_cap_from, Some(1_306));
    let fee = p.fee_policy.expect("preset carries the live fee policy");
    assert_eq!(fee.execution_gas_fee_bps, 100);
    assert_eq!(fee, boing_tokenomics::NetworkFeePolicy::micro_fee_default());
    // Other networks get no preset.
    assert!(network_preset::preset_for_chain(6914).is_none());
}

/// A fresh testnet node (preset only) replays genesis..=1310 across the micro-fee switch at 1188 with
/// every block hash and state root matching the live chain, and reports the live fee rule.
#[test]
fn replay_live_chain_to_1310_with_preset_only() {
    std::env::set_var("BOING_CHAIN_ID", "6913");
    let blocks = load_chain_to_1310();
    let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    replay(&mut node, &blocks).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    assert_eq!(node.chain.height(), 1_310);
    assert_eq!(node.chain.latest_hash(), blocks.last().unwrap().0.hash());
    assert_eq!(
        node.executor
            .fee_rule_at(MICRO_FEES_FROM - 1)
            .0
            .execution_gas_fee_bps,
        10_000
    );
    assert_eq!(
        node.executor
            .fee_rule_at(MICRO_FEES_FROM)
            .0
            .execution_gas_fee_bps,
        100
    );
    assert_eq!(node.executor.vm().media_blobs_from(), Some(1_200));
    assert_eq!(node.media_blobs_phase7_cap_activation_height(), Some(1_306));
}

/// Explicitly configuring exactly what the live Fly node runs (env/CLI heights, the fee policy file)
/// on top of the preset changes nothing: no double application, same roots.
#[test]
fn explicit_live_settings_on_top_of_the_preset_replay_identically() {
    std::env::set_var("BOING_CHAIN_ID", "6913");
    let blocks = load_chain_to_1310();
    let mut preset = network_preset::boing_testnet_preset();
    network_preset::resolve_micro_fees_from(Some(MICRO_FEES_FROM), &mut preset.history).unwrap();
    let mut node = BoingNode::with_validators(
        vec![BOING_TESTNET_GENESIS_ACCOUNT],
        BOING_TESTNET_GENESIS_ACCOUNT,
        None,
    );
    network_preset::apply_preset(&mut node, &preset);
    // What main() does after the preset: env/CLI heights, then --network-fee-config.
    node.enable_media_blobs(1_200);
    node.enable_media_blobs_phase7_cap(1_306);
    node.set_fee_policy(boing_tokenomics::NetworkFeePolicy::micro_fee_default());
    let mut open = boing_state::OpenValidatorParams::default();
    open.activation_height = TESTNET_OPEN_ACTIVATION;
    open.anchors = vec![BOING_TESTNET_GENESIS_ACCOUNT];
    node.enable_open_validator_set(open);
    node.enable_name_registry(boing_state::NameRegistryParams {
        activation_height: TESTNET_NAME_ACTIVATION,
        ..Default::default()
    });
    replay(&mut node, &blocks).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    assert_eq!(node.chain.latest_hash(), blocks.last().unwrap().0.hash());
}

/// Block 1188 needs the micro-fee height *and* the 100 bps policy. Dropping either, or moving the
/// height by one, breaks the replay at exactly the expected block (documents the regression).
#[test]
fn micro_fee_switch_needs_both_height_and_policy() {
    std::env::set_var("BOING_CHAIN_ID", "6913");
    let blocks = load_chain_to_1310();
    let upto_1190: Vec<_> = blocks[..=1_190].to_vec();

    // Height only (policy left at the 10_000 bps default): the reported failure at 1188.
    let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    node.set_fee_policy(boing_tokenomics::NetworkFeePolicy::treasury_default());
    assert_eq!(first_failure(&upto_1190, &mut node), Some(MICRO_FEES_FROM));

    // Policy only (height never): same block fails.
    let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    node.executor.set_execution_history(Some({
        let mut h = network_preset::boing_testnet_history();
        h.micro_fees_from = u64::MAX;
        h
    }));
    assert_eq!(first_failure(&upto_1190, &mut node), Some(MICRO_FEES_FROM));

    // One block early fails one block early; one late fails at 1188.
    for (from, fails_at) in [
        (MICRO_FEES_FROM - 1, MICRO_FEES_FROM - 1),
        (MICRO_FEES_FROM + 1, MICRO_FEES_FROM),
    ] {
        let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
        let mut h = network_preset::boing_testnet_history();
        h.micro_fees_from = from;
        node.executor.set_execution_history(Some(h));
        assert_eq!(
            first_failure(&upto_1190, &mut node),
            Some(fails_at),
            "micro_fees_from={from}"
        );
    }
}

/// A node's own fee policy (a persisted `network_fee_config.json`, as the live node has) is never
/// replaced by the preset default; a node with none gets the preset policy in memory only.
#[test]
fn preset_fee_policy_is_a_default_not_an_override() {
    let dir = std::env::temp_dir().join(format!(
        "boing-preset-fee-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let preset = network_preset::boing_testnet_preset();
    let mut custom = boing_tokenomics::NetworkFeePolicy::treasury_default();
    custom.execution_gas_fee_bps = 5_000;

    // No persisted policy: preset default applies and nothing is written.
    let mut fresh = BoingNode::with_data_dir_and_validators(
        Some(&dir),
        vec![BOING_TESTNET_GENESIS_ACCOUNT],
        BOING_TESTNET_GENESIS_ACCOUNT,
        None,
    )
    .unwrap();
    network_preset::apply_preset(&mut fresh, &preset);
    assert_eq!(fresh.executor.fee_policy().execution_gas_fee_bps, 100);
    assert!(
        !dir.join("network_fee_config.json").exists(),
        "preset default must not be persisted"
    );

    // Persisted policy (operator RPC / --network-fee-config): kept across a restart.
    fresh.set_fee_policy(custom.clone());
    drop(fresh);
    let mut restarted = BoingNode::with_data_dir_and_validators(
        Some(&dir),
        vec![BOING_TESTNET_GENESIS_ACCOUNT],
        BOING_TESTNET_GENESIS_ACCOUNT,
        None,
    )
    .unwrap();
    network_preset::apply_preset(&mut restarted, &preset);
    assert_eq!(restarted.executor.fee_policy(), &custom);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Replays a complete dump of the live chain (every block `boing_getBlockByHeight(h, true)`, one per
/// line, genesis..=tip) with the preset only. Skipped unless `BOING_REPLAY_FULL_CHAIN` names the dump;
/// blocks 1311..=1317 are huge `PutBlob` blocks, so the dump is ~750 MB and cannot be committed.
#[test]
fn replay_full_live_chain_to_the_tip() {
    let Ok(path) = std::env::var("BOING_REPLAY_FULL_CHAIN") else {
        eprintln!("BOING_REPLAY_FULL_CHAIN not set; skipping full-chain replay");
        return;
    };
    std::env::set_var("BOING_CHAIN_ID", "6913");
    let blocks = load_blocks_from(std::path::Path::new(&path));
    let tip = blocks.last().unwrap().0.header.height;
    let mut node = testnet_node(BOING_TESTNET_GENESIS_ACCOUNT, true);
    replay(&mut node, &blocks).unwrap_or_else(|(h, e)| panic!("height {h}: {e}"));
    assert_eq!(node.chain.height(), tip);
    assert_eq!(node.chain.latest_hash(), blocks.last().unwrap().0.hash());
    println!("full-chain replay OK to tip {tip}");
}
