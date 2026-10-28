//! Storage-tree v2 (full-key routing) vs the legacy 64-bit routing.
//!
//! `legacy_state_root_is_pinned` uses only the long-stable public API, so the identical file runs
//! against the pre-v2 code base and pins the roots every existing chain already committed to. The
//! other tests prove v2 is off unless configured and closes the 64-bit-prefix collision when on.

use boing_primitives::{Account, AccountId, AccountState, Hash};
use boing_state::StateStore;

fn id(n: u64) -> AccountId {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&n.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_be_bytes());
    b[8..16].copy_from_slice(&(n ^ 0xABCD).to_le_bytes());
    AccountId(b)
}

fn key(prefix: u64, tail: u64) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[..8].copy_from_slice(&prefix.to_be_bytes());
    k[24..].copy_from_slice(&tail.to_be_bytes());
    k
}

fn word(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&n.to_be_bytes());
    w
}

/// Deterministic state with ordinary accounts, a contract with well-spread slots, and a contract
/// whose slots all share their first 8 bytes (the collision case).
fn sample_state() -> StateStore {
    let mut st = StateStore::new();
    for n in 0..40u64 {
        st.insert(Account {
            id: id(n),
            state: AccountState {
                balance: 1_000 + n as u128,
                nonce: n,
                ..Default::default()
            },
        });
    }
    let spread = id(1000);
    st.set_contract_code(spread, vec![1, 2, 3, 4]);
    for n in 0..30u64 {
        st.merge_contract_storage(
            spread,
            key(n.wrapping_mul(0x0123_4567_89AB_CDEF), n),
            word(n + 1),
        );
    }
    let colliding = id(2000);
    st.set_contract_code(colliding, vec![9, 9, 9]);
    for n in 0..8u64 {
        st.merge_contract_storage(colliding, key(7, n), word(100 + n));
    }
    st
}

/// Roots committed by the pre-v2 implementation for `sample_state()`.
const GOLDEN_LEGACY_ROOT: &str = "0834991abf34bb83570d2fb0fc038525613b818f52cea1b1effc6d0a76626955";

