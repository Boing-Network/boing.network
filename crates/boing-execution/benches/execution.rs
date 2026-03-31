//! Benchmarks: sequential vs parallel block execution.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use boing_execution::BlockExecutor;
use boing_primitives::{AccessList, Account, AccountId, AccountState, Transaction, TransactionPayload};
use boing_state::StateStore;

/// Independent transfers (disjoint access lists) — triggers parallel execution.
fn mk_parallel_transfers(n: usize) -> (Vec<Transaction>, StateStore) {
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
                payload: TransactionPayload::Transfer { to, amount: 1 },
                access_list: AccessList::new(vec![from, to], vec![from, to]),
            }
        })
        .collect();
    (txs, state)
}

/// Transfers that share accounts — triggers sequential execution.
fn mk_sequential_transfers(n: usize) -> (Vec<Transaction>, StateStore) {
    let mut state = StateStore::new();
    for i in 0..=n {
        let id = AccountId({ let mut a = [0u8; 32]; a[0] = i as u8; a });
        state.insert(Account {
            id,
            state: AccountState { balance: 1_000_000, nonce: 0, stake: 0 },
        });
    }
    let shared = AccountId([0u8; 32]);
    let txs: Vec<Transaction> = (0..n)
        .map(|i| {
            let from = AccountId({ let mut a = [0u8; 32]; a[0] = (i % 2 + 1) as u8; a });
            let to = AccountId({ let mut a = [0u8; 32]; a[0] = (i % 2 + 2) as u8; a });
            let nonce = (i / 2) as u64; // per-sender: 0,0,1,1,2,2,...
            Transaction {
                nonce,
                sender: from,
                payload: TransactionPayload::Transfer { to, amount: 1 },
                access_list: AccessList::new(vec![from, to, shared], vec![from, to]),
            }
        })
        .collect();
    (txs, state)
}

fn bench_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("execute_block");
    for n in [10, 50, 100].iter() {
        group.bench_with_input(criterion::BenchmarkId::new("parallel", n), n, |b, &n| {
            let (txs, state) = mk_parallel_transfers(n);
            let exec = BlockExecutor::new();
            b.iter(|| {
                let mut s = state.snapshot();
                exec.execute_block(1, black_box(&txs), &mut s).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("execute_block");
    for n in [10, 50, 100].iter() {
        group.bench_with_input(criterion::BenchmarkId::new("sequential", n), n, |b, &n| {
            let (txs, state) = mk_sequential_transfers(n);
            let exec = BlockExecutor::new();
            b.iter(|| {
                let mut s = state.snapshot();
                exec.execute_block(1, black_box(&txs), &mut s).unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parallel, bench_sequential);
criterion_main!(benches);
