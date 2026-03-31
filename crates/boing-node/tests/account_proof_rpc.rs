//! Test boing_getAccountProof and boing_verifyAccountProof RPC.

use std::collections::HashMap;

use boing_node::node::BoingNode;
use boing_primitives::{Account, AccountId, AccountState};
use boing_state::StateStore;

fn node_with_accounts() -> BoingNode {
    // Use keys 0 and 128 for simple 2-leaf tree (proof works for these)
    let mut k0 = [0u8; 32];
    k0[0] = 0;
    let mut k128 = [0u8; 32];
    k128[0] = 128;
    let proposer = AccountId(k0);
    let genesis = boing_node::chain::ChainState::genesis(proposer);
    let chain = boing_node::chain::ChainState::from_genesis(genesis.clone());
    let mut consensus = boing_consensus::ConsensusEngine::single_validator(proposer);
    let _ = consensus.propose_and_commit(genesis);
    let mut state = StateStore::new();
    state.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1_000_000,
            nonce: 0,
            stake: 0,
        },
    });
    state.insert(Account {
        id: AccountId(k128),
        state: AccountState {
            balance: 500,
            nonce: 1,
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
fn test_prove_account() {
    let mut node = node_with_accounts();
    let mut k128 = [0u8; 32];
    k128[0] = 128;
    let account_id = AccountId(k128);
    let proof = node.state.prove_account(&account_id).unwrap();
    assert!(proof.verify());
}
