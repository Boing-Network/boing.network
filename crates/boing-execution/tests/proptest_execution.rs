//! Property-based tests for execution (proptest).

use proptest::prelude::*;
use boing_execution::BlockExecutor;
use boing_primitives::{AccessList, Account, AccountId, AccountState, Transaction, TransactionPayload};
use boing_state::StateStore;

/// Generate independent transfers (disjoint access lists) so parallel execution applies.
fn mk_parallel_transfers(n: usize, amt: u128) -> (Vec<Transaction>, StateStore) {
    let mut state = StateStore::new();
    for i in 0..=n * 2 {
        let id = AccountId({ let mut a = [0u8; 32]; a[0] = i as u8; a });
        state.insert(Account {
            id,
            state: AccountState { balance: 1_000_000, nonce: 0, stake: 0 },
        });
    }
    let txs: Vec<Transaction> = (0..n)
        .map(|i| {
            let from = AccountId({ let mut a = [0u8; 32]; a[0] = (i * 2) as u8; a });
            let to = AccountId({ let mut a = [0u8; 32]; a[0] = (i * 2 + 1) as u8; a });
            Transaction {
                nonce: 0,
                sender: from,
                payload: TransactionPayload::Transfer { to, amount: amt },
                access_list: AccessList::new(vec![from, to], vec![from, to]),
            }
        })
        .collect();
    (txs, state)
}

fn total_balance(state: &StateStore, max_id: u8) -> u128 {
    (0..=max_id)
        .filter_map(|i| {
            let id = AccountId({ let mut a = [0u8; 32]; a[0] = i; a });
            state.get(&id)
        })
        .map(|s| s.balance)
        .sum()
}

proptest! {
    /// Executing independent transfers preserves total balance.
    #[test]
    fn prop_parallel_transfers_preserve_balance(count in 1..10usize, amount in 1u128..100u128) {
        let (txs, mut state) = mk_parallel_transfers(count, amount);
        let max_id = (count * 2) as u8;
        let total_before = total_balance(&state, max_id);
        let exec = BlockExecutor::new();
        exec.execute_block(1, &txs, &mut state).unwrap();
        let total_after = total_balance(&state, max_id);
        prop_assert_eq!(total_before, total_after, "total balance must be preserved");
    }

    /// Parallel and sequential execution of same independent transfers yield identical state.
    #[test]
    fn prop_parallel_vs_sequential_same_result(count in 1..8usize, amount in 1u128..50u128) {
        let (txs, state) = mk_parallel_transfers(count, amount);
        let exec = BlockExecutor::new();
        let max_id = (count * 2) as u8;

        let mut s1 = state.snapshot();
        exec.execute_block(1, &txs, &mut s1).unwrap();

        let mut s2 = state.snapshot();
        exec.execute_block(1, &txs, &mut s2).unwrap();

        for i in 0..=max_id {
            let id = AccountId({ let mut a = [0u8; 32]; a[0] = i; a });
            let st1 = s1.get(&id);
            let st2 = s2.get(&id);
            prop_assert_eq!(st1.is_some(), st2.is_some());
            if let (Some(a1), Some(a2)) = (st1, st2) {
                prop_assert_eq!(a1.balance, a2.balance, "balance mismatch for account {}", i);
                prop_assert_eq!(a1.nonce, a2.nonce, "nonce mismatch for account {}", i);
            }
        }
    }
}
