//! Equivocation detection burns a fraction of the offender's active stake.

use std::collections::HashMap;

use boing_execution::{Vm, VmExecutionContext};
use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, Block, BlockHeader, ConsensusVote,
    EquivocationEvidence, Hash, Transaction, TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn ctx(h: u64) -> VmExecutionContext {
    VmExecutionContext {
        block_height: h,
        block_timestamp: 0,
        native_value: false,
    }
}

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
        slash_registry: boing_governance::SlashRegistry::new(),
        liveness_miss_streak: HashMap::new(),
        leader_wait_started: None,
        open_validator_params: None,
        anchor_fallback_timeout: boing_node::node::DEFAULT_ANCHOR_FALLBACK_TIMEOUT,
        anchor_fallback_wait: None,
        last_local_vote_round: None,
        media_blob_store: None,
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
            blobs_root: None,
        },
        transactions: vec![],
        signatures: vec![],
        priority_tips: vec![],
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
    // Phase 15: consensus book seeded at slash-apply time.
    let slash_id = *node.slashed_equivocations.get(&(offender, 9)).unwrap();
    let cons = node.state.slash_appeals().get_slash(slash_id).expect("consensus row");
    assert!(matches!(
        cons.reason,
        boing_state::ConsensusSlashReason::Equivocation
    ));
    assert_eq!(cons.amount, 5_000);
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

