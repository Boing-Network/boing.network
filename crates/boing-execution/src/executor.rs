//! Block executor — runs transactions through scheduler + VM.
//!
//! Batches run sequentially; within each batch, Transfer-only txs run in parallel via rayon.

use rayon::prelude::*;

use boing_primitives::{ExecutionReceipt, Transaction, TransactionPayload};
use boing_state::StateStore;

use boing_qa::RuleRegistry;

use super::parallel::ExecutionView;
use super::{TransactionScheduler, Vm, VmError};

/// Executes a block of transactions. Batches run sequentially; within each batch,
/// Transfer-only txs run in parallel.
pub struct BlockExecutor {
    vm: Vm,
    scheduler: TransactionScheduler,
}

impl BlockExecutor {
    pub fn new() -> Self {
        Self {
            vm: Vm::new(),
            scheduler: TransactionScheduler::new(),
        }
    }

    /// Same QA registry as the node mempool so block execution applies identical deploy rules.
    pub fn with_qa_registry(registry: RuleRegistry) -> Self {
        Self {
            vm: Vm::with_qa_registry(registry),
            scheduler: TransactionScheduler::new(),
        }
    }

    /// Execute all transactions. Returns total gas used and one receipt per tx (in block order).
    /// On error, state may be partially applied (caller should revert if needed).
    /// Transfer-only batches run in parallel; other batches run sequentially.
    pub fn execute_block(
        &self,
        block_height: u64,
        txs: &[Transaction],
        state: &mut StateStore,
    ) -> Result<(u64, Vec<ExecutionReceipt>), ExecutionError> {
        let batches = self.scheduler.schedule(txs);
        let mut total_gas = 0u64;
        let mut receipts: Vec<Option<ExecutionReceipt>> = vec![None; txs.len()];

        for batch in batches {
            let all_transfer = batch.iter().all(|&i| {
                matches!(&txs[i].payload, TransactionPayload::Transfer { .. })
            });

            if all_transfer && batch.len() > 1 {
                // Parallel path: copy state slice per tx (sequential read), execute in parallel, merge
                let snapshots: Vec<_> = batch
                    .iter()
                    .map(|&idx| {
                        let tx = txs[idx].clone();
                        let ids: Vec<_> = tx.access_list.all().copied().collect();
                        let snapshot: std::collections::HashMap<_, _> = ids
                            .iter()
                            .filter_map(|id| state.get(id).map(|s| (*id, s.clone())))
                            .collect();
                        (idx, tx, snapshot)
                    })
                    .collect();

                let batch_results: Result<Vec<_>, ExecutionError> = snapshots
                    .par_iter()
                    .map(|(idx, tx, snapshot)| {
                        let mut view = ExecutionView::from_snapshot(snapshot.clone());
                        let out = self
                            .vm
                            .execute_transfer(tx, &mut view)
                            .map_err(ExecutionError::Vm)?;
                        Ok((*idx, view, out.gas_used))
                    })
                    .collect();

                let batch_results = batch_results?;
                // Sanity check: verify no conflicting writes (access lists should be disjoint)
                let mut written: std::collections::HashSet<boing_primitives::AccountId> =
                    std::collections::HashSet::new();
                for (_, view, _) in &batch_results {
                    for id in view.account_ids() {
                        if !written.insert(*id) {
                            return Err(ExecutionError::ConflictDetected(format!(
                                "Parallel batch wrote to same account: {:?}",
                                id
                            )));
                        }
                    }
                }
                for (idx, view, gas) in batch_results {
                    receipts[idx] = Some(ExecutionReceipt::from_tx_outcome(
                        &txs[idx],
                        block_height,
                        idx as u32,
                        true,
                        gas,
                        Vec::new(),
                        vec![],
                        None,
                    ));
                    view.merge_into(state);
                    total_gas = total_gas.saturating_add(gas);
                }
            } else {
                // Sequential path
                for &idx in &batch {
                    let tx = &txs[idx];
                    match self.vm.execute(tx, state) {
                        Ok(out) => {
                            total_gas = total_gas.saturating_add(out.gas_used);
                            receipts[idx] = Some(ExecutionReceipt::from_tx_outcome(
                                tx,
                                block_height,
                                idx as u32,
                                true,
                                out.gas_used,
                                out.return_data,
                                out.logs,
                                None,
                            ));
                        }
                        Err(e) => {
                            receipts[idx] = Some(ExecutionReceipt::from_tx_outcome(
                                tx,
                                block_height,
                                idx as u32,
                                false,
                                0,
                                Vec::new(),
                                vec![],
                                Some(format!("{}", e)),
                            ));
                            return Err(ExecutionError::Vm(e));
                        }
                    }
                }
            }
        }
        let receipts: Vec<ExecutionReceipt> = receipts
            .into_iter()
            .enumerate()
            .map(|(i, r)| r.unwrap_or_else(|| panic!("internal error: missing receipt at index {i}")))
            .collect();
        Ok((total_gas, receipts))
    }
}

impl Default for BlockExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, Account, AccountId, AccountState, Transaction, TransactionPayload};

    fn tx(sender: AccountId, to: AccountId, nonce: u64, amount: u128) -> Transaction {
        Transaction {
            nonce,
            sender,
            payload: TransactionPayload::Transfer { to, amount },
            access_list: AccessList::new(vec![sender, to], vec![sender, to]),
        }
    }

    #[test]
    fn test_execute_block() {
        let exec = BlockExecutor::new();
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: a,
            state: AccountState { balance: 1000, nonce: 0, stake: 0 },
        });
        state.insert(Account {
            id: b,
            state: AccountState { balance: 0, nonce: 0, stake: 0 },
        });
        let txs = vec![tx(a, b, 0, 100)];
        let (gas, receipts) = exec.execute_block(1, &txs, &mut state).unwrap();
        assert_eq!(gas, super::super::vm::GAS_PER_TRANSFER);
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].success);
        assert_eq!(state.get(&a).unwrap().balance, 900);
        assert_eq!(state.get(&b).unwrap().balance, 100);
    }

    #[test]
    fn test_execute_block_parallel_transfers() {
        let exec = BlockExecutor::new();
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let d = AccountId::from_bytes([4u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account { id: a, state: AccountState { balance: 1000, nonce: 0, stake: 0 } });
        state.insert(Account { id: b, state: AccountState { balance: 0, nonce: 0, stake: 0 } });
        state.insert(Account { id: c, state: AccountState { balance: 500, nonce: 0, stake: 0 } });
        state.insert(Account { id: d, state: AccountState { balance: 0, nonce: 0, stake: 0 } });
        // Independent transfers a->b and c->d — same batch, parallel execution
        let txs = vec![
            tx(a, b, 0, 100),
            tx(c, d, 0, 50),
        ];
        let (gas, receipts) = exec.execute_block(1, &txs, &mut state).unwrap();
        assert_eq!(receipts.len(), 2);
        assert!(receipts.iter().all(|r| r.success));
        assert_eq!(state.get(&a).unwrap().balance, 900);
        assert_eq!(state.get(&b).unwrap().balance, 100);
        assert_eq!(state.get(&c).unwrap().balance, 450);
        assert_eq!(state.get(&d).unwrap().balance, 50);
        assert_eq!(gas, super::super::vm::GAS_PER_TRANSFER * 2);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("VM error: {0}")]
    Vm(#[from] VmError),
    #[error("Conflict detected: {0}")]
    ConflictDetected(String),
}
