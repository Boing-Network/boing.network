//! Account-tree v3 (full-key routing of the global account tree, audit #61 item 30) vs the legacy
//! 64-bit routing: off unless configured, identical roots before the activation height, and every
//! account committed from it.

use boing_primitives::{Account, AccountId, AccountState};
use boing_state::StateStore;

/// Account ids that share their first 8 bytes (the collision case) and differ after.
fn colliding(tail: u8) -> AccountId {
    let mut b = [0xAB; 32];
    b[31] = tail;
    AccountId(b)
}

fn spread(n: u8) -> AccountId {
    AccountId([n; 32])
}

fn put(st: &mut StateStore, id: AccountId, balance: u128) {
    st.insert(Account {
        id,
        state: AccountState {
            balance,
            ..Default::default()
        },
    });
}

fn sample() -> StateStore {
    let mut st = StateStore::new();
    for n in 1..20u8 {
        put(&mut st, spread(n), 1_000 + n as u128);
    }
    put(&mut st, colliding(1), 10);
    put(&mut st, colliding(2), 20);
    st
}

#[test]
fn off_by_default_and_unchanged_below_activation() {
    let mut plain = sample();
    let legacy = plain.state_root();
    let mut gated = sample();
    gated.set_account_tree_v3_activation(Some(100));
    assert_eq!(gated.state_root_at_height(99), legacy);
    assert!(!gated.account_tree_v3_active());
    assert_eq!(plain.state_root_at_height(1_000_000), legacy);
}

#[test]
fn legacy_hides_colliding_account_and_v3_commits_it() {
    // Legacy: the larger of two ids sharing 64 bits is not committed (the quirk being fixed).
    let mut a = sample();
    let mut b = sample();
    put(&mut b, colliding(2), 999);
    assert_eq!(a.state_root(), b.state_root());

    // v3: every account is committed.
    let mut a = sample();
    let mut b = sample();
    put(&mut b, colliding(2), 999);
    a.set_account_tree_v3_activation(Some(5));
    b.set_account_tree_v3_activation(Some(5));
    let ra = a.state_root_at_height(5);
    assert!(a.account_tree_v3_active());
    assert_ne!(ra, b.state_root_at_height(5));
    assert_eq!(ra, a.state_root_full_rebuild());
}

#[test]
fn v3_switch_is_incremental_safe_and_proofs_verify() {
    let mut st = sample();
    st.set_account_tree_v3_activation(Some(10));
    let before = st.state_root_at_height(9);
    let after = st.state_root_at_height(10);
    assert_ne!(before, after);
    put(&mut st, spread(3), 77);
    let r = st.state_root_at_height(11);
    assert_eq!(r, st.state_root_full_rebuild());
    for id in [spread(3), colliding(1), colliding(2)] {
        let p = st.prove_account(&id).expect("proof");
        assert!(p.verify());
        assert_eq!(p.root, r);
    }
    // Rewind below the activation restores the legacy root.
    assert_eq!(sample().state_root(), {
        let mut s = sample();
        s.set_account_tree_v3_activation(Some(10));
        s.state_root_at_height(10);
        s.state_root_at_height(9)
    });
}