#[test]
fn consensus_slash_appeal_txs_restore_stake_when_approved() {
    let v1 = AccountId([1u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(vec![v1, offender, v3, v4], &[(offender, 10_000)]);
    // Fund offender for SubmitSlashAppeal nonce / gas path.
    node.state.get_mut(&offender).unwrap().balance = 1_000_000;

    let a = ConsensusVote::sign(7, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(7, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
    assert!(node.on_equivocation_evidence(ev));
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);

    let slash_id = *node.slashed_equivocations.get(&(offender, 7)).unwrap();
    // Operator RPC refused — consensus book owns the slash (Phase 15).
    assert!(node
        .submit_slash_appeal(slash_id, b"false positive".to_vec())
        .is_err());

    // Media inactive: Submit/Resolve still execute (always-on admission).
    // Phase 16: M=4 needs ⌈2*4/3⌉=3 approve votes to finalize.
    let vm = Vm::new();
    assert!(vm.media_blobs_from().is_none());
    for id in [v3, v4] {
        node.state.insert(Account {
            id,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
    }
    node.state
        .set_slash_appeal_resolvers(Some(vec![v1, offender, v3, v4]));
    vm.execute_with_context(
        &Transaction {
            nonce: 0,
            sender: offender,
            payload: TransactionPayload::SubmitSlashAppeal {
                slash_id,
                evidence: b"false positive".to_vec(),
            },
            access_list: AccessList::new(vec![offender], vec![offender]),
        },
        &mut node.state,
        ctx(node.chain.height().saturating_add(1)),
    )
    .unwrap();
    let appeal_id = node.state.slash_appeals().list_appeals()[0].id;
    // First approve: still pending.
    vm.execute_with_context(
        &Transaction {
            nonce: 0,
            sender: v1,
            payload: TransactionPayload::ResolveSlashAppeal {
                appeal_id,
                approved: true,
            },
            access_list: AccessList::new(vec![v1], vec![v1]),
        },
        &mut node.state,
        ctx(node.chain.height().saturating_add(2)),
    )
    .unwrap();
    assert!(!node.state.slash_appeals().is_reversed(slash_id));
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);
    // Second + third approve → finalize + restore.
    for (i, voter) in [v3, v4].into_iter().enumerate() {
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: voter,
                payload: TransactionPayload::ResolveSlashAppeal {
                    appeal_id,
                    approved: true,
                },
                access_list: AccessList::new(vec![voter], vec![voter]),
            },
            &mut node.state,
            ctx(node.chain.height().saturating_add(3 + i as u64)),
        )
        .unwrap();
    }

    assert!(node.state.slash_appeals().is_reversed(slash_id));
    assert_eq!(node.state.get(&offender).unwrap().stake, 10_000);
    assert_eq!(
        node.state
            .get(&boing_tokenomics::FEE_BURN_SINK)
            .unwrap()
            .balance,
        0
    );

    // Further resolve after finalize → rejected; stake stays restored (no double credit).
    let err = vm.execute_with_context(
        &Transaction {
            nonce: 1,
            sender: v1,
            payload: TransactionPayload::ResolveSlashAppeal {
                appeal_id,
                approved: true,
            },
            access_list: AccessList::new(vec![v1], vec![v1]),
        },
        &mut node.state,
        ctx(node.chain.height().saturating_add(10)),
    );
    assert!(err.is_err());
    assert_eq!(node.state.get(&offender).unwrap().stake, 10_000);
}

#[test]
fn consensus_slash_appeal_rejected_keeps_burn() {
    let v1 = AccountId([1u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(vec![v1, offender, v3, v4], &[(offender, 10_000)]);
    node.state.get_mut(&offender).unwrap().balance = 1_000_000;

    let a = ConsensusVote::sign(4, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(4, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
    assert!(node.on_equivocation_evidence(ev));
    let slash_id = *node.slashed_equivocations.get(&(offender, 4)).unwrap();

    // Phase 16: M=4 needs 3 reject votes to finalize Rejected.
    let vm = Vm::new();
    for id in [v3, v4] {
        node.state.insert(Account {
            id,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
    }
    node.state
        .set_slash_appeal_resolvers(Some(vec![v1, offender, v3, v4]));
    vm.execute_with_context(
        &Transaction {
            nonce: 0,
            sender: offender,
            payload: TransactionPayload::SubmitSlashAppeal {
                slash_id,
                evidence: vec![],
            },
            access_list: AccessList::new(vec![offender], vec![offender]),
        },
        &mut node.state,
        ctx(node.chain.height().saturating_add(1)),
    )
    .unwrap();
    let appeal_id = node.state.slash_appeals().list_appeals()[0].id;
    for (i, voter) in [v1, v3, v4].into_iter().enumerate() {
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: voter,
                payload: TransactionPayload::ResolveSlashAppeal {
                    appeal_id,
                    approved: false,
                },
                access_list: AccessList::new(vec![voter], vec![voter]),
            },
            &mut node.state,
            ctx(node.chain.height().saturating_add(2 + i as u64)),
        )
        .unwrap();
    }
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);
    assert!(!node.state.slash_appeals().is_reversed(slash_id));
}

#[test]
fn failed_serve_and_equivocation_slash_ids_do_not_collide() {
    let v1 = AccountId([1u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(vec![v1, offender, v3, v4], &[(offender, 10_000)]);

    // Seed a FailedServe consensus row first (shared allocator).
    let fs_id = node
        .state
        .slash_appeals_mut()
        .record_slash(
            offender,
            42,
            boing_state::ConsensusSlashReason::BlobFailedServe,
            1,
            1000,
        )
        .unwrap();
    assert_eq!(fs_id, 1);

    let a = ConsensusVote::sign(5, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(5, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
    assert!(node.on_equivocation_evidence(ev));
    let eq_id = *node.slashed_equivocations.get(&(offender, 5)).unwrap();
    assert_ne!(eq_id, fs_id);
    assert_eq!(eq_id, 2);
    assert_eq!(node.state.slash_appeals().list_slashes().len(), 2);
}

#[tokio::test]
async fn list_slash_records_rpc_returns_equivocation_entry() {
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use boing_node::rpc::rpc_router;
    use boing_node::security::RateLimitConfig;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let v1 = AccountId([1u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(vec![v1, offender, v3, v4], &[(offender, 10_000)]);

    let a = ConsensusVote::sign(11, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(11, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
    assert!(node.on_equivocation_evidence(ev));

    let app = rpc_router(
        Arc::new(RwLock::new(node)),
        &RateLimitConfig::default(),
        None,
        None,
        None,
    );
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "boing_listSlashRecords",
                "params": []
            })
            .to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let slashes = v["result"]["slashes"].as_array().expect("slashes");
    assert_eq!(slashes.len(), 1);
    assert_eq!(slashes[0]["reason"], "equivocation");
    assert_eq!(slashes[0]["amount"], "5000");
    assert_eq!(slashes[0]["source"], "consensus");
    assert_eq!(v["result"]["persisted"], false);
}
