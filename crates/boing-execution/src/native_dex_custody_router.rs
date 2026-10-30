//! **Custody router (v1)** for token-pulling pools (**v8 / v9**): pulls the input from the
//! transaction signer with reference-token v2 `transfer_from`, then forwards 1..=4 sequential
//! `swap_to` calls.
//!
//! Older routers (`native_dex_ledger_router`, `native_dex_multihop_swap_router`) forward with
//! `Caller = router`; on v8 the pool would pull from the router's own balance. This router instead:
//!
//! 1. `transfer_from(token_in[0], CALLER -> router, amount_in)` (the user must `approve` the router);
//! 2. for each hop: `approve(pool_i, amt)` on `token_in[i]`, then `pool_i.swap_to(dir_i, amt, min_i,
//!    recipient_i, deadline)`. Intermediate hops pay the router (`min_i = 1`) and must be **v9**
//!    pools, which return `amount_out`; that exact amount feeds the next hop, so the router keeps no
//!    dust. The last hop pays `recipient` with `min_out` and may be v8 or v9.
//! 3. returns the last hop's return word (`amount_out` for v9, `0` for v8).
//!
//! Every failure (failed pull / approve, zero intermediate output, bad hop count, a pool rejecting
//! the swap) is a VM fault, so the whole transaction - every hop - is reverted. A reentrancy lock
//! (slot `k[31] = 0x0b`) blocks re-entry during the token / pool calls. The router only ever pulls from
//! `CALLER`, so an approval to the router can't be spent by anyone else.
//!
//! Calldata (selector [`SELECTOR_CUSTODY_ROUTER_SWAP`]): `sel | hops | amount_in | min_out | recipient |
//! deadline | (pool, direction, token_in) x hops` = `192 + 96 * hops` bytes.

use boing_primitives::AccountId;

use crate::asm::{w_sel, w_u64, Asm};
use crate::bytecode::Opcode;
use crate::native_amm::SELECTOR_SWAP_TO;
use crate::reference_token::amount_word;
use crate::reference_token_v2::{SELECTOR_APPROVE, SELECTOR_TRANSFER_FROM};

/// Multihop exact-input swap through custody pools.
pub const SELECTOR_CUSTODY_ROUTER_SWAP: u8 = 0xF1;
/// Maximum hops per call.
pub const CUSTODY_ROUTER_MAX_HOPS: usize = 4;
/// CREATE2 salt for [`native_dex_custody_router_bytecode`].
pub const NATIVE_DEX_CUSTODY_ROUTER_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_NATIVEDEX_CROUTER_V1\x00\x00\x00\x00\x00\x00";

/// Reentrancy lock slot.
#[must_use]
pub fn custody_router_lock_key() -> [u8; 32] {
    w_u64(0x0b)
}

/// One hop: `pool`, `direction` (`0` = A->B, `1` = B->A) and the pool's input token for that direction.
#[derive(Clone, Copy, Debug)]
pub struct CustodyHop {
    pub pool: AccountId,
    pub direction: u8,
    pub token_in: AccountId,
}

#[must_use]
pub fn encode_custody_router_swap_calldata(
    hops: &[CustodyHop],
    amount_in: u128,
    min_out: u128,
    recipient: &AccountId,
    deadline: u64,
) -> Vec<u8> {
    assert!(
        (1..=CUSTODY_ROUTER_MAX_HOPS).contains(&hops.len()),
        "1..=4 hops"
    );
    let mut v = w_sel(SELECTOR_CUSTODY_ROUTER_SWAP).to_vec();
    v.extend_from_slice(&amount_word(hops.len() as u128));
    v.extend_from_slice(&amount_word(amount_in));
    v.extend_from_slice(&amount_word(min_out));
    v.extend_from_slice(&recipient.0);
    v.extend_from_slice(&amount_word(u128::from(deadline)));
    for h in hops {
        v.extend_from_slice(&h.pool.0);
        v.extend_from_slice(&amount_word(u128::from(h.direction)));
        v.extend_from_slice(&h.token_in.0);
    }
    v
}

// Calldata words.
const N: u64 = 32;
const AMOUNT_IN: u64 = 64;
const MIN_OUT: u64 = 96;
const RECIPIENT: u64 = 128;
const DEADLINE: u64 = 160;
const HOPS: u64 = 192;
// Scratch (calldata is at most 192 + 384 = 576 bytes).
const AMT: u64 = 1024;
const RECIP: u64 = 1056;
const MIN: u64 = 1088;
const CB: u64 = 1152;
const RET: u64 = 1408;

