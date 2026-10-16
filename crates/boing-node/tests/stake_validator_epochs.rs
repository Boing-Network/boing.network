//! Opt-in stake-derived validator set refresh at epoch boundaries.

use std::collections::HashMap;

use boing_node::node::{BoingNode, StakeValidatorSetConfig};
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn node_with_accounts(
    proposer_key: &SigningKey,
    extra: Vec<(AccountId, u128, u128)>,
) -> BoingNode {
    let proposer = AccountId(proposer_key.verifying_key().to_bytes());
    let genesis = boing_node::chain::ChainState::genesis(proposer);
    let chain = boing_node::chain::ChainState::from_genesis(genesis.clone());
    let mut consensus = boing_consensus::ConsensusEngine::single_validator(proposer);
    let _ = consensus.propose_and_commit(genesis);

    let mut state = boing_state::StateStore::new();
    state.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1_000_000,
            nonce: 0,
            stake: 0, ..Default::default() },
    });
    for (id, balance, stake) in extra {
        state.insert(Account {
            id,
            state: AccountState {
                balance,
                nonce: 0,
                stake, ..Default::default() },
        });
    }

    let native_aggregates = state.compute_native_aggregates();
    BoingNode {
        chain,
        consensus,
        state,
        executor: boing_execution::BlockExecutor::new(),
        producer: boing_node::block_producer::BlockProducer::new(proposer).with_max_txs(100),
        vm: boing_execution::Vm::new(),
        scheduler: boing_execution::TransactionScheduler::new(),
        mempool: boing_node::mempool::Mempool::new(),
        p2p: boing_p2p::P2pNode::default(),
        dapp_registry: boing_node::dapp_registry::DappRegistry::new(),
        intent_pool: boing_node::intent_pool::IntentPool::new(),
        qa_pool: boing_node::node::pending_qa_pool_default(),
        persistence: None,
        receipts: HashMap::new(),
        native_aggregates,
        head_broadcast: None,
        validator_signing_key: None,
        pending_commit: None,
        early_votes: HashMap::new(),
        stake_validator_set: Some(StakeValidatorSetConfig {
            top_n: 2,
            epoch_len: 2,
            min_stake: 100,
            ..Default::default()
        }),
        slashed_equivocations: HashMap::new(),
        observed_votes: HashMap::new(),
        slash_registry: boing_governance::SlashRegistry::new(),
        liveness_miss_streak: HashMap::new(),
        leader_wait_started: None,
    }
}

fn submit_self_transfer(node: &mut BoingNode, key: &SigningKey, nonce: u64, amount: u128) {
    let sender = AccountId(key.verifying_key().to_bytes());
    let tx = Transaction {
        nonce,
        sender,
        payload: TransactionPayload::Transfer {
            to: sender,
            amount,
        },
        access_list: AccessList::new(vec![sender], vec![sender]),
    };
    node.submit_transaction(SignedTransaction::new(tx, key))
        .unwrap();
}

#[test]
fn stake_validator_set_refreshes_at_epoch() {
    let proposer_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(proposer_key.verifying_key().to_bytes());
    let rich = AccountId([0xAAu8; 32]);
    let mid = AccountId([0xBBu8; 32]);
    let poor = AccountId([0xCCu8; 32]);

    let mut node = node_with_accounts(
        &proposer_key,
        vec![(rich, 0, 500), (mid, 0, 200), (poor, 0, 50)],
    );

    assert_eq!(node.consensus.validators(), &[proposer]);

    // Mempool must be non-empty to produce blocks; epoch_len=2 → refresh at height 2.
    submit_self_transfer(&mut node, &proposer_key, 0, 1);
    let _ = node.produce_block_if_ready().expect("block 1");
    assert_eq!(node.chain.height(), 1);
    assert_eq!(node.consensus.validators(), &[proposer]);

    submit_self_transfer(&mut node, &proposer_key, 1, 1);
    let _ = node.produce_block_if_ready().expect("block 2");
    assert_eq!(node.chain.height(), 2);

    // Exactly the top-2 stakers, sorted by id; nothing node-local (the old "keep local proposer" rule
    // made every node compute a different set).
    let want: Vec<AccountId> = { let mut v = vec![rich, mid]; v.sort_by(|a, b| a.0.cmp(&b.0)); v };
    assert_eq!(node.consensus.validators(), want.as_slice());
    assert!(!node.consensus.validators().contains(&proposer));
    assert!(!node.consensus.validators().contains(&poor));
}

