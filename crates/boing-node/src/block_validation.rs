//! Block import and validation — validate blocks from peers.

use std::time::{SystemTime, UNIX_EPOCH};

use boing_consensus::ConsensusEngine;
use boing_execution::{apply_media_blob_block_housekeeping, BlockExecutor};
use boing_primitives::{
    blobs_root_of_transactions, receipts_root, tx_root, Account, AccountId, AccountState, Block,
    ExecutionReceipt, Hash,
};
use boing_state::StateStore;

/// How far ahead of wall-clock a block timestamp may be (seconds).
///
/// Keeps imported blocks from jumping far into the future while allowing modest clock skew
/// between producers and importers (issue #61 item 15).
/// Sized for burst production in the same second (`max(now, parent+1)` advances by one per
/// block): catch-up and name-registry tests can produce hundreds of blocks faster than wall
/// clock, so a few minutes of skew is required.
pub const MAX_BLOCK_TIMESTAMP_FUTURE_SECS: u64 = 600;

/// Validate and execute a block. Returns updated state and per-tx receipts on success.
/// Caller must ensure block chains to parent (parent_hash, height).
pub fn validate_and_execute_block(
    block: &Block,
    parent_state: &StateStore,
    validator_set: &[AccountId],
    expected_leader: AccountId,
    executor: &BlockExecutor,
    parent_timestamp: u64,
) -> Result<(StateStore, Vec<ExecutionReceipt>), BlockValidationError> {
    // 0. Timestamp ordering + future skew (issue #61 item 15).
    validate_block_timestamp(block, parent_timestamp)?;

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
    // Phase 14: pin consensus validators for ResolveSlashAppeal (not committed).
    state.set_slash_appeal_resolvers(Some(validator_set.to_vec()));
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

    // 4. Block reward (clamped to MAX_SUPPLY hard cap; must match producer).
    let reward = {
        let uncapped = executor.block_emission(block.header.height);
        let held = state.compute_native_aggregates().total_native_held;
        boing_tokenomics::clamp_emission_to_hard_cap(
            uncapped,
            held,
            executor.denomination_at(block.header.height),
        )
    };
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
                        ..Default::default()
                    },
                });
            }
        }
    }

    // 4b. Phase 11–13: settle FailedServe + prune settle receipts (deterministic; matches producer).
    apply_media_blob_block_housekeeping(&mut state, block.header.height)
        .map_err(|e| BlockValidationError::ExecutionFailed(e.to_string()))?;

    // 4c. Open validator-set bookkeeping (no-op unless open mode is configured).
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
    let computed_root = state.state_root_at_height(block.header.height);
    if block.header.state_root != computed_root {
        return Err(BlockValidationError::InvalidStateRoot {
            expected: block.header.state_root,
            computed: computed_root,
        });
    }

    // 7. Phase 3+ header blobs_root (activation-gated; omitted before fork).
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
pub fn verify_block_tx_signatures(
    block: &Block,
    executor: &BlockExecutor,
) -> Result<(), BlockValidationError> {
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

/// Enforce non-decreasing timestamps vs parent (except genesis) and reject far-future clocks.
///
/// Equal timestamps are allowed: recorded testnet history (and same-second production) reuse the
/// parent second. Strict `>` would reject valid historical blocks on replay.
pub fn validate_block_timestamp(
    block: &Block,
    parent_timestamp: u64,
) -> Result<(), BlockValidationError> {
    if block.header.height > 0 && block.header.timestamp < parent_timestamp {
        return Err(BlockValidationError::TimestampBeforeParent {
            parent: parent_timestamp,
            got: block.header.timestamp,
        });
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let max_allowed = now.saturating_add(MAX_BLOCK_TIMESTAMP_FUTURE_SECS);
    if block.header.timestamp > max_allowed {
        return Err(BlockValidationError::TimestampTooFarInFuture {
            got: block.header.timestamp,
            max_allowed,
        });
    }
    Ok(())
}

/// Full import: validate block and return new state + receipts if it chains and is valid.
pub fn import_block(
    block: &Block,
    our_latest_hash: Hash,
    our_height: u64,
    parent_state: &StateStore,
    consensus: &ConsensusEngine,
    executor: &BlockExecutor,
    parent_timestamp: u64,
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
        parent_timestamp,
    )
}

#[derive(Debug, thiserror::Error)]
pub enum BlockValidationError {
    #[error("Block does not chain to our tip")]
    DoesNotChain,
    #[error("Block timestamp {got} is before parent timestamp {parent}")]
    TimestampBeforeParent { parent: u64, got: u64 },
    #[error("Block timestamp {got} is too far in the future (max allowed {max_allowed})")]
    TimestampTooFarInFuture { got: u64, max_allowed: u64 },
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
