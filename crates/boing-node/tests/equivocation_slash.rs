//! Equivocation detection queues a consensus-ordered ReportEquivocation tx;
//! stake burns only during block / VM execution.

use std::collections::HashMap;

use boing_execution::{Vm, VmExecutionContext};
use boing_node::node::BoingNode;
use boing_primitives::{
    sign_slash_appeal_resolve_ballot, AccessList, Account, AccountId, AccountState, Block,
    BlockHeader, ConsensusVote, EquivocationEvidence, Hash, Transaction, TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn ctx(h: u64) -> VmExecutionContext {
    VmExecutionContext {
        block_height: h,
        block_timestamp: 0,
        native_value: false,
        vm_hardening: false,
        blake3_opcode: false,
        jump_strict: true,
    }
}

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
        governance: boing_governance::Governance::default(),
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

fn apply_report_equivocation(
    node: &mut BoingNode,
    reporter: AccountId,
    reporter_key: &SigningKey,
    evidence: EquivocationEvidence,
) {
    let nonce = node.state.get(&reporter).map(|s| s.nonce).unwrap_or(0);
    let offender = evidence.validator();
    let burn = boing_tokenomics::FEE_BURN_SINK;
    let tx = Transaction {
        nonce,
        sender: reporter,
        payload: TransactionPayload::ReportEquivocation { evidence },
        access_list: AccessList::new(
            vec![reporter, offender, burn],
            vec![reporter, offender, burn],
        ),
    };
    let vm = Vm::new();
    vm.execute_with_context(&tx, &mut node.state, ctx(1))
        .expect("ReportEquivocation executes");
    let _ = reporter_key; // nonce already taken from state; signature not required for VM path
}

#[test]
fn equivocation_slash_burns_half_active_stake() {
    let v1 = AccountId([1u8; 32]);
    let v2 = AccountId([2u8; 32]);
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let mut node = node_with_validators(vec![v1, v2, v3, v4], &[(v2, 10_000)], None);

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
        boing_consensus::ConsensusError::Equivocation {
            validator: v,
            round,
        } => {
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
fn gossiped_equivocation_evidence_queues_without_mutating_state() {
    let local_key = SigningKey::generate(&mut OsRng);
    let v1 = AccountId(local_key.verifying_key().to_bytes());
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(
        vec![v1, offender, v3, v4],
        &[(offender, 10_000)],
        Some(local_key.clone()),
    );

    let a = ConsensusVote::sign(9, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(9, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();

    assert!(node.on_equivocation_evidence(ev.clone()));
    // Detection must not change committed stake (issue #61 item 1).
    assert_eq!(node.state.get(&offender).unwrap().stake, 10_000);
    assert!(node.slashed_equivocations.contains_key(&(offender, 9)));
    assert!(!node.on_equivocation_evidence(ev.clone()));

    apply_report_equivocation(&mut node, v1, &local_key, ev);
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);
    let cons = node
        .state
        .slash_appeals()
        .find_equivocation_at_round(&offender, 9)
        .expect("consensus row");
    assert!(matches!(
        cons.reason,
        boing_state::ConsensusSlashReason::Equivocation
    ));
    assert_eq!(cons.amount, 5_000);
}

#[test]
fn report_equivocation_is_idempotent_across_two_state_clones() {
    let local_key = SigningKey::generate(&mut OsRng);
    let reporter = AccountId(local_key.verifying_key().to_bytes());
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());

    let a = ConsensusVote::sign(4, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(4, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();

    let mut state_a = boing_state::StateStore::new();
    state_a.insert(Account {
        id: reporter,
        state: AccountState {
            balance: 1_000_000,
            ..Default::default()
        },
    });
    state_a.insert(Account {
        id: offender,
        state: AccountState {
            stake: 10_000,
            ..Default::default()
        },
    });
    let mut state_b = state_a.snapshot();

    let burn = boing_tokenomics::FEE_BURN_SINK;
    let mk_tx = |nonce: u64, evidence: EquivocationEvidence| Transaction {
        nonce,
        sender: reporter,
        payload: TransactionPayload::ReportEquivocation { evidence },
        access_list: AccessList::new(
            vec![reporter, offender, burn],
            vec![reporter, offender, burn],
        ),
    };
    let vm = Vm::new();
    let tx = mk_tx(0, ev.clone());
    vm.execute_with_context(&tx, &mut state_a, ctx(1)).unwrap();
    vm.execute_with_context(&tx, &mut state_b, ctx(1)).unwrap();
    assert_eq!(
        state_a.state_root_at_height(1),
        state_b.state_root_at_height(1),
        "identical ReportEquivocation txs must yield identical roots"
    );
    assert_eq!(state_a.get(&offender).unwrap().stake, 5_000);

    // Second inclusion is a no-op burn but still bumps nonce — same on both clones.
    let tx2 = mk_tx(1, ev);
    vm.execute_with_context(&tx2, &mut state_a, ctx(2)).unwrap();
    vm.execute_with_context(&tx2, &mut state_b, ctx(2)).unwrap();
    assert_eq!(state_a.get(&offender).unwrap().stake, 5_000);
    assert_eq!(
        state_a.state_root_at_height(2),
        state_b.state_root_at_height(2)
    );
}

#[test]
fn conflicting_observed_votes_queue_report_via_on_consensus_vote() {
    let local_key = SigningKey::generate(&mut OsRng);
    let v1 = AccountId(local_key.verifying_key().to_bytes());
    let v3 = AccountId([3u8; 32]);
    let v4 = AccountId([4u8; 32]);
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(
        vec![v1, offender, v3, v4],
        &[(offender, 8_000)],
        Some(local_key.clone()),
    );

    let a = ConsensusVote::sign(3, Hash([9u8; 32]), &key);
    let b = ConsensusVote::sign(3, Hash([8u8; 32]), &key);
    assert!(node.on_consensus_vote(a).is_none());
    assert!(node.on_consensus_vote(b).is_none());
    assert_eq!(node.state.get(&offender).unwrap().stake, 8_000);
    assert!(node.slashed_equivocations.contains_key(&(offender, 3)));

    let ev = EquivocationEvidence::try_from_votes(
        ConsensusVote::sign(3, Hash([9u8; 32]), &key),
        ConsensusVote::sign(3, Hash([8u8; 32]), &key),
    )
    .unwrap();
    apply_report_equivocation(&mut node, v1, &local_key, ev);
    assert_eq!(node.state.get(&offender).unwrap().stake, 4_000);
}

#[test]
fn consensus_slash_appeal_txs_restore_stake_when_approved() {
    // Phase 19: multi-pin committee must cast via attested 32.
    let key1 = SigningKey::generate(&mut OsRng);
    let key3 = SigningKey::generate(&mut OsRng);
    let key4 = SigningKey::generate(&mut OsRng);
    let key = SigningKey::generate(&mut OsRng);
    let v1 = AccountId(key1.verifying_key().to_bytes());
    let v3 = AccountId(key3.verifying_key().to_bytes());
    let v4 = AccountId(key4.verifying_key().to_bytes());
    let offender = AccountId(key.verifying_key().to_bytes());
    let chain_id = 6913u64;
    let mut node = node_with_validators(
        vec![v1, offender, v3, v4],
        &[(offender, 10_000)],
        Some(key1.clone()),
    );
    node.state.get_mut(&offender).unwrap().balance = 1_000_000;

    let a = ConsensusVote::sign(7, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(7, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
    assert!(node.on_equivocation_evidence(ev.clone()));
    apply_report_equivocation(&mut node, v1, &key1, ev);
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);

    let slash_id = node
        .state
        .slash_appeals()
        .find_equivocation_at_round(&offender, 7)
        .expect("slash")
        .id;
    assert!(node
        .submit_slash_appeal(slash_id, b"false positive".to_vec())
        .is_err());

    let mut vm = Vm::new();
    vm.set_registration_proof_rule(Some(chain_id), 0);
    node.state.get_mut(&v1).unwrap().stake = 5_000;
    for id in [v3, v4] {
        node.state.insert(Account {
            id,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 5_000,
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
    let sig1 = sign_slash_appeal_resolve_ballot(chain_id, appeal_id, slash_id, true, &key1);
    let v1_nonce = node.state.get(&v1).unwrap().nonce;
    vm.execute_with_context(
        &Transaction {
            nonce: v1_nonce,
            sender: v1,
            payload: TransactionPayload::ResolveSlashAppealAttested {
                appeal_id,
                approved: true,
                ballot_signature: sig1,
            },
            access_list: AccessList::new(vec![v1], vec![v1]),
        },
        &mut node.state,
        ctx(node.chain.height().saturating_add(2)),
    )
    .unwrap();
    assert!(!node.state.slash_appeals().is_reversed(slash_id));
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);
    for (i, (voter, sk)) in [(v3, &key3), (v4, &key4)].into_iter().enumerate() {
        let sig = sign_slash_appeal_resolve_ballot(chain_id, appeal_id, slash_id, true, sk);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: voter,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id,
                    approved: true,
                    ballot_signature: sig,
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
}

#[test]
fn consensus_slash_appeal_rejected_keeps_burn() {
    let key1 = SigningKey::generate(&mut OsRng);
    let key3 = SigningKey::generate(&mut OsRng);
    let key4 = SigningKey::generate(&mut OsRng);
    let key = SigningKey::generate(&mut OsRng);
    let v1 = AccountId(key1.verifying_key().to_bytes());
    let v3 = AccountId(key3.verifying_key().to_bytes());
    let v4 = AccountId(key4.verifying_key().to_bytes());
    let offender = AccountId(key.verifying_key().to_bytes());
    let chain_id = 6913u64;
    let mut node = node_with_validators(
        vec![v1, offender, v3, v4],
        &[(offender, 10_000)],
        Some(key1.clone()),
    );
    node.state.get_mut(&offender).unwrap().balance = 1_000_000;

    let a = ConsensusVote::sign(4, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(4, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
    assert!(node.on_equivocation_evidence(ev.clone()));
    apply_report_equivocation(&mut node, v1, &key1, ev);
    let slash_id = node
        .state
        .slash_appeals()
        .find_equivocation_at_round(&offender, 4)
        .unwrap()
        .id;

    let mut vm = Vm::new();
    vm.set_registration_proof_rule(Some(chain_id), 0);
    node.state.get_mut(&v1).unwrap().stake = 5_000;
    for id in [v3, v4] {
        node.state.insert(Account {
            id,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 5_000,
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
    for (i, (voter, sk)) in [(v1, &key1), (v3, &key3), (v4, &key4)]
        .into_iter()
        .enumerate()
    {
        let sig = sign_slash_appeal_resolve_ballot(chain_id, appeal_id, slash_id, false, sk);
        let nonce = node.state.get(&voter).unwrap().nonce;
        vm.execute_with_context(
            &Transaction {
                nonce,
                sender: voter,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id,
                    approved: false,
                    ballot_signature: sig,
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
fn operator_resolve_rpc_does_not_mutate_committed_state() {
    let local_key = SigningKey::generate(&mut OsRng);
    let v1 = AccountId(local_key.verifying_key().to_bytes());
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(
        vec![v1, offender, AccountId([3u8; 32]), AccountId([4u8; 32])],
        &[(offender, 10_000)],
        Some(local_key.clone()),
    );
    let a = ConsensusVote::sign(1, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(1, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
    apply_report_equivocation(&mut node, v1, &local_key, ev);
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);
    let slash_id = node
        .state
        .slash_appeals()
        .find_equivocation_at_round(&offender, 1)
        .unwrap()
        .id;
    // Seed local mirror and approve via deprecated operator RPC — must not restore stake.
    node.slash_registry.record_slash_with_id(
        slash_id,
        offender.0,
        5_000,
        boing_governance::SlashReason::Equivocation,
        1,
        1000,
    );
    let appeal_id = node
        .slash_registry
        .submit_appeal(slash_id, b"rpc".to_vec(), 1)
        .unwrap();
    assert_eq!(node.resolve_slash_appeal(appeal_id, true).unwrap(), 0);
    assert_eq!(node.state.get(&offender).unwrap().stake, 5_000);
}

#[test]
fn failed_serve_and_equivocation_share_slash_book() {
    let local_key = SigningKey::generate(&mut OsRng);
    let v1 = AccountId(local_key.verifying_key().to_bytes());
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let mut node = node_with_validators(
        vec![v1, offender, AccountId([3u8; 32]), AccountId([4u8; 32])],
        &[(offender, 10_000)],
        Some(local_key.clone()),
    );
    node.state
        .slash_appeals_mut()
        .record_slash(
            offender,
            1,
            boing_state::ConsensusSlashReason::BlobFailedServe,
            0,
            1000,
        )
        .unwrap();
    let a = ConsensusVote::sign(2, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(2, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();
    apply_report_equivocation(&mut node, v1, &local_key, ev);
    assert_eq!(node.state.slash_appeals().list_slashes().len(), 2);
}

/// Equivocation slashing is gated (#125): below the activation height the report tx fails and
/// nothing is slashed; at the height it burns.
#[test]
fn report_equivocation_is_inert_before_activation_height() {
    let local_key = SigningKey::generate(&mut OsRng);
    let reporter = AccountId(local_key.verifying_key().to_bytes());
    let key = SigningKey::generate(&mut OsRng);
    let offender = AccountId(key.verifying_key().to_bytes());
    let a = ConsensusVote::sign(4, Hash([1u8; 32]), &key);
    let b = ConsensusVote::sign(4, Hash([2u8; 32]), &key);
    let ev = EquivocationEvidence::try_from_votes(a, b).unwrap();

    let mut state = boing_state::StateStore::new();
    state.insert(Account {
        id: reporter,
        state: AccountState {
            balance: 1_000_000,
            ..Default::default()
        },
    });
    state.insert(Account {
        id: offender,
        state: AccountState {
            stake: 10_000,
            ..Default::default()
        },
    });
    let burn = boing_tokenomics::FEE_BURN_SINK;
    let tx = Transaction {
        nonce: 0,
        sender: reporter,
        payload: TransactionPayload::ReportEquivocation { evidence: ev },
        access_list: AccessList::new(
            vec![reporter, offender, burn],
            vec![reporter, offender, burn],
        ),
    };
    let mut vm = Vm::new();
    vm.set_equivocation_slash_from(100);
    assert!(vm.execute_with_context(&tx, &mut state, ctx(99)).is_err());
    assert_eq!(state.get(&offender).unwrap().stake, 10_000);
    vm.execute_with_context(&tx, &mut state, ctx(100)).unwrap();
    assert_eq!(state.get(&offender).unwrap().stake, 5_000);
}
