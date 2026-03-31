//! Test block import and validation.

use boing_consensus::ConsensusEngine;
use boing_execution::BlockExecutor;
use boing_node::block_validation::{chains_to, import_block, validate_and_execute_block};
use boing_node::chain::ChainState;
use boing_primitives::{
    receipts_root, tx_root, AccessList, Account, AccountId, AccountState, Block, BlockHeader,
    Transaction, TransactionPayload,
};
use boing_state::StateStore;

fn mk_transfer(from: AccountId, to: AccountId, nonce: u64, amount: u128) -> Transaction {
    Transaction {
        nonce,
        sender: from,
        payload: TransactionPayload::Transfer { to, amount },
        access_list: AccessList::new(vec![from, to], vec![from, to]),
    }
}

#[test]
fn test_chains_to() {
    let genesis = ChainState::genesis(AccountId([1u8; 32]));
    let chain = ChainState::from_genesis(genesis.clone());
    let block = Block {
        header: BlockHeader {
            parent_hash: genesis.hash(),
            height: 1,
            timestamp: 1,
            proposer: AccountId([1u8; 32]),
            tx_root: boing_primitives::Hash::ZERO,
            receipts_root: boing_primitives::Hash::ZERO,
            state_root: boing_primitives::Hash::ZERO,
        },
        transactions: vec![],
    };
    assert!(chains_to(&block, chain.latest_hash(), chain.height()));
    assert!(!chains_to(&block, chain.latest_hash(), chain.height() + 1));
}

#[test]
fn test_validate_and_execute_block() {
    let proposer = AccountId([1u8; 32]);
    let to = AccountId([2u8; 32]);
    let mut parent = StateStore::new();
    parent.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1000,
            nonce: 0,
            stake: 0,
        },
    });
    parent.insert(Account {
        id: to,
        state: AccountState {
            balance: 0,
            nonce: 0,
            stake: 0,
        },
    });

    let tx = mk_transfer(proposer, to, 0, 100);
    let txs = vec![tx.clone()];
    let exec = BlockExecutor::new();
    let mut state = parent.snapshot();
    let (_g, exec_receipts) = exec.execute_block(1, &txs, &mut state).unwrap();
    let reward = boing_tokenomics::block_emission_validators(1);
    state.get_mut(&proposer).unwrap().balance =
        state.get(&proposer).unwrap().balance.saturating_add(reward);
    let state_root = state.state_root();
    let rr = receipts_root(&exec_receipts);

    let block = Block {
        header: BlockHeader {
            parent_hash: boing_primitives::Hash::ZERO,
            height: 1,
            timestamp: 1,
            proposer,
            tx_root: tx_root(&txs),
            receipts_root: rr,
            state_root,
        },
        transactions: txs,
    };

    let validators = vec![proposer];
    let result = validate_and_execute_block(&block, &parent, &validators, &exec);
    assert!(result.is_ok());
    let (new_state, receipts) = result.unwrap();
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].success);
    assert_eq!(
        new_state.get(&proposer).unwrap().balance,
        1000 - 100 + reward
    );
    assert_eq!(new_state.get(&to).unwrap().balance, 100);
}

#[test]
fn test_import_block() {
    let proposer = AccountId([1u8; 32]);
    let genesis = ChainState::genesis(proposer);
    let chain = ChainState::from_genesis(genesis.clone());
    let mut consensus = ConsensusEngine::single_validator(proposer);
    let _ = consensus.propose_and_commit(genesis.clone());

    let mut parent = StateStore::new();
    parent.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1_000_000,
            nonce: 0,
            stake: 0,
        },
    });
    let to = AccountId([2u8; 32]);
    parent.insert(Account {
        id: to,
        state: AccountState {
            balance: 0,
            nonce: 0,
            stake: 0,
        },
    });

    let tx = mk_transfer(proposer, to, 0, 50);
    let txs = vec![tx];
    let exec = BlockExecutor::new();
    let mut state = parent.snapshot();
    let (_g, exec_receipts) = exec.execute_block(1, &txs, &mut state).unwrap();
    let reward = boing_tokenomics::block_emission_validators(1);
    state.get_mut(&proposer).unwrap().balance =
        state.get(&proposer).unwrap().balance.saturating_add(reward);
    let state_root = state.state_root();
    let rr = receipts_root(&exec_receipts);

    let block = Block {
        header: BlockHeader {
            parent_hash: genesis.hash(),
            height: 1,
            timestamp: 1,
            proposer,
            tx_root: tx_root(&txs),
            receipts_root: rr,
            state_root,
        },
        transactions: txs,
    };

    let result = import_block(
        &block,
        chain.latest_hash(),
        chain.height(),
        &parent,
        &consensus,
        &exec,
    );
    assert!(result.is_ok());
}
