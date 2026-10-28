//! Mempool admission micro-benchmark (std only).
//!
//! Run: `cargo bench -p boing-node --bench admission`
//! Env: `BOING_BENCH_ACCOUNTS` (comma list, default `1000,10000,50000`), `BOING_BENCH_ITERS` (default 200).
//!
//! Compares the cost of deciding "can this Transfer's sender pay?" with the clone-free fast path
//! (`precheck_transfer_fast`, used by `submit_transaction`) versus the original snapshot + VM dry
//! run (`precheck_sender_can_pay_fee_full`), plus a full `submit_transaction` round trip.
//! Reports the median per-call time in microseconds.

use std::time::Instant;

use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn median_us(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let sizes: Vec<usize> = std::env::var("BOING_BENCH_ACCOUNTS")
        .unwrap_or_else(|_| "1000,10000,50000".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let iters: usize = std::env::var("BOING_BENCH_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    println!(
        "{:>10} {:>14} {:>14} {:>16}",
        "accounts", "fast (us)", "dry-run (us)", "submit (us)"
    );
    for &n in &sizes {
        let sk = SigningKey::generate(&mut OsRng);
        let sender = AccountId(sk.verifying_key().to_bytes());
        let proposer = AccountId([9u8; 32]);
        let mut node = BoingNode::with_validators(vec![proposer], proposer, None);
        for i in 0..n as u64 {
            let mut id = [0u8; 32];
            id[..8].copy_from_slice(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_le_bytes());
            id[8..16].copy_from_slice(&i.to_le_bytes());
            node.state.insert(Account {
                id: AccountId(id),
                state: AccountState {
                    balance: 1_000_000_000_000,
                    ..Default::default()
                },
            });
        }
        node.state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000_000_000_000_000_000,
                ..Default::default()
            },
        });
        let to = AccountId([2u8; 32]);
        let mk = |nonce: u64| {
            let tx = Transaction {
                nonce,
                sender,
                payload: TransactionPayload::Transfer { to, amount: 1 },
                access_list: AccessList::new(vec![sender, to], vec![sender, to]),
            };
            SignedTransaction::new(tx, &sk)
        };
        let probe = mk(0);
        // Warm the state-root / account caches once.
        let _ = node.state.state_root();

        let mut fast = Vec::new();
        let mut full = Vec::new();
        for _ in 0..iters {
            let t = Instant::now();
            let r = node.precheck_transfer_fast(&probe, &to, 1);
            fast.push(t.elapsed().as_secs_f64() * 1e6);
            assert!(r.is_ok());
            let t = Instant::now();
            let r = node.precheck_sender_can_pay_fee_full(&probe);
            full.push(t.elapsed().as_secs_f64() * 1e6);
            assert!(r.is_ok());
        }
        let mut sub = Vec::new();
        for nonce in 0..iters.min(12) as u64 {
            let s = mk(nonce);
            let t = Instant::now();
            node.submit_transaction(s).expect("admit");
            sub.push(t.elapsed().as_secs_f64() * 1e6);
        }
        println!(
            "{:>10} {:>14.1} {:>14.1} {:>16.1}",
            n,
            median_us(fast),
            median_us(full),
            median_us(sub)
        );
    }
}
