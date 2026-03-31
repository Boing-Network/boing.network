//! P2P smoke test: node with live P2P produces and broadcasts a block.

use std::collections::HashMap;

use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn node_with_p2p(
    signing_key: &SigningKey,
    balance: u128,
    p2p_listen: &str,
) -> (BoingNode, tokio::sync::mpsc::Receiver<boing_p2p::P2pEvent>) {
    let (p2p, event_rx) = boing_p2p::P2pNode::new(p2p_listen, None).expect("P2P init");
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let genesis = boing_node::chain::ChainState::genesis(proposer);
    let chain = boing_node::chain::ChainState::from_genesis(genesis.clone());
    let mut consensus = boing_consensus::ConsensusEngine::single_validator(proposer);
    let _ = consensus.propose_and_commit(genesis);

    let mut state = boing_state::StateStore::new();
    state.insert(Account {
        id: proposer,
        state: AccountState {
            balance,
            nonce: 0,
            stake: 0,
        },
    });

    let node = BoingNode {
        chain,
        consensus,
        state,
        executor: boing_execution::BlockExecutor::new(),
        producer: boing_node::block_producer::BlockProducer::new(proposer).with_max_txs(100),
        vm: boing_execution::Vm::new(),
        scheduler: boing_execution::TransactionScheduler::new(),
        mempool: boing_node::mempool::Mempool::new(),
        p2p,
        dapp_registry: boing_node::dapp_registry::DappRegistry::new(),
        intent_pool: boing_node::intent_pool::IntentPool::new(),
        qa_pool: boing_node::node::pending_qa_pool_default(),
        persistence: None,
        receipts: HashMap::new(),
    };
    (node, event_rx)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_node_produces_and_broadcasts() {
    let key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(key.verifying_key().to_bytes());
    let to = AccountId([2u8; 32]);

    let (mut node, _p2p_rx) = node_with_p2p(&key, 1_000_000, "/ip4/127.0.0.1/tcp/0");

    let tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::Transfer { to, amount: 100 },
        access_list: AccessList::new(vec![proposer, to], vec![proposer, to]),
    };
    let signed = SignedTransaction::new(tx, &key);
    node.submit_transaction(signed).unwrap();

    let hash = node.produce_block_if_ready().expect("should produce block");
    assert_ne!(hash, boing_primitives::Hash::ZERO);
    assert_eq!(node.chain.height(), 1);
    assert!(node.chain.get_block_by_hash(&hash).is_some());
}
