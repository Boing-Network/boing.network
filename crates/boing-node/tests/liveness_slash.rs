//! Leader-timeout liveness slash (multi-validator).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, SignedTransaction, Transaction, TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn node_with_validators(
    validators: Vec<AccountId>,
    stakes: &[(AccountId, u128)],
    local_key: Option<SigningKey>,
) -> BoingNode {
    let local = validators[0];
    let genesis = boing_node::chain::ChainState::genesis(local);
    let chain = boing_node::chain::ChainState::from_genesis(genesis.clone());
    let mut consensus = boing_consensus::ConsensusEngine::new(validators);
    let _ = consensus.propose_and_commit(genesis);

    let mut state = boing_state::StateStore::new();
    state.insert(Account {
        id: local,
        state: AccountState {
            balance: 1_000_000,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
    for (id, stake) in stakes {
        state.insert(Account {
            id: *id,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: *stake,
                ..Default::default()
            },
        });
    }
    let native_aggregates = state.compute_native_aggregates();
    BoingNode {
        chain,
        consensus,
        state,
        executor: boing_execution::BlockExecutor::new(),
        producer: boing_node::block_producer::BlockProducer::new(local).with_max_txs(100),
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
        validator_signing_key: local_key,
        pending_commit: None,
        early_votes: HashMap::new(),
        stake_validator_set: None,
        slashed_equivocations: HashMap::new(),
        observed_votes: HashMap::new(),
        slash_registry: boing_governance::SlashRegistry::new(),
        liveness_miss_streak: HashMap::new(),
        leader_wait_started: None,
    }
}

#[test]
fn leader_timeout_slash_after_miss_threshold() {
    let local_key = SigningKey::generate(&mut OsRng);
    let v1 = AccountId(local_key.verifying_key().to_bytes());
    let v2 = AccountId([2u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let mut node = node_with_validators(vec![v1, v2, v3, v4], &[(v2, 10_000)], Some(local_key.clone()));

    // Pending work so idle networks do not false-positive.
    let to = AccountId([9u8; 32]);
    let tx = Transaction {
        nonce: 0,
        sender: v1,
        payload: TransactionPayload::Transfer { to, amount: 1 },
        access_list: AccessList::new(vec![v1, to], vec![v1, to]),
    };
    node.submit_transaction(SignedTransaction::new(tx, &local_key))
        .unwrap();

    let next_h = node.chain.height() + 1;
    assert_eq!(node.consensus.leader(next_h), v2);
    assert_ne!(node.producer.proposer(), v2);

    for _ in 0..boing_tokenomics::LIVENESS_MISS_THRESHOLD {
        node.leader_wait_started = Some((
            next_h,
            Instant::now() - Duration::from_secs(boing_tokenomics::LIVENESS_LEADER_TIMEOUT_SECS + 1),
        ));
        node.tick_liveness();
    }

    assert_eq!(node.state.get(&v2).unwrap().stake, 9_000); // 10% of 10_000
    assert_eq!(
        node.state
            .get(&boing_tokenomics::FEE_BURN_SINK)
            .unwrap()
            .balance,
        1_000
    );
    assert!(node
        .slash_registry
        .list_slashes()
        .iter()
        .any(|s| s.reason == boing_governance::SlashReason::Liveness && s.amount == 1_000));
}

#[test]
fn tick_liveness_noop_when_local_is_leader() {
    let local_key = SigningKey::generate(&mut OsRng);
    let v1 = AccountId(local_key.verifying_key().to_bytes());
    // Single-validator: always local leader; tick must no-op.
    let mut node = node_with_validators(vec![v1], &[], Some(local_key));
    node.tick_liveness();
    assert!(node.liveness_miss_streak.is_empty());
    assert!(node.slash_registry.list_slashes().is_empty());
}
