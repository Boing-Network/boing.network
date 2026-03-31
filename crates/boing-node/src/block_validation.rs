//! Block import and validation — validate blocks from peers.

use boing_consensus::ConsensusEngine;
use boing_execution::BlockExecutor;
use boing_primitives::{
    receipts_root, tx_root, Account, AccountId, AccountState, Block, ExecutionReceipt, Hash,
};
use boing_state::StateStore;
use boing_tokenomics::block_emission_validators;

/// Validate and execute a block. Returns updated state and per-tx receipts on success.
/// Caller must ensure block chains to parent (parent_hash, height).
pub fn validate_and_execute_block(
    block: &Block,
    parent_state: &StateStore,
    validator_set: &[AccountId],
    executor: &BlockExecutor,
) -> Result<(StateStore, Vec<ExecutionReceipt>), BlockValidationError> {
    // 1. Tx root
    let expected_tx_root = tx_root(&block.transactions);
    if block.header.tx_root != expected_tx_root {
        return Err(BlockValidationError::InvalidTxRoot);
    }

    // 2. Proposer in validator set
    if !validator_set.contains(&block.header.proposer) {
        return Err(BlockValidationError::InvalidProposer);
    }

    // 3. Execute on snapshot
    let mut state = parent_state.snapshot();
    let (_gas, receipts) = executor
        .execute_block(block.header.height, &block.transactions, &mut state)
        .map_err(|e| BlockValidationError::ExecutionFailed(e.to_string()))?;

    // 4. Block reward
    let reward = block_emission_validators(block.header.height);
    if reward > 0 {
        match state.get_mut(&block.header.proposer) {
            Some(s) => s.balance = s.balance.saturating_add(reward),
            None => {
                state.insert(Account {
                    id: block.header.proposer,
                    state: AccountState {
                        balance: reward,
                        nonce: 0,
                        stake: 0,
                    },
                });
            }
        }
    }

    // 5. Receipts root
    let expected_receipts_root = receipts_root(&receipts);
    if block.header.receipts_root != expected_receipts_root {
        return Err(BlockValidationError::InvalidReceiptsRoot {
            expected: block.header.receipts_root,
            computed: expected_receipts_root,
        });
    }

    // 6. State root
    let computed_root = state.state_root();
    if block.header.state_root != computed_root {
        return Err(BlockValidationError::InvalidStateRoot {
            expected: block.header.state_root,
            computed: computed_root,
        });
    }

    Ok((state, receipts))
}

/// Check that a block chains to our tip. Does not execute.
pub fn chains_to(block: &Block, our_latest_hash: Hash, our_height: u64) -> bool {
    block.header.parent_hash == our_latest_hash && block.header.height == our_height + 1
}

/// Full import: validate block and return new state + receipts if it chains and is valid.
pub fn import_block(
    block: &Block,
    our_latest_hash: Hash,
    our_height: u64,
    parent_state: &StateStore,
    consensus: &ConsensusEngine,
    executor: &BlockExecutor,
) -> Result<(StateStore, Vec<ExecutionReceipt>), BlockValidationError> {
    if !chains_to(block, our_latest_hash, our_height) {
        return Err(BlockValidationError::DoesNotChain);
    }
    let validator_set = consensus.validators();
    if validator_set.is_empty() {
        return Err(BlockValidationError::NoValidators);
    }
    validate_and_execute_block(block, parent_state, validator_set, executor)
}

#[derive(Debug, thiserror::Error)]
pub enum BlockValidationError {
    #[error("Block does not chain to our tip")]
    DoesNotChain,
    #[error("Invalid tx root")]
    InvalidTxRoot,
    #[error("Invalid receipts root: expected {expected:?}, computed {computed:?}")]
    InvalidReceiptsRoot { expected: Hash, computed: Hash },
    #[error("Invalid proposer")]
    InvalidProposer,
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Invalid state root: expected {expected:?}, computed {computed:?}")]
    InvalidStateRoot { expected: Hash, computed: Hash },
    #[error("No validators configured")]
    NoValidators,
}