#[test]
fn stake_validator_set_ignores_zero_stake_accounts() {
    let proposer_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(proposer_key.verifying_key().to_bytes());
    let zero_stake = AccountId([0xDDu8; 32]);

    let mut node = node_with_accounts(&proposer_key, vec![(zero_stake, 1_000, 0)]);
    node.stake_validator_set = Some(StakeValidatorSetConfig {
        top_n: 1,
        epoch_len: 1,
        min_stake: 100,
        ..Default::default()
    });

    submit_self_transfer(&mut node, &proposer_key, 0, 1);
    let _ = node.produce_block_if_ready().expect("block 1");
    // No positive stake elsewhere → keep current (proposer-only) set.
    assert_eq!(node.consensus.validators(), &[proposer]);
}

#[test]
fn stake_validator_set_respects_min_stake() {
    let proposer_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(proposer_key.verifying_key().to_bytes());
    let below = AccountId([0xEEu8; 32]);
    let above = AccountId([0xFFu8; 32]);

    let mut node = node_with_accounts(
        &proposer_key,
        vec![(below, 0, 50), (above, 0, 500)],
    );
    node.stake_validator_set = Some(StakeValidatorSetConfig {
        top_n: 2,
        epoch_len: 1,
        min_stake: 100,
        ..Default::default()
    });

    submit_self_transfer(&mut node, &proposer_key, 0, 1);
    let _ = node.produce_block_if_ready().expect("block 1");
    assert_eq!(node.consensus.validators(), &[above]);
    let _ = (below, proposer);
}

#[test]
fn bond_then_epoch_includes_new_staker() {
    let proposer_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(proposer_key.verifying_key().to_bytes());
    let other_key = SigningKey::generate(&mut OsRng);
    let other = AccountId(other_key.verifying_key().to_bytes());

    let mut node = node_with_accounts(&proposer_key, vec![(other, 50_000, 0)]);
    node.stake_validator_set = Some(StakeValidatorSetConfig {
        top_n: 1,
        epoch_len: 1,
        min_stake: 10_000,
        ..Default::default()
    });

    let bond = Transaction {
        nonce: 0,
        sender: other,
        payload: TransactionPayload::Bond { amount: 10_000 },
        access_list: AccessList::new(vec![other], vec![other]),
    };
    node.submit_transaction(SignedTransaction::new(bond, &other_key))
        .unwrap();

    let _ = node.produce_block_if_ready().expect("bond block");
    assert_eq!(node.state.get(&other).unwrap().stake, 10_000);

    assert_eq!(node.consensus.validators(), &[other]);
    let _ = proposer;
}

fn stake_cfg(top_n: usize, epoch_len: u64, min_stake: u128, activation_height: u64, bootstrap: Vec<AccountId>) -> StakeValidatorSetConfig {
    StakeValidatorSetConfig { top_n, epoch_len, min_stake, activation_height, bootstrap }
}

/// Before the activation height the static list stays, even at epoch boundaries.
#[test]
fn stake_set_waits_for_activation_height() {
    let key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(key.verifying_key().to_bytes());
    let rich = AccountId([0xAAu8; 32]);
    let mut node = node_with_accounts(&key, vec![(rich, 0, 50_000)]);
    node.stake_validator_set = Some(stake_cfg(21, 1, 10_000, 3, vec![proposer]));
    for nonce in 0..2 {
        submit_self_transfer(&mut node, &key, nonce, 1);
        node.produce_block_if_ready().expect("block");
        assert_eq!(node.consensus.validators(), &[proposer], "height {}", node.chain.height());
    }
    submit_self_transfer(&mut node, &key, 2, 1);
    node.produce_block_if_ready().expect("block 3");
    assert_eq!(node.chain.height(), 3);
    assert_eq!(node.consensus.validators(), &[rich]);
}

/// The live testnet shape: one bootstrap validator, nobody (incl. itself) at 10k stake. Stake mode is on,
/// yet every boundary falls back to the bootstrap list and the chain keeps producing.
#[test]
fn single_validator_without_stake_keeps_producing() {
    let key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(key.verifying_key().to_bytes());
    let below = AccountId([0xB1u8; 32]);
    let mut node = node_with_accounts(&key, vec![(below, 0, 9_999)]);
    node.stake_validator_set = Some(stake_cfg(21, 1, 10_000, 0, vec![proposer]));
    for nonce in 0..5 {
        submit_self_transfer(&mut node, &key, nonce, 1);
        node.produce_block_if_ready().expect("block");
        assert_eq!(node.consensus.validators(), &[proposer]);
    }
    assert_eq!(node.chain.height(), 5);
}

