//! Block production — build blocks from mempool (consensus commit is handled by the node).

use tracing::info;

use boing_consensus::ConsensusEngine;
use boing_execution::{apply_media_blob_block_housekeeping, BlockExecutor};
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
        let signatures: Vec<boing_primitives::Signature> =
            signed_txs.iter().map(|s| s.signature).collect();
        let priority_tips: Vec<u128> = signed_txs.iter().map(|s| s.priority_tip).collect();
        // Omit all-zero tip vector from the block encoding (historical wire compatibility).
        let priority_tips_field: Vec<u128> = if priority_tips.iter().any(|&t| t != 0) {
            priority_tips.clone()
        } else {
            Vec::new()
        };

        // Prefer wall-clock, but never emit a timestamp ≤ parent (issue #61 item 15). On clock
        // failure, advance from the parent rather than silently using the Unix epoch (0).
        let parent_ts = chain
            .get_block_by_height(chain.height())
            .map(|b| b.header.timestamp)
            .unwrap_or(0);
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let block_timestamp = now_ts.max(parent_ts.saturating_add(1));
        let tx_root = tx_root(&txs);

        let checkpoint = state.checkpoint();
        // Phase 14: pin consensus validators for ResolveSlashAppeal (not committed).
        state.set_slash_appeal_resolvers(Some(consensus.validators().to_vec()));
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

        // Credit block reward to proposer (clamped to MAX_SUPPLY hard cap).
        let reward = {
            let uncapped = executor.block_emission(height);
            let held = state.compute_native_aggregates().total_native_held;
            boing_tokenomics::clamp_emission_to_hard_cap(
                uncapped,
                held,
                executor.denomination_at(height),
            )
        };
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
                            ..Default::default()
                        },
                    });
                }
            }
        }

        // Phase 11–13: settle FailedServe + prune stale settle receipts even without sample txs.
        if let Err(e) = apply_media_blob_block_housekeeping(state, height) {
            boing_telemetry::component_warn(
                "boing_node::block_producer",
                "block_producer",
                "serve_challenge_settle_failed",
                e,
            );
            state.revert(checkpoint);
            mempool.reinsert(signed_txs);
            return None;
        }

        state.apply_open_validator_block_hook(height, self.proposer);

        let state_root = state.state_root_at_height(height);
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
        info!("Block proposed: height={} hash={:?}", height, block.hash());
        Some(ProducedProposal {
            block,
            receipts,
            new_state,
        })
    }
}
