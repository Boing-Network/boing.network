//! State-root / snapshot micro-benchmark (std only, no extra deps).
//!
//! Run: `cargo bench -p boing-state --bench state_root`
//! Env: `BOING_BENCH_ACCOUNTS` (comma list, default `1000,10000,50000`), `BOING_BENCH_TOUCH` (default 200).
//!
//! Models one block: take a snapshot of the live state, touch `TOUCH` accounts (+ a few contract
//! storage slots), compute `state_root()`. Reports the median of several rounds in microseconds.

use std::time::Instant;

use boing_primitives::{Account, AccountId, AccountState};
use boing_state::StateStore;

fn id(i: u64) -> AccountId {
    let mut a = [0u8; 32];
    // Spread keys over the whole 256-bit space (like real ed25519 public keys).
    let h = blake3_like(i);
    a.copy_from_slice(&h);
    AccountId(a)
}

/// Cheap deterministic key spreader (splitmix64 x4) so the bench needs no hashing dependency.
fn blake3_like(seed: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    for chunk in out.chunks_mut(8) {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        chunk.copy_from_slice(&z.to_le_bytes());
    }
    out
}

fn build(n: u64) -> StateStore {
    let mut s = StateStore::new();
    for i in 0..n {
        s.insert(Account {
            id: id(i),
            state: AccountState {
                balance: 1_000_000 + i as u128,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
    }
    // 20 contracts x 50 slots.
    for c in 0..20u64 {
        let contract = id(10_000_000 + c);
        s.set_contract_code(contract, vec![1, 2, 3, c as u8]);
        for k in 0..50u64 {
            let mut key = [0u8; 32];
            key[..8].copy_from_slice(&k.to_le_bytes());
            let mut val = [0u8; 32];
            val[..8].copy_from_slice(&(k + c).to_le_bytes());
            s.merge_contract_storage(contract, key, val);
        }
    }
    s
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let sizes: Vec<u64> = std::env::var("BOING_BENCH_ACCOUNTS")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![1_000, 10_000, 50_000]);
    let touch: u64 = std::env::var("BOING_BENCH_TOUCH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    println!(
        "{:>10} {:>16} {:>16} {:>22}",
        "accounts", "cold_root_us", "snapshot_us", "block_cycle_us(touch)"
    );
    for &n in &sizes {
        let rounds = if n >= 50_000 { 5 } else { 9 };
        // Cold root (first computation).
        let mut cold = Vec::new();
        for _ in 0..rounds {
            let mut s = build(n);
            let t = Instant::now();
            std::hint::black_box(s.state_root());
            cold.push(t.elapsed().as_secs_f64() * 1e6);
        }
        // Snapshot cost.
        let mut live = build(n);
        live.state_root();
        let mut snap = Vec::new();
        for _ in 0..rounds {
            let t = Instant::now();
            let s = live.snapshot();
            snap.push(t.elapsed().as_secs_f64() * 1e6);
            std::hint::black_box(&s);
        }
        // Block cycle: snapshot -> touch -> root, then adopt the snapshot as the new live state
        // (exactly what produce_block / commit_pending do).
        let mut cycle = Vec::new();
        for r in 0..(rounds as u64 + 2) {
            let t = Instant::now();
            let mut next = live.snapshot();
            for j in 0..touch {
                let who = id((r * 7919 + j * 31) % n);
                if let Some(a) = next.get_mut(&who) {
                    a.nonce += 1;
                    a.balance -= 1;
                }
            }
            let contract = id(10_000_000 + (r % 20));
            let mut key = [0u8; 32];
            key[0] = (r % 50) as u8;
            next.merge_contract_storage(contract, key, [r as u8; 32]);
            std::hint::black_box(next.state_root());
            let dt = t.elapsed().as_secs_f64() * 1e6;
            live = next;
            if r >= 2 {
                cycle.push(dt); // first two rounds warm up (first adopted snapshot may be cold)
            }
        }
        println!(
            "{:>10} {:>16.0} {:>16.0} {:>22.0}",
            n,
            median(cold),
            median(snap),
            median(cycle)
        );
    }
}
