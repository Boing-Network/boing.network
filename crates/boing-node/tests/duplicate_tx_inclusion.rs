//! Regression: a tx committed in a block proposed by **another** validator stayed in this node's
//! mempool (it arrived by gossip and was never drained here), so this node's next proposal
//! re-included the same tx id. The copy failed with `Invalid nonce` and, because receipts are
//! indexed by tx id, overwrote the successful receipt. Seen on testnet 6913 for every tx from
//! roughly block 65000 on (e.g. tx 0x58d2b3f6… in blocks 66276 and 66277).

use std::collections::HashMap;

use boing_node::chain::ChainState;
use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, Block, ExecutionReceipt, SignedTransaction,
    Transaction, TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

const FUNDS: u128 = 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING;

/// One validator of a two-validator set (quorum 1, round-robin leaders, like testnet 6913 today).
fn validator(keys: &[SigningKey], local: usize, genesis: &Block, user: AccountId) -> BoingNode {
    let validators: Vec<AccountId> = keys
        .iter()
        .map(|k| AccountId(k.verifying_key().to_bytes()))
        .collect();
    let mut consensus = boing_consensus::ConsensusEngine::new(validators.clone());
    let _ = consensus.propose_and_commit(genesis.clone());
    consensus.sync_round(1);
    let mut state = boing_state::StateStore::new();
    for id in validators.iter().copied().chain(std::iter::once(user)) {
        state.insert(Account {
            id,
            state: AccountState {
                balance: FUNDS,
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
        producer: boing_node::block_producer::BlockProducer::new(validators[local])
            .with_max_txs(100),
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
        validator_signing_key: Some(keys[local].clone()),
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

fn transfer(key: &SigningKey, nonce: u64) -> SignedTransaction {
    let sender = AccountId(key.verifying_key().to_bytes());
    let to = AccountId([0xee; 32]);
    SignedTransaction::new(
        Transaction {
            nonce,
            sender,
            payload: TransactionPayload::Transfer { to, amount: 1 },
            access_list: AccessList::new(vec![sender, to], vec![sender, to]),
        },
        key,
    )
}

#[test]
fn tx_committed_by_peer_is_not_reincluded_by_next_leader() {
    let keys: Vec<SigningKey> = (0..2).map(|_| SigningKey::generate(&mut OsRng)).collect();
    let user_key = SigningKey::generate(&mut OsRng);
    let user = AccountId(user_key.verifying_key().to_bytes());
    let genesis = ChainState::genesis(AccountId(keys[0].verifying_key().to_bytes()));
    let mut nodes = [
        validator(&keys, 0, &genesis, user),
        validator(&keys, 1, &genesis, user),
    ];
    let leader1 = nodes[0].consensus.leader(1);
    let (l, f) = if leader1 == nodes[0].producer.proposer() {
        (0, 1)
    } else {
        (1, 0)
    };
    assert_ne!(nodes[f].consensus.leader(2), leader1, "round-robin leaders");

    // RPC submit on one node + gossip copy on the other: both pools hold the same signed tx.
    let tx = transfer(&user_key, 0);
    let tx_id = tx.tx.id();
    nodes[l].submit_transaction(tx.clone()).unwrap();
    nodes[f].submit_transaction(tx.clone()).unwrap();

    // Height 1: leader proposes and commits (quorum 1); the follower imports and commits.
    nodes[l]
        .produce_block_if_ready()
        .expect("leader commits block 1");
    let b1 = nodes[l].chain.get_block_by_height(1).unwrap();
    assert_eq!(b1.transactions.len(), 1);
    nodes[f]
        .handle_network_block(&b1)
        .unwrap()
        .expect("follower commits block 1");
    assert_eq!(nodes[f].chain.height(), 1);
    assert_eq!(nodes[f].state.get(&user).unwrap().nonce, 1);
    assert!(nodes[f].receipts[&tx_id].success);

    // The committed tx must have left the follower's pool...
    assert!(
        !nodes[f].mempool.contains_tx_id(&tx_id),
        "tx committed by the peer's block is still pending on the follower"
    );
    assert_eq!(nodes[f].mempool.len(), 0);
    // ...so the follower, leader at height 2, has nothing to propose (before the fix it built
    // block 2 with the same tx id, which failed `Invalid nonce: expected 1, got 0`).
    assert!(nodes[f].produce_block_if_ready().is_none());
    assert_eq!(nodes[f].chain.height(), 1);
    let r = &nodes[f].receipts[&tx_id];
    assert!(r.success && r.block_height == 1, "receipt: {r:?}");

    // A later gossip echo of the same tx is refused (stale nonce) rather than re-queued.
    assert!(nodes[f].submit_transaction(tx.clone()).is_err());
    assert!(nodes[l].submit_transaction(tx).is_err());
}

#[test]
fn catch_up_import_prunes_mempool_too() {
    let keys: Vec<SigningKey> = (0..2).map(|_| SigningKey::generate(&mut OsRng)).collect();
    let user_key = SigningKey::generate(&mut OsRng);
    let user = AccountId(user_key.verifying_key().to_bytes());
    let genesis = ChainState::genesis(AccountId(keys[0].verifying_key().to_bytes()));
    let mut nodes = [
        validator(&keys, 0, &genesis, user),
        validator(&keys, 1, &genesis, user),
    ];
    let leader1 = nodes[0].consensus.leader(1);
    let (l, f) = if leader1 == nodes[0].producer.proposer() {
        (0, 1)
    } else {
        (1, 0)
    };
    let t0 = transfer(&user_key, 0);
    let t1 = transfer(&user_key, 1);
    nodes[l].submit_transaction(t0.clone()).unwrap();
    nodes[f].submit_transaction(t0.clone()).unwrap();
    nodes[f].submit_transaction(t1.clone()).unwrap(); // future nonce: must survive the prune
    nodes[l].produce_block_if_ready().expect("block 1");
    let b1 = nodes[l].chain.get_block_by_height(1).unwrap();
    nodes[f].import_network_block_catchup(&b1).unwrap();
    assert!(!nodes[f].mempool.contains_tx_id(&t0.tx.id()));
    assert!(nodes[f].mempool.contains_tx_id(&t1.tx.id()));
    assert_eq!(nodes[f].mempool.len(), 1);
}

#[test]
fn builder_skips_consumed_nonce_even_if_pool_holds_it() {
    // Defence in depth: even if some path leaves a consumed-nonce tx in the pool, the producer
    // must not put it in a block.
    let keys: Vec<SigningKey> = vec![SigningKey::generate(&mut OsRng)];
    let user_key = SigningKey::generate(&mut OsRng);
    let user = AccountId(user_key.verifying_key().to_bytes());
    let genesis = ChainState::genesis(AccountId(keys[0].verifying_key().to_bytes()));
    let mut n = validator(&keys, 0, &genesis, user);
    let tx = transfer(&user_key, 0);
    n.submit_transaction(tx.clone()).unwrap();
    n.produce_block_if_ready().expect("block 1");
    assert_eq!(n.state.get(&user).unwrap().nonce, 1);
    // Bypass node-level admission (stale-nonce precheck) and push the executed tx back.
    n.mempool.reinsert(vec![tx.clone()]);
    assert!(n.mempool.contains_tx_id(&tx.tx.id()));
    assert!(
        n.produce_block_if_ready().is_none(),
        "producer re-included a tx whose nonce is already consumed"
    );
    assert_eq!(n.chain.height(), 1);
    assert!(n.receipts[&tx.tx.id()].success);
}

#[test]
fn reload_keeps_executed_receipt_over_legacy_duplicate() {
    // On-disk shape left by the bug: receipts_N has the success, receipts_{N+1} the same tx id
    // failing `Invalid nonce`. Reload used to index last-write-wins (failure); it must now keep
    // the success, while the block-with-receipts view of N+1 still shows that block's own receipt.
    let key = SigningKey::generate(&mut OsRng);
    let me = AccountId(key.verifying_key().to_bytes());
    let dir = std::env::temp_dir().join(format!(
        "boing-dup-receipt-{}-{}",
        std::process::id(),
        hex::encode(&me.0[..4])
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let self_tx = |nonce: u64| {
        SignedTransaction::new(
            Transaction {
                nonce,
                sender: me,
                payload: TransactionPayload::Transfer { to: me, amount: 1 },
                access_list: AccessList::new(vec![me], vec![me]),
            },
            &key,
        )
    };
    let t0 = self_tx(0);
    let t0_id = t0.tx.id();
    {
        let mut node =
            BoingNode::with_data_dir_and_validators(Some(&dir), vec![me], me, Some(key.clone()))
                .unwrap();
        node.submit_transaction(t0).unwrap();
        node.produce_block_if_ready().expect("block 1");
        node.submit_transaction(self_tx(1)).unwrap();
        node.produce_block_if_ready().expect("block 2");
        assert!(node.receipts[&t0_id].success);
        // Inject the legacy duplicate receipt into block 2's persisted list.
        let p = node.persistence.as_ref().unwrap();
        let mut r2 = p.load_receipts_for_height(2).unwrap().unwrap();
        r2.push(ExecutionReceipt {
            tx_id: t0_id,
            block_height: 2,
            tx_index: r2.len() as u32,
            success: false,
            gas_used: 0,
            return_data: vec![],
            logs: vec![],
            error: Some("Invalid nonce: expected 1, got 0".into()),
        });
        p.save_receipts(2, &r2).unwrap();
        // Live index behaviour for an arriving duplicate (commit path uses the same helper).
        assert!(!boing_node::receipt_retention::index_receipt(
            &mut node.receipts,
            r2.last().unwrap().clone()
        ));
        assert!(node.receipts[&t0_id].success);
    }
    let restarted =
        BoingNode::with_data_dir_and_validators(Some(&dir), vec![me], me, Some(key)).unwrap();
    assert_eq!(restarted.chain.height(), 2);
    let r = &restarted.receipts[&t0_id];
    assert!(
        r.success,
        "reloaded receipt index returned the duplicate: {r:?}"
    );
    assert_eq!(r.block_height, 1);
    // Per-block view: block 1 shows the success, block 2 shows its own (failed) copy.
    assert!(restarted.receipt_in_block(1, 0, &t0_id).unwrap().success);
    let dup = restarted.receipt_in_block(2, 1, &t0_id).unwrap();
    assert!(!dup.success && dup.block_height == 2);
    let _ = std::fs::remove_dir_all(&dir);
}
