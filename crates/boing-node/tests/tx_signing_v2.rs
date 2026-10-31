//! Security review #2/#3 end to end on one node: chain-id (v2) signatures, height-gated rejection of
//! legacy signatures, cross-chain replay rejection, and the failed-tx fee.
//! (Own test binary: it sets process env `BOING_CHAIN_ID` / `BOING_TX_V2_FROM`.)

use std::collections::HashMap;

use boing_node::mempool::MempoolError;
use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn node_with_proposer_key(signing_key: &SigningKey, balance: u128) -> BoingNode {
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let genesis = boing_node::chain::ChainState::genesis(proposer);
    let chain = boing_node::chain::ChainState::from_genesis(genesis.clone());
    let mut consensus = boing_consensus::ConsensusEngine::single_validator(proposer);
    let _ = consensus.propose_and_commit(genesis);

    let mut state = boing_state::StateStore::new();
    state.insert(Account {
        id: proposer,
        state: AccountState {
            balance: balance * boing_primitives::BASE_UNITS_PER_BOING,
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

fn transfer(sender: AccountId, nonce: u64, to: AccountId, amount: u128) -> Transaction {
    Transaction {
        nonce,
        sender,
        payload: TransactionPayload::Transfer { to, amount },
        access_list: AccessList::new(vec![sender, to], vec![sender, to]),
    }
}

#[test]
fn v2_signatures_legacy_cutover_cross_chain_and_failed_tx_fee() {
    std::env::set_var("BOING_CHAIN_ID", "6913");
    std::env::set_var("BOING_TX_V2_FROM", "2");
    let key = SigningKey::generate(&mut OsRng);
    let me = AccountId(key.verifying_key().to_bytes());
    let to = AccountId([2u8; 32]);
    let mut node = node_with_proposer_key(&key, 1_000_000);

    // Before activation (next block 1): legacy and v2 both admitted; another chain's v2 is not.
    node.submit_transaction(SignedTransaction::new_legacy(transfer(me, 0, to, 1), &key))
        .unwrap();
    node.submit_transaction(SignedTransaction::new_for_chain(
        transfer(me, 1, to, 1),
        &key,
        6913,
    ))
    .unwrap();
    assert!(matches!(
        node.submit_transaction(SignedTransaction::new_for_chain(
            transfer(me, 2, to, 1),
            &key,
            6914
        )),
        Err(MempoolError::InvalidSignature)
    ));
    node.produce_block_if_ready().expect("block 1");
    assert_eq!(node.chain.height(), 1);
    assert_eq!(node.state.get(&to).unwrap().balance, 2);

    // From activation (next block 2): legacy rejected, cross-chain rejected, v2 admitted.
    assert!(matches!(
        node.submit_transaction(SignedTransaction::new_legacy(transfer(me, 2, to, 1), &key)),
        Err(MempoolError::LegacySignature)
    ));
    assert!(matches!(
        node.submit_transaction(SignedTransaction::new_for_chain(
            transfer(me, 2, to, 1),
            &key,
            6914
        )),
        Err(MempoolError::InvalidSignature)
    ));
    // A deliberately failing tx (amount far above the balance) passes admission (signature, nonce,
    // fee balance), is included, charged, and advances the nonce.
    let failing = transfer(me, 2, to, u128::MAX / 4);
    let failing_id = failing.id();
    node.submit_transaction(SignedTransaction::new_for_chain(failing, &key, 6913))
        .unwrap();
    let treasury = |n: &BoingNode| {
        n.state
            .get(&boing_tokenomics::PROTOCOL_TREASURY)
            .map(|s| s.balance)
            .unwrap_or(0)
    };
    let t_before = treasury(&node);
    node.produce_block_if_ready().expect("block 2");
    assert_eq!(node.chain.height(), 2);
    let r = node
        .receipts
        .get(&failing_id)
        .expect("failure receipt recorded");
    assert!(!r.success);
    assert_eq!(r.gas_used, boing_execution::FAILED_TX_GAS);
    let fee = boing_execution::failed_tx_fee_amount(node.executor.fee_policy());
    assert_eq!(
        treasury(&node) - t_before,
        fee,
        "failed-tx fee charged to the treasury"
    );
    assert_eq!(node.state.get(&me).unwrap().nonce, 3, "nonce advanced");
    assert_eq!(node.state.get(&to).unwrap().balance, 2, "transfer reverted");
    // The next tx from the same sender works.
    node.submit_transaction(SignedTransaction::new_for_chain(
        transfer(me, 3, to, 5),
        &key,
        6913,
    ))
    .unwrap();
    node.produce_block_if_ready().expect("block 3");
    assert_eq!(node.state.get(&to).unwrap().balance, 7);
}