fn hex(h: &Hash) -> String {
    h.0.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn legacy_state_root_is_pinned() {
    let mut st = sample_state();
    let root = hex(&st.state_root());
    assert_eq!(root, GOLDEN_LEGACY_ROOT);
}

#[test]
fn inactive_by_default_never_changes_roots() {
    let mut st = sample_state();
    assert_eq!(st.storage_tree_v2_activation(), None);
    for h in [0u64, 1, 10, u64::MAX] {
        assert_eq!(
            hex(&st.state_root_at_height(h)),
            GOLDEN_LEGACY_ROOT,
            "height {h}"
        );
        assert!(!st.storage_tree_v2_active());
    }
    // Configured but not yet reached: still the legacy root.
    st.set_storage_tree_v2_activation(Some(1_000_000));
    assert_eq!(hex(&st.state_root_at_height(999_999)), GOLDEN_LEGACY_ROOT);
    assert_eq!(hex(&st.state_root_full_rebuild()), GOLDEN_LEGACY_ROOT);
}

#[test]
fn active_mode_commits_every_colliding_slot() {
    let colliding = id(2000);
    let mut legacy = sample_state();
    let legacy_root = legacy.state_root();

    // Legacy: the value of a non-smallest slot in a shared-prefix bucket is NOT committed.
    legacy.merge_contract_storage(colliding, key(7, 5), word(555));
    assert_eq!(
        legacy.state_root(),
        legacy_root,
        "legacy collision: change is invisible to the root"
    );

    // Active: the same edit moves the root.
    let mut v2 = sample_state();
    v2.set_storage_tree_v2_activation(Some(10));
    assert_eq!(hex(&v2.state_root_at_height(9)), GOLDEN_LEGACY_ROOT);
    let v2_root = v2.state_root_at_height(10);
    assert_ne!(
        v2_root, legacy_root,
        "activation changes the root of a state with colliding slots"
    );
    assert!(v2.storage_tree_v2_active());
    v2.merge_contract_storage(colliding, key(7, 5), word(555));
    assert_ne!(
        v2.state_root_at_height(10),
        v2_root,
        "v2 commits the colliding slot"
    );
    assert_eq!(v2.state_root_at_height(10), v2.state_root_full_rebuild());

    // Deleting back (restoring the old value) restores the v2 root.
    v2.merge_contract_storage(colliding, key(7, 5), word(105));
    assert_eq!(v2.state_root_at_height(10), v2_root);
}

#[test]
fn accounts_only_state_is_unaffected_by_activation() {
    // The account tree keeps 64-bit routing; only per-contract storage trees change.
    let mut only_accounts = StateStore::new();
    for n in 0..20u64 {
        only_accounts.insert(Account {
            id: id(n),
            state: AccountState {
                balance: n as u128 + 1,
                ..Default::default()
            },
        });
    }
    let a = only_accounts.state_root_at_height(0);
    only_accounts.set_storage_tree_v2_activation(Some(0));
    assert_eq!(
        only_accounts.state_root_at_height(0),
        a,
        "no storage => no change at activation"
    );
}

#[test]
fn crossing_the_activation_height_is_symmetric_and_deterministic() {
    let mut a = sample_state();
    a.set_storage_tree_v2_activation(Some(5));
    let r4 = a.state_root_at_height(4);
    let r5 = a.state_root_at_height(5);
    let r6 = a.state_root_at_height(6);
    assert_eq!(hex(&r4), GOLDEN_LEGACY_ROOT);
    assert_eq!(r5, r6, "no state change between heights >= activation");
    // Rewinding below the activation returns to the legacy root.
    assert_eq!(a.state_root_at_height(4), r4);

    // A fresh node that replays straight to the post-activation height agrees.
    let mut b = sample_state();
    b.set_storage_tree_v2_activation(Some(5));
    assert_eq!(b.state_root_at_height(6), r6);

    // Snapshots carry the mode and keep producing incremental roots.
    let mut snap = a.snapshot();
    assert_eq!(snap.storage_tree_v2_activation(), Some(5));
    assert_eq!(snap.state_root_at_height(6), r6);
}

#[test]
fn proofs_verify_under_both_modes() {
    let colliding = id(2000);
    for activation in [None, Some(0u64)] {
        let mut st = sample_state();
        st.set_storage_tree_v2_activation(activation);
        st.state_root_at_height(1);
        let proof = st
            .prove_storage(&colliding, &key(7, 3))
            .expect("slot present");
        if activation.is_some() {
            assert!(proof.verify(), "v2 storage proofs verify");
            assert_eq!(proof.root, st.storage_root_of(&colliding));
        }
        // Account proofs verify against the global root in both modes.
        let acct = st.prove_account(&id(3)).expect("account");
        assert!(acct.verify(), "account proofs verify (mode {activation:?})");
        assert!(st.prove_storage(&colliding, &key(7, 99)).is_none());
    }
}

#[test]
fn persistence_round_trip_matches_under_v2() {
    let mut st = sample_state();
    st.set_storage_tree_v2_activation(Some(0));
    let want = st.state_root_at_height(3);
    let (accounts, code, storage) = st.export_for_persistence();
    let mut loaded = StateStore::load_from_persistence(accounts, code, storage);
    loaded.set_storage_tree_v2_activation(Some(0));
    assert_eq!(loaded.state_root_at_height(3), want);
}

/// Random mutations + checkpoints/reverts: incremental roots equal a full rebuild in both modes.
#[test]
fn incremental_equals_full_rebuild_in_both_modes() {
    for activation in [None, Some(0u64)] {
        let mut st = sample_state();
        st.set_storage_tree_v2_activation(activation);
        let mut x: u64 = 0x1234_5678_9ABC_DEF1;
        let mut next = move || {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        let contract = id(2000);
        for step in 0..300u64 {
            let cp = st.checkpoint();
            for _ in 0..(next() % 4) {
                st.merge_contract_storage(
                    contract,
                    key(7 + next() % 2, next() % 6),
                    word(next() % 4),
                );
            }
            if next() % 3 == 0 {
                st.revert(cp);
            }
            if step % 5 == 0 {
                let inc = st.state_root_at_height(step);
                assert_eq!(
                    inc,
                    st.state_root_full_rebuild(),
                    "mode {activation:?} step {step}"
                );
            }
        }
    }
}
