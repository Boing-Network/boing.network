//! End-to-end test: single node produces blocks with transactions.

use std::collections::HashMap;

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
            balance,
            nonce: 0,
            stake: 0,
        },
    });

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
    }
}

#[test]
fn test_single_node_produces_block_with_transfer() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let to = AccountId([2u8; 32]);

    let mut node = node_with_proposer_key(&signing_key, 1_000_000);

    let tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::Transfer { to, amount: 100 },
        access_list: AccessList::new(vec![proposer, to], vec![proposer, to]),
    };
    let signed = SignedTransaction::new(tx, &signing_key);

    node.submit_transaction(signed).unwrap();
    assert_eq!(node.mempool.len(), 1);

    let hash = node.produce_block_if_ready().expect("should produce block");
    assert!(hash != boing_primitives::Hash::ZERO);

    assert_eq!(node.mempool.len(), 0);
    assert_eq!(node.chain.height(), 1);
    assert!(node.state.get(&proposer).unwrap().balance >= 999_900); // 999_900 after transfer + block reward
    assert_eq!(node.state.get(&to).unwrap().balance, 100);
}
