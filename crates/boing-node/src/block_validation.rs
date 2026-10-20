//! Block import and validation — validate blocks from peers.

use boing_consensus::ConsensusEngine;
use boing_execution::BlockExecutor;
use boing_primitives::{
    blobs_root_of_transactions, receipts_root, tx_root, Account, AccountId, AccountState, Block,
    ExecutionReceipt, Hash,
};
use boing_state::StateStore;

/// Validate and execute a block. Returns updated state and per-tx receipts on success.
/// Caller must ensure block chains to parent (parent_hash, height).
pub fn validate_and_execute_block(
    block: &Block,
    parent_state: &StateStore,
    validator_set: &[AccountId],
    expected_leader: AccountId,
    executor: &BlockExecutor,
) -> Result<(StateStore, Vec<ExecutionReceipt>), BlockValidationError> {
    // 1. Tx root
    let expected_tx_root = tx_root(&block.transactions);
    if block.header.tx_root != expected_tx_root {
        return Err(BlockValidationError::InvalidTxRoot);
    }

    // 1b. Transaction signatures (security review #16): from the activation height every tx must
    // carry a valid signature in the format required at this height; signatures present below it
    // must be valid too. Without this an importer cannot tell a forged tx from a real one.
    // Tips (when present) are bound into the signature preimage.
    verify_block_tx_signatures(block, executor)?;
    if !block.priority_tips.is_empty() && block.priority_tips.len() != block.transactions.len() {
        return Err(BlockValidationError::InvalidPriorityTips {
            expected: block.transactions.len(),
            found: block.priority_tips.len(),
        });
    }

    // 2. Proposer in validator set and is the round leader for this height
    if !validator_set.contains(&block.header.proposer) {
        return Err(BlockValidationError::InvalidProposer);
    }
    if block.header.proposer != expected_leader {
        return Err(BlockValidationError::NotRoundLeader {
            proposer: block.header.proposer,
            expected_leader,
        });
    }

    // 3. Execute on snapshot
    let mut state = parent_state.snapshot();
    let tips = &block.priority_tips;
    let (_gas, receipts) = executor
        .execute_block_with_tips(
            block.header.height,
            block.header.timestamp,
            &block.transactions,
            &mut state,
            block.header.proposer,
            tips,
        )
        .map_err(|e| BlockValidationError::ExecutionFailed(e.to_string()))?;

    // 4. Block reward
    let reward = executor.block_emission(block.header.height);
    if reward > 0 {
        match state.get_mut(&block.header.proposer) {
            Some(s) => s.balance = s.balance.saturating_add(reward),
            None => {
                state.insert(Account {
                    id: block.header.proposer,
                    state: AccountState {
                        balance: reward,
                        nonce: 0,
                        stake: 0, ..Default::default() },
                });
            }
        }
    }

    // 4b. Open validator-set bookkeeping (no-op unless open mode is configured).
    state.apply_open_validator_block_hook(block.header.height, block.header.proposer);

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

    // 7. Optional Phase 3 header blobs_root (activation-gated; omit before fork).
    let expected_blobs = if executor.vm().blobs_root_header_active(block.header.height) {
        Some(blobs_root_of_transactions(&block.transactions))
    } else {
        None
    };
    if block.header.blobs_root != expected_blobs {
        return Err(BlockValidationError::InvalidBlobsRoot {
            expected: expected_blobs,
            found: block.header.blobs_root,
        });
    }

    Ok((state, receipts))
}

/// Verify the block's tx signatures against the rules in force at its height.
pub fn verify_block_tx_signatures(block: &Block, executor: &BlockExecutor) -> Result<(), BlockValidationError> {
    let history = executor.execution_history();
    let height = block.header.height;
    let required = height >= crate::network_preset::block_sigs_from(history);
    if !required && block.signatures.is_empty() {
        return Ok(());
    }
    if block.signatures.len() != block.transactions.len() {
        return Err(BlockValidationError::MissingTxSignatures {
            expected: block.transactions.len(),
            found: block.signatures.len(),
        });
    }
    let policy = crate::network_preset::tx_sig_policy(
        crate::network_preset::effective_chain_id(history),
        history,
        height,
    );
    for (index, (tx, sig)) in block.transactions.iter().zip(&block.signatures).enumerate() {
        let tip = block.priority_tip_at(index);
        boing_primitives::verify_signature_with_tip(tx, sig, &tx.sender, policy, tip).map_err(
            |e| BlockValidationError::InvalidTxSignature {
                index,
                reason: e.to_string(),
            },
        )?;
    }
    Ok(())
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
    // Anchor mode: an anchor may propose at any height (fallback for an offline scheduled leader).
    let expected_leader = if consensus.may_propose(block.header.height, &block.header.proposer) {
        block.header.proposer
    } else {
        consensus.leader(block.header.height)
    };
    validate_and_execute_block(
        block,
        parent_state,
        validator_set,
        expected_leader,
        executor,
    )
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
    #[error("Proposer {proposer:?} is not round leader {expected_leader:?}")]
    NotRoundLeader {
        proposer: AccountId,
        expected_leader: AccountId,
    },
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Invalid state root: expected {expected:?}, computed {computed:?}")]
    InvalidStateRoot { expected: Hash, computed: Hash },
    #[error("Invalid blobs_root: expected {expected:?}, found {found:?}")]
    InvalidBlobsRoot {
        expected: Option<Hash>,
        found: Option<Hash>,
    },
    #[error("No validators configured")]
    NoValidators,
    #[error("Block must carry one signature per transaction: {expected} txs, {found} signatures")]
    MissingTxSignatures { expected: usize, found: usize },
    #[error("Block priority_tips length {found} does not match {expected} transactions")]
    InvalidPriorityTips { expected: usize, found: usize },
    #[error("Transaction {index} has an invalid signature: {reason}")]
    InvalidTxSignature { index: usize, reason: String },
}
