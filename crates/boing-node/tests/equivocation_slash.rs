//! Equivocation detection burns a fraction of the offender's active stake.

use std::collections::HashMap;

use boing_node::node::BoingNode;
use boing_primitives::{
    Account, AccountId, AccountState, Block, BlockHeader, ConsensusVote, EquivocationEvidence, Hash,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn node_with_validators(validators: Vec<AccountId>, stakes: &[(AccountId, u128)]) -> BoingNode {
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
        validator_signing_key: None,
        pending_commit: None,
        early_votes: HashMap::new(),
        stake_validator_set: None,
        slashed_equivocations: HashMap::new(),
        observed_votes: HashMap::new(),
    }
}

#[test]
fn equivocation_slash_burns_half_active_stake() {
    let v1 = AccountId([1u8; 32]);
    let v2 = AccountId([2u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let mut node = node_with_validators(vec![v1, v2, v3, v4], &[(v2, 10_000)]);

    let parent = node.chain.latest_hash();
    let height = node.chain.height() + 1;
    node.consensus.sync_round(height);
    let leader = node.consensus.leader(height);
    let block = Block {
        header: BlockHeader {
            parent_hash: parent,
            height,
            timestamp: 0,
            proposer: leader,
            tx_root: Hash::ZERO,
            receipts_root: Hash::ZERO,
            state_root: Hash::ZERO,
        },
        transactions: vec![],
    };
    node.consensus.propose(block.clone()).unwrap();
    let good = block.hash();
    assert!(node.consensus.vote(good, v2).unwrap().is_none());

    let bad = Hash([0xABu8; 32]);
    let err = node.consensus.vote(bad, v2).unwrap_err();
    match err {
        boing_consensus::ConsensusError::Equivocation { validator: v, round } => {
            assert_eq!(v, v2);
            let burned = boing_tokenomics::slash_equivocation_stake(&mut node.state, &v);
            assert_eq!(burned, 5_000);
            assert_eq!(node.state.get(&v2).unwrap().stake, 5_000);
            assert_eq!(
                node.state
                    .get(&boing_tokenomics::FEE_BURN_SINK)
                    .unwrap()
                    .balance,
                5_000
            );
            assert_eq!(round, height);
        }
        other => panic!("expected Equivocation, got {other:?}"),
    }
}

#[test]
fn gossiped_equivocation_evidence_slashes_once() {
    let v1 = AccountId([1u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(vec![v1, offender, v3, v4], &[(offender, 10_000)]);

    let a = ConsensusVote::sign(9, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(9, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();

    assert!(node.on_equivocation_evidence(ev.clone()));
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);
    assert!(!node.on_equivocation_evidence(ev));
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);
}

#[test]
fn conflicting_observed_votes_slash_via_on_consensus_vote() {
    let v1 = AccountId([1u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(vec![v1, offender, v3, v4], &[(offender, 8_000)]);

    let a = ConsensusVote::sign(3, Hash([9u8; 32]), &key);
    let b = ConsensusVote::sign(3, Hash([8u8; 32]), &key);
    assert!(node.on_consensus_vote(a).is_none());
    assert!(node.on_consensus_vote(b).is_none());
    assert_eq!(node.state.get(&offender).unwrap().stake, 4_000);
    assert!(node
        .slashed_equivocations
        .contains_key(&(offender, 3)));
}
