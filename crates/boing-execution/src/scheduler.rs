//! Parallel transaction scheduler using declared access lists.
//!
//! Transactions with disjoint access lists execute in parallel.

use boing_primitives::Transaction;

/// Groups transactions into parallel batches based on access list conflicts.
pub struct TransactionScheduler;

impl TransactionScheduler {
    pub fn new() -> Self {
        Self
    }

    /// Schedule transactions into batches. Batches can run in parallel; within a batch, txs are sequential.
    pub fn schedule(&self, txs: &[Transaction]) -> Vec<Vec<usize>> {
        let mut batches: Vec<Vec<usize>> = vec![];
        let mut assigned = vec![false; txs.len()];

        for i in 0..txs.len() {
            if assigned[i] {
                continue;
            }
            let mut batch = vec![i];
            assigned[i] = true;

            for j in (i + 1)..txs.len() {
                if assigned[j] {
                    continue;
                }
                let conflicts = batch
                    .iter()
                    .any(|&k| txs[k].access_list.conflicts_with(&txs[j].access_list));
                if !conflicts {
                    batch.push(j);
                    assigned[j] = true;
                }
            }
            batches.push(batch);
        }
        batches
    }
}

impl Default for TransactionScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, AccountId, Transaction, TransactionPayload};

    fn tx_with_access(accounts: Vec<AccountId>) -> Transaction {
        Transaction {
            nonce: 0,
            sender: accounts[0],
            payload: TransactionPayload::Transfer {
                to: accounts.get(1).copied().unwrap_or(accounts[0]),
                amount: 1,
            },
            access_list: AccessList::new(accounts.clone(), accounts),
        }
    }

    #[test]
    fn test_scheduler_independent_txs_in_same_batch() {
        let s = TransactionScheduler::new();
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let txs = vec![tx_with_access(vec![a, b]), tx_with_access(vec![c])];
        let batches = s.schedule(&txs);
        assert_eq!(batches.len(), 1, "Independent txs should be in one batch");
        assert_eq!(batches[0].len(), 2);
    }

    #[test]
    fn test_scheduler_conflicting_txs_separate_batches() {
        let s = TransactionScheduler::new();
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let txs = vec![tx_with_access(vec![a, b]), tx_with_access(vec![a])];
        let batches = s.schedule(&txs);
        assert_eq!(
            batches.len(),
            2,
            "Conflicting txs should be in separate batches"
        );
    }
}
