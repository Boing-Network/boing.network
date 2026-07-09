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

    let set = node.consensus.validators();
    assert!(set.contains(&rich), "top staker should be in set: {set:?}");
    assert!(set.contains(&mid), "second staker should be in set: {set:?}");
    assert!(
        set.contains(&proposer),
        "local proposer is always retained: {set:?}"
    );
    assert!(!set.contains(&poor), "third staker should be excluded: {set:?}");
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
    });

    submit_self_transfer(&mut node, &proposer_key, 0, 1);
    let _ = node.produce_block_if_ready().expect("block 1");
    let set = node.consensus.validators();
    assert!(set.contains(&above), "{set:?}");
    assert!(!set.contains(&below), "{set:?}");
    assert!(set.contains(&proposer), "{set:?}");
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

    let set = node.consensus.validators();
    assert!(set.contains(&other), "bonded account should enter set: {set:?}");
    assert!(
        set.contains(&proposer),
        "local proposer retained: {set:?}"
    );
}
