//! Block production — build blocks from mempool and consensus.

use tracing::{info, warn};

use boing_consensus::ConsensusEngine;
use boing_execution::BlockExecutor;
use boing_primitives::{
    receipts_root, tx_root, Account, AccountId, AccountState, Block, BlockHeader, ExecutionReceipt,
    Hash, Transaction,
};
use boing_state::StateStore;
use boing_tokenomics::block_emission_validators;

use crate::chain::ChainState;
use crate::mempool::Mempool;

/// Block producer — drains mempool, executes, builds block, proposes to consensus.
pub struct BlockProducer {
    proposer: AccountId,
    max_txs_per_block: usize,
}

impl BlockProducer {
    pub fn new(proposer: AccountId) -> Self {
        Self {
            proposer,
            max_txs_per_block: 1000,
        }
    }

    pub fn with_max_txs(mut self, max: usize) -> Self {
        self.max_txs_per_block = max;
        self
    }

    /// Produce and commit a block. Returns `(block_hash, receipts)` if successful.
    /// Only the round leader produces; other validators skip.
    pub fn produce_block(
        &self,
        chain: &ChainState,
        mempool: &Mempool,
        state: &mut StateStore,
        executor: &BlockExecutor,
        consensus: &mut ConsensusEngine,
    ) -> Option<(Hash, Vec<ExecutionReceipt>)> {
        let next_height = chain.height() + 1;
        if consensus.leader(next_height) != self.proposer {
            return None; // Not our turn to propose
        }
        let signed_txs = mempool.drain_for_block(self.max_txs_per_block);
        if signed_txs.is_empty() {
            return None;
        }
        let txs: Vec<Transaction> = signed_txs.iter().map(|s| s.tx.clone()).collect();

        let parent_hash = chain.parent_hash();
        let height = chain.height() + 1;
        let tx_root = tx_root(&txs);

        // Execute transactions; revert on failure and re-insert txs so they can be retried
        let checkpoint = state.checkpoint();
        let receipts = match executor.execute_block(height, &txs, state) {
            Ok((_gas, r)) => r,
            Err(e) => {
                warn!("Block execution failed: {}", e);
                state.revert(checkpoint);
                mempool.reinsert(signed_txs);
                return None;
            }
        };

        // Credit block reward to proposer
        let reward = block_emission_validators(height);
        if reward > 0 {
            match state.get_mut(&self.proposer) {
                Some(s) => s.balance = s.balance.saturating_add(reward),
                None => {
                    state.insert(Account {
                        id: self.proposer,
                        state: AccountState {
                            balance: reward,
                            nonce: 0,
                            stake: 0,
                        },
                    });
                }
            }
        }

        let state_root = state.state_root();
        let rr = receipts_root(&receipts);

        let block = Block {
            header: BlockHeader {
                parent_hash,
                height,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                proposer: self.proposer,
                tx_root,
                receipts_root: rr,
                state_root,
            },
            transactions: txs,
        };

        match consensus.propose_and_commit(block.clone()) {
            Ok(hash) => {
                if let Err(e) = chain.append(block) {
                    warn!("Failed to append block to chain: {}", e);
                    state.revert(checkpoint);
                    mempool.reinsert(signed_txs);
                    return None;
                }
                info!("Block committed: height={} hash={:?}", height, hash);
                Some((hash, receipts))
            }
            Err(e) => {
                warn!("Consensus failed: {}", e);
                state.revert(checkpoint);
                mempool.reinsert(signed_txs);
                None
            }
        }
    }
}