/// Custody router bytecode (purpose `dapp`).
#[must_use]
pub fn native_dex_custody_router_bytecode() -> Vec<u8> {
    let mut a = Asm::new();
    let fail = a.label();
    let done = a.label();

    a.sload_key(&custody_router_lock_key()).jumpi(fail);
    a.push_u(1).sstore_key(&custody_router_lock_key());
    a.mload(0)
        .push(&w_sel(SELECTOR_CUSTODY_ROUTER_SWAP))
        .op(Opcode::Eq)
        .op(Opcode::IsZero)
        .jumpi(fail);
    // 1 <= hops <= MAX; amount_in > 0; recipient != 0.
    a.mload(N).op(Opcode::IsZero).jumpi(fail);
    a.mload(N)
        .push_u(CUSTODY_ROUTER_MAX_HOPS as u64)
        .op(Opcode::Gt)
        .jumpi(fail);
    a.mload(AMOUNT_IN).op(Opcode::IsZero).jumpi(fail);
    a.mload(RECIPIENT).op(Opcode::IsZero).jumpi(fail);

    // Pull amount_in of token_in[0] from CALLER.
    a.push(&w_sel(SELECTOR_TRANSFER_FROM)).mstore(CB);
    a.op(Opcode::Caller).mstore(CB + 32);
    a.op(Opcode::Address).mstore(CB + 64);
    a.mcopy(AMOUNT_IN, CB + 96);
    a.push_u(0).mstore(RET);
    a.mload(HOPS + 64)
        .push_u(CB)
        .push_u(128)
        .push_u(RET)
        .push_u(32)
        .op(Opcode::Call);
    a.op(Opcode::IsZero).jumpi(fail);
    a.mload(RET)
        .push_u(1)
        .op(Opcode::Eq)
        .op(Opcode::IsZero)
        .jumpi(fail);
    a.mcopy(AMOUNT_IN, AMT);

    for i in 0..CUSTODY_ROUTER_MAX_HOPS as u64 {
        let h = HOPS + 96 * i;
        let (last, go) = (a.label(), a.label());
        if i > 0 {
            // hops <= i  ->  done
            a.mload(N).push_u(i + 1).op(Opcode::Lt).jumpi(done);
        }
        // approve(pool_i, AMT) on token_in_i, must return 1.
        a.push(&w_sel(SELECTOR_APPROVE)).mstore(CB);
        a.mcopy(h, CB + 32).mcopy(AMT, CB + 64);
        a.push_u(0).mstore(RET);
        a.mload(h + 64)
            .push_u(CB)
            .push_u(96)
            .push_u(RET)
            .push_u(32)
            .op(Opcode::Call);
        a.op(Opcode::IsZero).jumpi(fail);
        a.mload(RET)
            .push_u(1)
            .op(Opcode::Eq)
            .op(Opcode::IsZero)
            .jumpi(fail);
        // Last hop?  (hops == i + 1)
        a.mload(N).push_u(i + 1).op(Opcode::Eq).jumpi(last);
        a.op(Opcode::Address).mstore(RECIP);
        a.push_u(1).mstore(MIN);
        a.jump(go);
        a.mark(last);
        a.mcopy(RECIPIENT, RECIP).mcopy(MIN_OUT, MIN);
        a.mark(go);
        // pool_i.swap_to(dir, AMT, MIN, RECIP, deadline)
        a.push(&w_sel(SELECTOR_SWAP_TO)).mstore(CB);
        a.mcopy(h + 32, CB + 32)
            .mcopy(AMT, CB + 64)
            .mcopy(MIN, CB + 96)
            .mcopy(RECIP, CB + 128)
            .mcopy(DEADLINE, CB + 160);
        a.push_u(0).mstore(RET);
        a.mload(h)
            .push_u(CB)
            .push_u(192)
            .push_u(RET)
            .push_u(32)
            .op(Opcode::Call);
        a.op(Opcode::IsZero).jumpi(fail);
        // Intermediate hop: the (v9) pool's returned amount_out feeds the next hop; 0 -> fault.
        a.mload(N).push_u(i + 1).op(Opcode::Eq).jumpi(done);
        a.mcopy(RET, AMT);
        a.mload(AMT).op(Opcode::IsZero).jumpi(fail);
    }

    a.mark(done);
    a.push_u(0).sstore_key(&custody_router_lock_key());
    a.return_word(RET);
    a.mark(fail);
    a.fault();
    a.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::{Interpreter, VmExecutionContext};
    use crate::native_amm::{
        constant_product_amount_out_fee_on_input_with_bps, encode_set_tokens_calldata,
        reserve_a_key, reserve_b_key,
    };
    use crate::native_amm_v8::{
        constant_product_pool_bytecode_v8, constant_product_pool_bytecode_v9,
        encode_v8_add_liquidity_calldata, encode_v8_swap_calldata,
    };
    use crate::reference_token::{encode_mint_first_calldata, REF_FUNGIBLE_BALANCE_XOR};
    use crate::reference_token_v2::{
        encode_approve_calldata, ref_fungible_allowance_key,
        reference_fungible_template_v2_bytecode,
    };
    use boing_primitives::Account;
    use boing_qa::RuleRegistry;
    use boing_state::StateStore;

    const ALICE: AccountId = AccountId([0xa1; 32]);
    const BOB: AccountId = AccountId([0xb2; 32]);
    const ROUTER: AccountId = AccountId([0xe0; 32]);
    const TA: AccountId = AccountId([0x0a; 32]);
    const TB: AccountId = AccountId([0x0b; 32]);
    const TC: AccountId = AccountId([0x0c; 32]);
    const P_AB9: AccountId = AccountId([0x91; 32]);
    const P_BC9: AccountId = AccountId([0x92; 32]);
    const P_AB8: AccountId = AccountId([0x81; 32]);
    const P_BC8: AccountId = AccountId([0x82; 32]);

    struct Env {
        st: StateStore,
        height: u64,
    }

    fn tail(w: &[u8; 32]) -> u128 {
        u128::from_be_bytes(w[16..32].try_into().unwrap())
    }

    impl Env {
        /// Like a transaction: a fault reverts every nested change.
        fn call(&mut self, who: AccountId, to: AccountId, cd: &[u8]) -> Option<Vec<u8>> {
            let cp = self.st.checkpoint();
            let code = self.st.get_contract_code(&to).unwrap().clone();
            let mut it = Interpreter::new(code, 40_000_000);
            let ctx = VmExecutionContext {
                block_height: self.height,
                block_timestamp: 0,
                native_value: false,
                vm_hardening: false,
            };
            match it.run_with_qa(who, to, cd, &mut self.st, &RuleRegistry::new(), ctx) {
                Ok(_) => Some(it.return_data.clone().unwrap_or_default()),
                Err(_) => {
                    self.st.revert(cp);
                    None
                }
            }
        }
        fn ok(&mut self, who: AccountId, to: AccountId, cd: &[u8]) -> Vec<u8> {
            self.call(who, to, cd).expect("call should succeed")
        }
        fn bal(&self, t: AccountId, who: AccountId) -> u128 {
            let mut k = [0u8; 32];
            for i in 0..32 {
                k[i] = who.0[i] ^ REF_FUNGIBLE_BALANCE_XOR[i];
            }
            tail(&self.st.get_contract_storage(&t, &k))
        }
        fn reserves(&self, p: AccountId) -> (u128, u128) {
            (
                tail(&self.st.get_contract_storage(&p, &reserve_a_key())),
                tail(&self.st.get_contract_storage(&p, &reserve_b_key())),
            )
        }
    }

    fn env() -> Env {
        let mut st = StateStore::new();
        let tok = reference_fungible_template_v2_bytecode();
        let mut code = vec![(ROUTER, native_dex_custody_router_bytecode())];
        for t in [TA, TB, TC] {
            code.push((t, tok.clone()));
        }
        for p in [P_AB9, P_BC9] {
            code.push((p, constant_product_pool_bytecode_v9()));
        }
        for p in [P_AB8, P_BC8] {
            code.push((p, constant_product_pool_bytecode_v8()));
        }
        for id in [ALICE, BOB] {
            st.insert(Account {
                id,
                state: Default::default(),
            });
        }
        for (id, c) in code {
            st.insert(Account {
                id,
                state: Default::default(),
            });
            st.set_contract_code(id, c);
        }
        let mut e = Env { st, height: 100 };
        // Admin = ALICE mints; ALICE seeds every pool, then BOB gets trading balances.
        for t in [TA, TB, TC] {
            e.ok(ALICE, t, &encode_mint_first_calldata(&ALICE, 10_000_000));
        }
        for (p, x, y) in [
            (P_AB9, TA, TB),
            (P_BC9, TB, TC),
            (P_AB8, TA, TB),
            (P_BC8, TB, TC),
        ] {
            e.ok(ALICE, p, &encode_set_tokens_calldata(&x, &y));
            e.ok(ALICE, x, &encode_approve_calldata(&p, 1_000_000));
            e.ok(ALICE, y, &encode_approve_calldata(&p, 2_000_000));
            e.ok(
                ALICE,
                p,
                &encode_v8_add_liquidity_calldata(1_000_000, 2_000_000, 1, 0),
            );
        }
        for t in [TA, TB, TC] {
            e.ok(
                ALICE,
                t,
                &crate::reference_token::encode_transfer_calldata(&BOB, 100_000),
            );
        }
        e
    }

    fn q(ri: u128, ro: u128, ain: u128) -> u128 {
        u128::from(constant_product_amount_out_fee_on_input_with_bps(
            ri as u64, ro as u64, ain as u64, 30,
        ))
    }
    fn hop(pool: AccountId, direction: u8, token_in: AccountId) -> CustodyHop {
        CustodyHop {
            pool,
            direction,
            token_in,
        }
    }
    fn router_holds_nothing(e: &Env) {
        for t in [TA, TB, TC] {
            assert_eq!(e.bal(t, ROUTER), 0, "router keeps no tokens");
            for p in [P_AB9, P_BC9, P_AB8, P_BC8] {
                let w =
                    e.st.get_contract_storage(&t, &ref_fungible_allowance_key(&ROUTER, &p));
                assert_eq!(tail(&w), 0, "no leftover router allowance");
            }
        }
        assert_eq!(
            e.st.get_contract_storage(&ROUTER, &custody_router_lock_key()),
            [0u8; 32]
        );
    }

    #[test]
    fn v9_swap_returns_amount_out_and_v8_returns_nothing() {
        let mut e = env();
        let want = q(1_000_000, 2_000_000, 1_000);
        e.ok(BOB, TA, &encode_approve_calldata(&P_AB9, 1_000));
        let r = e.ok(BOB, P_AB9, &encode_v8_swap_calldata(0, 1_000, 1, 0));
        assert_eq!(r, amount_word(want).to_vec());
        e.ok(BOB, TA, &encode_approve_calldata(&P_AB8, 1_000));
        let r = e.ok(BOB, P_AB8, &encode_v8_swap_calldata(0, 1_000, 1, 0));
        assert!(r.is_empty());
    }

    #[test]
    fn single_hop_pulls_from_signer_works_with_v8_and_v9() {
        for pool in [P_AB8, P_AB9] {
            let mut e = env();
            let want = q(1_000_000, 2_000_000, 5_000);
            e.ok(BOB, TA, &encode_approve_calldata(&ROUTER, 5_000));
            let cd = encode_custody_router_swap_calldata(&[hop(pool, 0, TA)], 5_000, want, &BOB, 0);
            e.ok(BOB, ROUTER, &cd);
            assert_eq!(e.bal(TA, BOB), 95_000);
            assert_eq!(e.bal(TB, BOB), 100_000 + want);
            assert_eq!(e.reserves(pool), (1_005_000, 2_000_000 - want));
            assert_eq!((e.bal(TA, pool), e.bal(TB, pool)), e.reserves(pool));
            router_holds_nothing(&e);
        }
    }

    #[test]
    fn multihop_a_b_c_matches_chained_quotes_and_pays_recipient() {
        let mut e = env();
        let o1 = q(1_000_000, 2_000_000, 7_000);
        let o2 = q(1_000_000, 2_000_000, o1);
        e.ok(BOB, TA, &encode_approve_calldata(&ROUTER, 7_000));
        let path = [hop(P_AB9, 0, TA), hop(P_BC9, 0, TB)];
        let r = e.ok(
            BOB,
            ROUTER,
            &encode_custody_router_swap_calldata(&path, 7_000, o2, &ALICE, 0),
        );
        assert_eq!(r, amount_word(o2).to_vec());
        assert_eq!(e.bal(TA, BOB), 93_000);
        assert_eq!(e.bal(TC, ALICE), 10_000_000 - 2_000_000 * 2 - 100_000 + o2);
        assert_eq!(e.reserves(P_AB9), (1_007_000, 2_000_000 - o1));
        assert_eq!(e.reserves(P_BC9), (1_000_000 + o1, 2_000_000 - o2));
        router_holds_nothing(&e);
        // Reverse direction C -> B -> A, last hop on a v8 pool is fine.
        e.ok(BOB, TC, &encode_approve_calldata(&ROUTER, 3_000));
        let back = [hop(P_BC9, 1, TC), hop(P_AB8, 1, TB)];
        e.ok(
            BOB,
            ROUTER,
            &encode_custody_router_swap_calldata(&back, 3_000, 1, &BOB, 0),
        );
        assert!(e.bal(TA, BOB) > 93_000);
        router_holds_nothing(&e);
    }

    #[test]
    fn four_hops_round_trip_and_five_hops_rejected() {
        let mut e = env();
        e.ok(BOB, TA, &encode_approve_calldata(&ROUTER, 2_000));
        let path = [
            hop(P_AB9, 0, TA),
            hop(P_BC9, 0, TB),
            hop(P_BC9, 1, TC),
            hop(P_AB9, 1, TB),
        ];
        e.ok(
            BOB,
            ROUTER,
            &encode_custody_router_swap_calldata(&path, 2_000, 1, &BOB, 0),
        );
        router_holds_nothing(&e);
        let mut cd = encode_custody_router_swap_calldata(&path, 10, 1, &BOB, 0);
        cd[63] = 5;
        cd.extend_from_slice(&[0u8; 96]);
        assert!(e.call(BOB, ROUTER, &cd).is_none());
    }

    /// Any failure in any hop reverts the pull and every earlier hop.
    #[test]
    fn failures_roll_back_everything() {
        let mut e = env();
        let path = [hop(P_AB9, 0, TA), hop(P_BC9, 0, TB)];
        let snapshot = |e: &Env| {
            (
                e.bal(TA, BOB),
                e.bal(TC, BOB),
                e.reserves(P_AB9),
                e.reserves(P_BC9),
                e.bal(TA, ROUTER),
                e.bal(TB, ROUTER),
            )
        };
        let before = snapshot(&e);
        // No approval to the router.
        assert!(e
            .call(
                BOB,
                ROUTER,
                &encode_custody_router_swap_calldata(&path, 7_000, 1, &BOB, 0)
            )
            .is_none());
        assert_eq!(snapshot(&e), before);
        e.ok(BOB, TA, &encode_approve_calldata(&ROUTER, 7_000));
        let before = snapshot(&e);
        // Final slippage fails on hop 2 -> hop 1 and the pull are undone too.
        let o2 = q(1_000_000, 2_000_000, q(1_000_000, 2_000_000, 7_000));
        assert!(e
            .call(
                BOB,
                ROUTER,
                &encode_custody_router_swap_calldata(&path, 7_000, o2 + 1, &BOB, 0)
            )
            .is_none());
        assert_eq!(snapshot(&e), before);
        // Intermediate hop on a v8 pool returns no amount -> fault.
        let bad = [hop(P_AB8, 0, TA), hop(P_BC9, 0, TB)];
        assert!(e
            .call(
                BOB,
                ROUTER,
                &encode_custody_router_swap_calldata(&bad, 7_000, 1, &BOB, 0)
            )
            .is_none());
        assert_eq!(snapshot(&e), before);
        // Wrong token_in for hop 2 (router has none of it).
        let wrong = [hop(P_AB9, 0, TA), hop(P_BC9, 0, TC)];
        assert!(e
            .call(
                BOB,
                ROUTER,
                &encode_custody_router_swap_calldata(&wrong, 7_000, 1, &BOB, 0)
            )
            .is_none());
        assert_eq!(snapshot(&e), before);
        // Deadline passed.
        assert!(e
            .call(
                BOB,
                ROUTER,
                &encode_custody_router_swap_calldata(&path, 7_000, 1, &BOB, 99)
            )
            .is_none());
        assert_eq!(snapshot(&e), before);
        // Still works afterwards (lock released by the revert).
        e.ok(
            BOB,
            ROUTER,
            &encode_custody_router_swap_calldata(&path, 7_000, o2, &BOB, 100),
        );
        router_holds_nothing(&e);
    }

    #[test]
    fn approval_to_router_cannot_be_spent_by_someone_else() {
        let mut e = env();
        e.ok(BOB, TA, &encode_approve_calldata(&ROUTER, 5_000));
        let a_before = e.bal(TA, ALICE);
        // ALICE calls the router: it pulls from ALICE (caller) only, never from BOB.
        let cd = encode_custody_router_swap_calldata(&[hop(P_AB9, 0, TA)], 5_000, 1, &ALICE, 0);
        assert!(
            e.call(ALICE, ROUTER, &cd).is_none(),
            "alice gave the router no allowance"
        );
        assert_eq!(e.bal(TA, BOB), 100_000);
        assert_eq!(e.bal(TA, ALICE), a_before);
    }

    #[test]
    fn router_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult};
        for code in [
            native_dex_custody_router_bytecode(),
            constant_product_pool_bytecode_v9(),
        ] {
            let r = check_contract_deploy_full(&code, Some("dapp"), None, &RuleRegistry::new());
            assert!(matches!(r, QaResult::Allow), "{r:?}");
        }
    }
}
