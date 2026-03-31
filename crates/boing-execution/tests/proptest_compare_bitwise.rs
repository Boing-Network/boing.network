//! Property-based tests: compare and bitwise opcodes match unsigned / bitwise reference semantics.

use std::cmp::Ordering;

use boing_execution::Interpreter;
use boing_primitives::{Account, AccountId, AccountState};
use boing_state::StateStore;
use proptest::prelude::*;

const CONTRACT: AccountId = AccountId([1u8; 32]);

fn word_one() -> [u8; 32] {
    let mut w = [0u8; 32];
    w[31] = 1;
    w
}

fn push32(w: &[u8; 32]) -> Vec<u8> {
    let mut v = vec![0x7f];
    v.extend_from_slice(w);
    v
}

fn mk_state() -> StateStore {
    let mut state = StateStore::new();
    state.insert(Account {
        id: CONTRACT,
        state: AccountState::default(),
    });
    state
}

fn run_binary(op: u8, a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let mut code = push32(&a);
    code.extend(push32(&b));
    code.push(op);
    code.push(0x00);
    let mut it = Interpreter::new(code, 10_000_000);
    let mut state = mk_state();
    it.run(CONTRACT, CONTRACT, &[], &mut state).unwrap();
    assert_eq!(it.stack.len(), 1, "binary op must leave one word");
    it.stack[0]
}

fn run_unary(op: u8, a: [u8; 32]) -> [u8; 32] {
    let mut code = push32(&a);
    code.push(op);
    code.push(0x00);
    let mut it = Interpreter::new(code, 10_000_000);
    let mut state = mk_state();
    it.run(CONTRACT, CONTRACT, &[], &mut state).unwrap();
    assert_eq!(it.stack.len(), 1, "unary op must leave one word");
    it.stack[0]
}

fn expect_lt(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    if a.cmp(b) == Ordering::Less {
        word_one()
    } else {
        [0u8; 32]
    }
}

fn expect_gt(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    if a.cmp(b) == Ordering::Greater {
        word_one()
    } else {
        [0u8; 32]
    }
}

fn expect_eq(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    if a == b {
        word_one()
    } else {
        [0u8; 32]
    }
}

fn expect_is_zero(a: &[u8; 32]) -> [u8; 32] {
    if *a == [0u8; 32] {
        word_one()
    } else {
        [0u8; 32]
    }
}

fn expect_and(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..32 {
        o[i] = a[i] & b[i];
    }
    o
}

fn expect_or(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..32 {
        o[i] = a[i] | b[i];
    }
    o
}

fn expect_xor(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..32 {
        o[i] = a[i] ^ b[i];
    }
    o
}

fn expect_not(a: &[u8; 32]) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..32 {
        o[i] = !a[i];
    }
    o
}

fn arb_word() -> impl Strategy<Value = [u8; 32]> {
    proptest::collection::vec(any::<u8>(), 32).prop_map(|bytes| {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        arr
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn prop_lt_unsigned(a in arb_word(), b in arb_word()) {
        let got = run_binary(0x10, a, b);
        prop_assert_eq!(got, expect_lt(&a, &b));
    }

    #[test]
    fn prop_gt_unsigned(a in arb_word(), b in arb_word()) {
        let got = run_binary(0x11, a, b);
        prop_assert_eq!(got, expect_gt(&a, &b));
    }

    #[test]
    fn prop_eq_word(a in arb_word(), b in arb_word()) {
        let got = run_binary(0x14, a, b);
        prop_assert_eq!(got, expect_eq(&a, &b));
    }

    #[test]
    fn prop_iszero(a in arb_word()) {
        let got = run_unary(0x15, a);
        prop_assert_eq!(got, expect_is_zero(&a));
    }

    #[test]
    fn prop_and_bitwise(a in arb_word(), b in arb_word()) {
        let got = run_binary(0x16, a, b);
        prop_assert_eq!(got, expect_and(&a, &b));
    }

    #[test]
    fn prop_or_bitwise(a in arb_word(), b in arb_word()) {
        let got = run_binary(0x17, a, b);
        prop_assert_eq!(got, expect_or(&a, &b));
    }

    #[test]
    fn prop_xor_bitwise(a in arb_word(), b in arb_word()) {
        let got = run_binary(0x18, a, b);
        prop_assert_eq!(got, expect_xor(&a, &b));
    }

    #[test]
    fn prop_not_bitwise(a in arb_word()) {
        let got = run_unary(0x19, a);
        prop_assert_eq!(got, expect_not(&a));
    }
}
