//! Equivocation slashing is block-ordered: two nodes that observe the same equivocation at
//! different times keep the same state root, and the slash lands once, in the block that
//! carries the `ReportEquivocation` tx.

use std::collections::HashMap;

use boing_node::chain::ChainState;
use boing_node::node::BoingNode;
use boing_primitives::{
    Account, AccountId, AccountState, Block, ConsensusVote, EquivocationEvidence, Hash,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn id(k: &SigningKey) -> AccountId {
    AccountId(k.verifying_key().to_bytes())
}

fn validator_node(
    validators: &[AccountId],
    key: &SigningKey,
    genesis: &Block,
    offender: AccountId,
) -> BoingNode {
    let local = id(key);
    let mut consensus = boing_consensus::ConsensusEngine::new(validators.to_vec());
    let _ = consensus.propose_and_commit(genesis.clone());
    let mut state = boing_state::StateStore::new();
    for v in validators {
        state.insert(Account {
            id: *v,
            state: AccountState {
                balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
                stake: if *v == offender { 10_000 } else { 0 },
                ..Default::default()
            },
        });
    }
    let native_aggregates = state.compute_native_aggregates();
    BoingNode {
        chain: ChainState::from_genesis(genesis.clone()),
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
        validator_signing_key: Some(key.clone()),
        pending_commit: None,
        early_votes: HashMap::new(),
        stake_validator_set: None,
        slashed_equivocations: HashMap::new(),
        observed_votes: HashMap::new(),
        slash_registry: boing_governance::SlashRegistry::new(),
        liveness_miss_streak: HashMap::new(),
        leader_wait_started: None,
        open_validator_params: None,
        anchor_fallback_timeout: boing_node::node::DEFAULT_ANCHOR_FALLBACK_TIMEOUT,
        anchor_fallback_wait: None,
        last_local_vote_round: None,
        media_blob_store: None,
        bls_dual_key_registry: boing_primitives::BlsDualKeyRegistry::new(),
    }
}

fn equivocation_rows(node: &BoingNode) -> usize {
    node.state
        .slash_appeals()
        .list_slashes()
        .iter()
        .filter(|s| matches!(s.reason, boing_state::ConsensusSlashReason::Equivocation))
        .count()
}

#[test]
fn nodes_observing_equivocation_at_different_times_keep_same_state_root() {
    let key_a = SigningKey::generate(&mut OsRng);
    let key_b = SigningKey::generate(&mut OsRng);
    let key_c = SigningKey::generate(&mut OsRng);
    let key_d = SigningKey::generate(&mut OsRng);
    let (a, b, c, d) = (id(&key_a), id(&key_b), id(&key_c), id(&key_d));
    // Round-robin: height 1 leader is validators[1] = a.
    let validators = vec![b, a, c, d];
    let genesis = ChainState::genesis(b);
    let mut node_a = validator_node(&validators, &key_a, &genesis, c);
    let mut node_b = validator_node(&validators, &key_b, &genesis, c);
    assert_eq!(node_a.consensus.leader(1), a);
    assert_eq!(node_a.state.state_root(), node_b.state.state_root());

    // c signs two different blocks in round 1.
    let ev = EquivocationEvidence::try_from_votes(
        ConsensusVote::sign(1, Hash([0x11; 32]), &key_c),
        ConsensusVote::sign(1, Hash([0x22; 32]), &key_c),
    )
    .unwrap();

    // Node A sees it first. Detection must not touch committed state.
    assert!(node_a.on_equivocation_evidence(ev.clone()));
    assert_eq!(node_a.state.get(&c).unwrap().stake, 10_000);
    assert_eq!(node_a.state.state_root(), node_b.state.state_root());

    // A leads height 1 and includes its report.
    assert!(node_a.produce_block_if_ready().is_none());
    let block = node_a.pending_commit.as_ref().unwrap().block.clone();
    assert!(block.transactions.iter().any(|tx| matches!(
        &tx.payload,
        TransactionPayload::ReportEquivocation { evidence } if *evidence == ev
    )));

    // B has not seen the evidence yet and still executes the block to the same root.
    node_b.handle_network_block(&block).unwrap();

    let hash = block.hash();
    assert!(node_a
        .on_consensus_vote(ConsensusVote::sign(1, hash, &key_b))
        .is_none());
    assert_eq!(
        node_a.on_consensus_vote(ConsensusVote::sign(1, hash, &key_d)),
        Some(hash)
    );
    assert!(node_b
        .on_consensus_vote(ConsensusVote::sign(1, hash, &key_a))
        .is_none());
    assert_eq!(
        node_b.on_consensus_vote(ConsensusVote::sign(1, hash, &key_d)),
        Some(hash)
    );

    // B sees the same evidence only after the commit: nothing left to report.
    assert!(!node_b.on_equivocation_evidence(ev));
    assert!(node_b.mempool.is_empty());

    for node in [&mut node_a, &mut node_b] {
        assert_eq!(node.chain.height(), 1);
        assert_eq!(node.state.get(&c).unwrap().stake, 5_000);
        assert_eq!(equivocation_rows(node), 1);
    }
    assert_eq!(block.header.state_root, node_a.state.state_root());
    assert_eq!(node_a.state.state_root(), node_b.state.state_root());
}
