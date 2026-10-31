//! Proposal timeout / view-change (issue #61 item 4): a stuck pending_commit must not
//! block produce_block_if_ready forever; abandoning advances the consensus view.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, Block, BlockHeader, Hash, SignedTransaction,
    Transaction, TransactionPayload,
};
use boing_state::StateStore;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn multi_validator_node(local_key: SigningKey, others: &[AccountId]) -> BoingNode {
    let local = AccountId(local_key.verifying_key().to_bytes());
    let mut validators = vec![local];
    validators.extend_from_slice(others);
    let genesis = boing_node::chain::ChainState::genesis(local);
    let chain = boing_node::chain::ChainState::from_genesis(genesis.clone());
    let mut consensus = boing_consensus::ConsensusEngine::new(validators);
    let _ = consensus.propose_and_commit(genesis);

    let mut state = StateStore::new();
    state.insert(Account {
        id: local,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
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
        validator_signing_key: Some(local_key),
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
        pending_since: None,
        proposal_timeout: boing_node::node::DEFAULT_PROPOSAL_TIMEOUT,
        media_blob_store: None,
        bls_dual_key_registry: boing_primitives::BlsDualKeyRegistry::new(),
        account_tx_cache_key: boing_node::account_tx_index::alloc_cache_key(),
    }
}

#[test]
fn proposal_timeout_clears_pending_and_advances_view() {
    let local_key = SigningKey::generate(&mut OsRng);
    let v2 = AccountId([2u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let mut node = multi_validator_node(local_key, &[v2, v3, v4]);

    // Install a fake stuck pending commit at the next height (as if we accepted a proposal
    // that never reached quorum).
    let height = node.chain.height().saturating_add(1);
    node.consensus.sync_round(height);
    assert_eq!(node.consensus.view(), 0);
    let leader = node.consensus.leader(height);
    let stuck = Block {
        header: BlockHeader {
            parent_hash: node.chain.latest_hash(),
            height,
            timestamp: 1,
            proposer: leader,
            tx_root: Hash::ZERO,
            receipts_root: Hash::ZERO,
            state_root: Hash::ZERO,
            blobs_root: None,
        },
        transactions: vec![],
        signatures: vec![],
        priority_tips: vec![],
    };
    node.consensus.accept_proposal(stuck.clone()).unwrap();
    node.pending_commit = Some(boing_node::node::PendingCommit {
        block: stuck,
        receipts: vec![],
        new_state: StateStore::new(),
    });
    node.last_local_vote_round = Some(height);
    node.pending_since = Some(Instant::now() - node.proposal_timeout - Duration::from_secs(1));

    // Before the timeout tick, produce_block_if_ready refuses while pending.
    // After tick_proposal_timeout (also invoked inside produce_block_if_ready), pending clears.
    assert!(node.pending_commit.is_some());
    node.tick_proposal_timeout();
    assert!(node.pending_commit.is_none());
    assert!(node.last_local_vote_round.is_none());
    assert_eq!(node.consensus.view(), 1);
    assert!(node.consensus.pending_block().is_none());
}

#[test]
fn after_timeout_new_view_leader_can_propose() {
    let keys: Vec<SigningKey> = (0..4).map(|_| SigningKey::generate(&mut OsRng)).collect();
    let validators: Vec<AccountId> = keys
        .iter()
        .map(|k| AccountId(k.verifying_key().to_bytes()))
        .collect();

    // Build a node whose local identity is validators[1] — leader of view 1 at height 1.
    let mut node = multi_validator_node(
        keys[1].clone(),
        &[validators[0], validators[2], validators[3]],
    );
    // Rebuild consensus with a deterministic validator order matching `validators`.
    node.consensus = boing_consensus::ConsensusEngine::new(validators.clone());
    let genesis = boing_node::chain::ChainState::genesis(validators[0]);
    let _ = node.consensus.propose_and_commit(genesis);
    node.consensus.sync_round(1);
    node.producer = boing_node::block_producer::BlockProducer::new(validators[1]).with_max_txs(100);
    node.validator_signing_key = Some(keys[1].clone());

    // Simulate view-0 leader (validators[1] at round 1? leader(1)=validators[1] with order above:
    // validators[0] is index 0 → leader(0); leader(1)=validators[1]. View 0 at height 1 → v[1].
    // After abandon, view 1 → leader = validators[(1+1)%4]=validators[2].
    // So make local = validators[2] to propose after timeout.
    node.producer = boing_node::block_producer::BlockProducer::new(validators[2]).with_max_txs(100);
    node.validator_signing_key = Some(keys[2].clone());

    // Stuck proposal from view-0 leader validators[1].
    let stuck = Block {
        header: BlockHeader {
            parent_hash: node.chain.latest_hash(),
            height: 1,
            timestamp: 1,
            proposer: validators[1],
            tx_root: Hash::ZERO,
            receipts_root: Hash::ZERO,
            state_root: Hash::ZERO,
            blobs_root: None,
        },
        transactions: vec![],
        signatures: vec![],
        priority_tips: vec![],
    };
    node.consensus.accept_proposal(stuck.clone()).unwrap();
    node.pending_commit = Some(boing_node::node::PendingCommit {
        block: stuck,
        receipts: vec![],
        new_state: StateStore::new(),
    });
    node.pending_since = Some(Instant::now() - Duration::from_secs(60));
    node.last_local_vote_round = Some(1);

    // Fund a tx so the new leader has mempool work.
    let to = AccountId([0xee; 32]);
    let funder = keys[0].clone();
    let funder_id = validators[0];
    node.state.insert(Account {
        id: funder_id,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
    let tx = Transaction {
        nonce: 0,
        sender: funder_id,
        payload: TransactionPayload::Transfer { to, amount: 1 },
        access_list: AccessList::new(vec![funder_id, to], vec![funder_id, to]),
    };
    node.mempool
        .insert(SignedTransaction::new(tx, &funder))
        .unwrap();

    assert_eq!(node.consensus.leader(1), validators[1]);
    node.tick_proposal_timeout();
    assert_eq!(node.consensus.view(), 1);
    assert_eq!(node.consensus.leader(1), validators[2]);
    assert_eq!(node.producer.proposer(), validators[2]);

    // New view leader can propose (pending was cleared).
    let produced = node.produce_block_if_ready();
    assert!(
        node.pending_commit.is_some() || produced.is_some(),
        "view-1 leader should propose after timeout"
    );
    if let Some(pending) = &node.pending_commit {
        assert_eq!(pending.block.header.proposer, validators[2]);
        assert_eq!(pending.block.header.height, 1);
    }
}
