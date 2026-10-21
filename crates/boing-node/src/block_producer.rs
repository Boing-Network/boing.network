//! Block production — build blocks from mempool (consensus commit is handled by the node).

use tracing::info;

use boing_consensus::ConsensusEngine;
use boing_execution::BlockExecutor;
use boing_primitives::{
    blobs_root_of_transactions, receipts_root, tx_root, Account, AccountId, AccountState, Block,
    BlockHeader, ExecutionReceipt, Transaction,
};
use boing_state::StateStore;

use crate::chain::ChainState;
use crate::mempool::Mempool;

/// A locally produced block proposal: executed state is returned separately so the live tip
/// can stay unchanged until consensus quorum commits.
pub struct ProducedProposal {
    pub block: Block,
    pub receipts: Vec<ExecutionReceipt>,
    pub new_state: StateStore,
}

/// Block producer — drains mempool, executes, builds block. Does not append to the chain.
pub struct BlockProducer {
    proposer: AccountId,
    max_txs_per_block: usize,
    /// Phase 3: when `Some(h)`, headers at height ≥ `h` carry `blobs_root`.
    blobs_root_header_from: Option<u64>,
}

impl BlockProducer {
    pub fn new(proposer: AccountId) -> Self {
        Self {
            proposer,
            max_txs_per_block: 1000,
            blobs_root_header_from: None,
        }
    }

    pub fn proposer(&self) -> AccountId {
        self.proposer
    }

    pub fn with_max_txs(mut self, max: usize) -> Self {
        self.max_txs_per_block = max;
        self
    }

    pub fn with_blobs_root_header_from(mut self, from: Option<u64>) -> Self {
        self.blobs_root_header_from = from;
        self
    }

    pub fn set_blobs_root_header_from(&mut self, from: Option<u64>) {
        self.blobs_root_header_from = from;
    }

    /// Build a block proposal if this node is the round leader and the mempool is non-empty.
    ///
    /// Mutates `state` only transiently: on success the tip is restored and the post-execution
    /// state is returned in [`ProducedProposal::new_state`]. On execution failure, txs are
    /// re-inserted into the mempool.
    pub fn produce_proposal(
        &self,
        chain: &ChainState,
        mempool: &Mempool,
        state: &mut StateStore,
        executor: &BlockExecutor,
        consensus: &ConsensusEngine,
    ) -> Option<ProducedProposal> {
        self.produce_proposal_with_fallback(chain, mempool, state, executor, consensus, false)
    }

    /// Like [`Self::produce_proposal`]; with `anchor_fallback`, an anchor validator may propose even
    /// when it is not the scheduled leader (open validator-set mode, leader presumed offline).
    pub fn produce_proposal_with_fallback(
        &self,
        chain: &ChainState,
        mempool: &Mempool,
        state: &mut StateStore,
        executor: &BlockExecutor,
        consensus: &ConsensusEngine,
        anchor_fallback: bool,
    ) -> Option<ProducedProposal> {
        let next_height = chain.height() + 1;
        let is_leader = consensus.leader(next_height) == self.proposer;
        let fallback_ok = anchor_fallback && consensus.may_propose(next_height, &self.proposer);
        if !is_leader && !fallback_ok {
            return None; // Not our turn to propose
        }
        let mut signed_txs = mempool.drain_for_block(self.max_txs_per_block);
        let parent_hash = chain.parent_hash();
        let height = chain.height() + 1;
        // Importers verify every tx signature against the rule for this height (security review
        // #16). A tx admitted under an earlier rule (e.g. a legacy signature before the v2
        // activation) would make the whole block invalid, so drop such txs here.
        let history = executor.execution_history();
        let policy = crate::network_preset::tx_sig_policy(
            crate::network_preset::effective_chain_id(history),
            history,
            height,
        );
        signed_txs.retain(|s| s.verify_with(policy).is_ok());
        if signed_txs.is_empty() {
            return None;
        }
        let txs: Vec<Transaction> = signed_txs.iter().map(|s| s.tx.clone()).collect();
        let signatures: Vec<boing_primitives::Signature> = signed_txs.iter().map(|s| s.signature).collect();
        let priority_tips: Vec<u128> = signed_txs.iter().map(|s| s.priority_tip).collect();
        // Omit all-zero tip vector from the block encoding (historical wire compatibility).
        let priority_tips_field: Vec<u128> = if priority_tips.iter().any(|&t| t != 0) {
            priority_tips.clone()
        } else {
            Vec::new()
        };

        let block_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let tx_root = tx_root(&txs);

        let checkpoint = state.checkpoint();
        let receipts = match executor.execute_block_with_tips(
            height,
            block_timestamp,
            &txs,
            state,
            self.proposer,
            &priority_tips,
        ) {
            Ok((_gas, r)) => r,
            Err(e) => {
                boing_telemetry::component_warn(
                    "boing_node::block_producer",
                    "block_producer",
                    "block_execution_failed",
                    e,
                );
                state.revert(checkpoint);
                mempool.reinsert(signed_txs);
                return None;
            }
        };

        // Credit block reward to proposer
        let reward = executor.block_emission(height);
        if reward > 0 {
            match state.get_mut(&self.proposer) {
                Some(s) => s.balance = s.balance.saturating_add(reward),
                None => {
                    state.insert(Account {
                        id: self.proposer,
                        state: AccountState {
                            balance: reward,
                            nonce: 0,
                            stake: 0, ..Default::default() },
                    });
                }
            }
        }

        state.apply_open_validator_block_hook(height, self.proposer);

        let state_root = state.state_root();
        let rr = receipts_root(&receipts);
        let blobs_root = if self
            .blobs_root_header_from
            .is_some_and(|from| height >= from)
        {
            Some(blobs_root_of_transactions(&txs))
        } else {
            None
        };

        let block = Block {
            header: BlockHeader {
                parent_hash,
                height,
                timestamp: block_timestamp,
                proposer: self.proposer,
                tx_root,
                receipts_root: rr,
                state_root,
                blobs_root,
            },
            transactions: txs,
            signatures,
            priority_tips: priority_tips_field,
        };

        let new_state = state.snapshot();
        state.revert(checkpoint);
        info!(
            "Block proposed: height={} hash={:?}",
            height,
            block.hash()
        );
        Some(ProducedProposal {
            block,
            receipts,
            new_state,
        })
    }
}