/// The single validator bonds 10k: it becomes the (one-member) stake set and keeps producing; when a second
/// staker shows up at a later boundary it joins, and when that staker unbonds below the minimum it leaves.
#[test]
fn bonded_single_validator_continues_then_others_join_and_leave() {
    let key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_accounts(&key, vec![]);
    node.stake_validator_set = Some(stake_cfg(21, 2, 10_000, 0, vec![proposer]));
    let bond = Transaction { nonce: 0, sender: proposer, payload: TransactionPayload::Bond { amount: 10_000 }, access_list: AccessList::new(vec![proposer], vec![proposer]) };
    node.submit_transaction(SignedTransaction::new(bond, &key)).unwrap();
    node.produce_block_if_ready().expect("block 1");
    submit_self_transfer(&mut node, &key, 1, 1);
    node.produce_block_if_ready().expect("block 2 (boundary)");
    assert_eq!(node.consensus.validators(), &[proposer]);
    submit_self_transfer(&mut node, &key, 2, 1);
    node.produce_block_if_ready().expect("block 3");
    // Another account reaches the minimum mid-epoch: no change until the next boundary.
    let other = AccountId([0x01u8; 32]); // sorts before most keys; set order is by id
    node.state.insert(Account { id: other, state: AccountState { stake: 10_000, ..Default::default() } });
    assert_eq!(node.consensus.validators(), &[proposer]);
    submit_self_transfer(&mut node, &key, 3, 1);
    node.produce_block_if_ready().expect("block 4 (boundary)");
    let want: Vec<AccountId> = { let mut v = vec![proposer, other]; v.sort_by(|a, b| a.0.cmp(&b.0)); v };
    assert_eq!(node.consensus.validators(), want.as_slice());
    // It unbonds 1 unit (stake drops below 10k immediately): out at the next boundary (via restore = same rule).
    node.state.get_mut(&other).unwrap().stake = 9_999;
    assert_eq!(node.restore_stake_validator_set().unwrap(), vec![proposer]);
    assert_eq!(node.consensus.validators(), &[proposer]);
}

/// The set chosen at a boundary is persisted and re-applied after a restart (otherwise a restarted node
/// would fall back to the static list and disagree with its peers).
#[test]
fn stake_set_survives_restart() {
    let key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(key.verifying_key().to_bytes());
    let other = AccountId([0x02u8; 32]);
    let dir = std::env::temp_dir().join(format!("boing-stake-set-{}", hex::encode(&proposer.0[..6])));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = stake_cfg(21, 1, 10_000, 0, vec![proposer]);
    let want: Vec<AccountId> = { let mut v = vec![proposer, other]; v.sort_by(|a, b| a.0.cmp(&b.0)); v };
    {
        let mut node = BoingNode::with_data_dir_and_validators(Some(&dir), vec![proposer], proposer, Some(key.clone())).unwrap();
        node.stake_validator_set = Some(cfg.clone());
        node.state.get_mut(&proposer).unwrap().stake = 20_000;
        node.state.insert(Account { id: other, state: AccountState { stake: 10_000, ..Default::default() } });
        submit_self_transfer(&mut node, &key, 0, 1);
        node.produce_block_if_ready().expect("block 1");
        assert_eq!(node.consensus.validators(), want.as_slice());
    }
    let mut restarted = BoingNode::with_data_dir_and_validators(Some(&dir), vec![proposer], proposer, Some(key.clone())).unwrap();
    assert_eq!(restarted.chain.height(), 1);
    assert_eq!(restarted.consensus.validators(), &[proposer], "static list before restore");
    restarted.stake_validator_set = Some(cfg.clone());
    assert_eq!(restarted.restore_stake_validator_set().unwrap(), want);
    assert_eq!(restarted.consensus.validators(), want.as_slice());
    // Before activation nothing is restored.
    let mut early = BoingNode::with_data_dir_and_validators(Some(&dir), vec![proposer], proposer, Some(key)).unwrap();
    early.stake_validator_set = Some(stake_cfg(21, 1, 10_000, 5, vec![proposer]));
    assert!(early.restore_stake_validator_set().is_none());
    assert_eq!(early.consensus.validators(), &[proposer]);
    let _ = std::fs::remove_dir_all(&dir);
}
