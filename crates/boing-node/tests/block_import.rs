//! Test block import and validation.

use boing_consensus::ConsensusEngine;
use boing_execution::BlockExecutor;
use boing_node::block_validation::{
    chains_to, import_block, validate_and_execute_block, BlockValidationError,
    MAX_BLOCK_TIMESTAMP_FUTURE_SECS,
};
use boing_node::chain::ChainState;
use boing_primitives::{
    receipts_root, tx_root, AccessList, Account, AccountId, AccountState, Block, BlockHeader, Hash,
    Transaction, TransactionPayload,
};
use boing_state::StateStore;
use ed25519_dalek::SigningKey;

/// Proposer/sender key: blocks must carry a valid signature per tx (security review #16).
fn key() -> SigningKey {
    SigningKey::from_bytes(&[1u8; 32])
}

fn sigs(txs: &[Transaction]) -> Vec<boing_primitives::Signature> {
    txs.iter()
        .map(|t| boing_primitives::sign_transaction(t, &key()))
        .collect()
}

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
            blobs_root: None,
        },
        transactions: vec![],
        signatures: vec![],
        priority_tips: vec![],
    };
    assert!(chains_to(&block, chain.latest_hash(), chain.height()));
    assert!(!chains_to(&block, chain.latest_hash(), chain.height() + 1));
}

#[test]
fn test_validate_and_execute_block() {
    let proposer = AccountId(key().verifying_key().to_bytes());
    let to = AccountId([2u8; 32]);
    let mut parent = StateStore::new();
    parent.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
    parent.insert(Account {
        id: to,
        state: AccountState {
            balance: 0,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });

    let tx = mk_transfer(proposer, to, 0, 100);
    let txs = vec![tx.clone()];
    let exec = BlockExecutor::new();
    let mut state = parent.snapshot();
    let (_g, exec_receipts) = exec
        .execute_block(1, 0, &txs, &mut state, proposer)
        .unwrap();
    let reward = boing_tokenomics::block_emission_validators_in(
        1,
        boing_tokenomics::Denomination::BaseUnits,
    );
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
            blobs_root: None,
        },
        signatures: sigs(&txs),
        priority_tips: vec![],
        transactions: txs,
    };

    let validators = vec![proposer];
    let result = validate_and_execute_block(&block, &parent, &validators, proposer, &exec, 0);
    assert!(result.is_ok());
    let (new_state, receipts) = result.unwrap();
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].success);
    let fee = boing_tokenomics::fee_for_gas_in(
        receipts[0].gas_used,
        boing_tokenomics::Denomination::BaseUnits,
    );
    let (v, _, _) = boing_tokenomics::split_fee(fee);
    // Proposer is also sender: pays amount+fee, then receives validator fee share + block reward.
    assert_eq!(
        new_state.get(&proposer).unwrap().balance,
        1_000_000 * boing_primitives::BASE_UNITS_PER_BOING - 100 - fee + v + reward
    );
    assert_eq!(new_state.get(&to).unwrap().balance, 100);
}

#[test]
fn test_import_block() {
    let proposer = AccountId(key().verifying_key().to_bytes());
    let genesis = ChainState::genesis(proposer);
    let chain = ChainState::from_genesis(genesis.clone());
    let mut consensus = ConsensusEngine::single_validator(proposer);
    let _ = consensus.propose_and_commit(genesis.clone());

    let mut parent = StateStore::new();
    parent.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
    let to = AccountId([2u8; 32]);
    parent.insert(Account {
        id: to,
        state: AccountState {
            balance: 0,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });

    let tx = mk_transfer(proposer, to, 0, 50);
    let txs = vec![tx];
    let exec = BlockExecutor::new();
    let mut state = parent.snapshot();
    let (_g, exec_receipts) = exec
        .execute_block(1, 0, &txs, &mut state, proposer)
        .unwrap();
    let reward = boing_tokenomics::block_emission_validators_in(
        1,
        boing_tokenomics::Denomination::BaseUnits,
    );
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
            blobs_root: None,
        },
        signatures: sigs(&txs),
        priority_tips: vec![],
        transactions: txs,
    };

    let result = import_block(
        &block,
        chain.latest_hash(),
        chain.height(),
        &parent,
        &consensus,
        &exec,
        0,
    );
    assert!(result.is_ok());
}

#[test]
fn reject_block_timestamp_not_after_parent() {
    let proposer = AccountId(key().verifying_key().to_bytes());
    let mut parent = StateStore::new();
    parent.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
    let exec = BlockExecutor::new();
    let block = Block {
        header: BlockHeader {
            parent_hash: Hash::ZERO,
            height: 1,
            timestamp: 5, // not strictly after parent_timestamp=5
            proposer,
            tx_root: tx_root(&[]),
            receipts_root: receipts_root(&[]),
            state_root: Hash::ZERO,
            blobs_root: None,
        },
        signatures: vec![],
        priority_tips: vec![],
        transactions: vec![],
    };
    match validate_and_execute_block(&block, &parent, &[proposer], proposer, &exec, 5) {
        Err(BlockValidationError::TimestampNotAfterParent { .. }) => {}
        Err(e) => panic!("equal timestamp must fail, got error: {e}"),
        Ok(_) => panic!("equal timestamp must fail"),
    }
}

#[test]
fn reject_block_timestamp_too_far_in_future() {
    let proposer = AccountId(key().verifying_key().to_bytes());
    let mut parent = StateStore::new();
    parent.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
    let exec = BlockExecutor::new();
    let far_future = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .saturating_add(MAX_BLOCK_TIMESTAMP_FUTURE_SECS)
        .saturating_add(60);
    let block = Block {
        header: BlockHeader {
            parent_hash: Hash::ZERO,
            height: 1,
            timestamp: far_future,
            proposer,
            tx_root: tx_root(&[]),
            receipts_root: receipts_root(&[]),
            state_root: Hash::ZERO,
            blobs_root: None,
        },
        signatures: vec![],
        priority_tips: vec![],
        transactions: vec![],
    };
    match validate_and_execute_block(&block, &parent, &[proposer], proposer, &exec, 0) {
        Err(BlockValidationError::TimestampTooFarInFuture { .. }) => {}
        Err(e) => panic!("far-future timestamp must fail, got error: {e}"),
        Ok(_) => panic!("far-future timestamp must fail"),
    }
}
