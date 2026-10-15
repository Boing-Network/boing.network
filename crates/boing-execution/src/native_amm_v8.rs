//! **Native constant-product pool v8** -- custody-safe successor of v7 (new, versioned bytecode;
//! v1..v7 are unchanged). Requires **reference fungible v2** tokens (`transfer_from`).
//!
//! Differences from v7:
//! - `add_liquidity` pulls `amount_a` / `amount_b` and `swap` pulls `amount_in` with
//!   `transfer_from(CALLER, pool, amount)` **before** any reserve write. A pull that does not
//!   return word `1` faults the transaction, so the block executor reverts everything (including
//!   an earlier successful pull in the same call).
//! - Tokens must be configured (`set_tokens`, both non-zero, distinct, once) before liquidity.
//! - `min_liquidity` is enforced on-chain; every liquidity/swap selector takes an optional
//!   **deadline block height** word (`0` = none; `BlockHeight > deadline` faults).
//! - Every rejection is a VM **fault** (failed receipt, state reverted) instead of a silent `STOP`.
//! - Reentrancy lock ([`pool_v8_lock_key`]): any call while the lock is set faults.
//! - Reserves stay below `2^64` (u64 ledger convention shared with the SDK quotes).
//!
//! Storage keys, LP balance slots, fee semantics (fee-on-input, default 30 bps, fee admin) and log
//! topics are the same as v7, so existing readers / indexers work unchanged.
//!
//! | Selector | Words after selector |
//! |---|---|
//! | `0x10` swap | `direction`, `amount_in`, `min_out`, `[deadline]` |
//! | `0x15` swap_to | `direction`, `amount_in`, `min_out`, `recipient`, `[deadline]` |
//! | `0x11` add_liquidity | `amount_a`, `amount_b`, `min_liquidity`, `[deadline]` (returns LP minted) |
//! | `0x12` remove_liquidity | `liquidity`, `min_a`, `min_b`, `[deadline]` |
//! | `0x16` remove_liquidity_to | `liquidity`, `min_a`, `min_b`, `recipient_a`, `recipient_b`, `[deadline]` |
//! | `0x13` set_tokens / `0x14` set_swap_fee_bps / `0x17` set_fee_admin | as v7 (rejections fault) |

use boing_primitives::AccountId;

use crate::asm::{w_sel, w_two_pow_64, w_u64, Asm, Label};
use crate::bytecode::Opcode;
use crate::native_amm::{
    fee_admin_key, reserve_a_key, reserve_b_key, swap_fee_bps_key, token_a_key, token_b_key,
    tokens_configured_key, total_lp_supply_key, LP_BALANCE_STORAGE_XOR,
    NATIVE_AMM_TOPIC_ADD_LIQUIDITY, NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY, NATIVE_AMM_TOPIC_SWAP,
    NATIVE_CP_SWAP_FEE_BPS, SELECTOR_ADD_LIQUIDITY, SELECTOR_REMOVE_LIQUIDITY,
    SELECTOR_REMOVE_LIQUIDITY_TO, SELECTOR_SET_FEE_ADMIN, SELECTOR_SET_SWAP_FEE_BPS,
    SELECTOR_SET_TOKENS, SELECTOR_SWAP, SELECTOR_SWAP_TO,
};
use crate::reference_token::{amount_word, SELECTOR_TRANSFER};
use crate::reference_token_v2::SELECTOR_TRANSFER_FROM;

/// CREATE2 salt for **v8** pool bytecode ([`constant_product_pool_bytecode_v8`]).
pub const NATIVE_CP_POOL_CREATE2_SALT_V8: [u8; 32] =
    *b"BOING_NATIVECP_C2V8\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Reentrancy lock slot (`k[31] = 0x09`); non-zero only during a call.
#[must_use]
pub fn pool_v8_lock_key() -> [u8; 32] {
    w_u64(0x09)
}

pub fn encode_v8_swap_calldata(
    direction: u128,
    amount_in: u128,
    min_out: u128,
    deadline: u64,
) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_SWAP).to_vec();
    for w in [direction, amount_in, min_out, u128::from(deadline)] {
        v.extend_from_slice(&amount_word(w));
    }
    v
}

pub fn encode_v8_swap_to_calldata(
    direction: u128,
    amount_in: u128,
    min_out: u128,
    recipient: &AccountId,
    deadline: u64,
) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_SWAP_TO).to_vec();
    for w in [direction, amount_in, min_out] {
        v.extend_from_slice(&amount_word(w));
    }
    v.extend_from_slice(&recipient.0);
    v.extend_from_slice(&amount_word(u128::from(deadline)));
    v
}

pub fn encode_v8_add_liquidity_calldata(
    amount_a: u128,
    amount_b: u128,
    min_liquidity: u128,
    deadline: u64,
) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_ADD_LIQUIDITY).to_vec();
    for w in [amount_a, amount_b, min_liquidity, u128::from(deadline)] {
        v.extend_from_slice(&amount_word(w));
    }
    v
}

pub fn encode_v8_remove_liquidity_calldata(
    liquidity: u128,
    min_a: u128,
    min_b: u128,
    deadline: u64,
) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_REMOVE_LIQUIDITY).to_vec();
    for w in [liquidity, min_a, min_b, u128::from(deadline)] {
        v.extend_from_slice(&amount_word(w));
    }
    v
}

pub fn encode_v8_remove_liquidity_to_calldata(
    liquidity: u128,
    min_a: u128,
    min_b: u128,
    recipient_a: &AccountId,
    recipient_b: &AccountId,
    deadline: u64,
) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_REMOVE_LIQUIDITY_TO).to_vec();
    for w in [liquidity, min_a, min_b] {
        v.extend_from_slice(&amount_word(w));
    }
    v.extend_from_slice(&recipient_a.0);
    v.extend_from_slice(&recipient_b.0);
    v.extend_from_slice(&amount_word(u128::from(deadline)));
    v
}

// Scratch memory (calldata occupies < 256 bytes).
const DL: u64 = 1024;
const A: u64 = 1056;
const B: u64 = 1088;
const MIN: u64 = 1120;
const T: u64 = 1152;
const RA: u64 = 1184;
const RB: u64 = 1216;
const LP: u64 = 1248;
const LP1: u64 = 1280;
const LP2: u64 = 1312;
const FEE: u64 = 1344;
const DIR: u64 = 1376;
const AIN: u64 = 1408;
const AINF: u64 = 1440;
const OUT: u64 = 1472;
const RI: u64 = 1504;
const RO: u64 = 1536;
const TIN: u64 = 1568;
const TOUT: u64 = 1600;
const RECIP: u64 = 1632;
const BURN: u64 = 1664;
const KEY: u64 = 1696;
const ULP: u64 = 1728;
const OA: u64 = 1760;
const OB: u64 = 1792;
const RECA: u64 = 1824;
const RECB: u64 = 1856;
const TA: u64 = 1888;
const TB: u64 = 1920;
const MINA: u64 = 1952;
const MINB: u64 = 1984;
const CB: u64 = 2048;
const RET: u64 = 2304;
const LOGB: u64 = 2400;

struct P {
    a: Asm,
    fail: Label,
}

impl P {
    /// Fault unless `mem[x] != 0`.
    fn nonzero(&mut self, x: u64) {
        let f = self.fail;
        self.a.mload(x).op(Opcode::IsZero).jumpi(f);
    }
    /// Fault if `mem[x] < mem[y]`.
    fn ge(&mut self, x: u64, y: u64) {
        let f = self.fail;
        self.a.mload(x).mload(y).op(Opcode::Lt).jumpi(f);
    }
    /// Fault unless `mem[x] < 2^64`.
    fn below_2_64(&mut self, x: u64) {
        let f = self.fail;
        self.a
            .mload(x)
            .push(&w_two_pow_64())
            .op(Opcode::Lt)
            .op(Opcode::IsZero)
            .jumpi(f);
    }
    fn deadline(&mut self) {
        let ok = self.a.label();
        let f = self.fail;
        self.a.mload(DL).op(Opcode::IsZero).jumpi(ok);
        self.a
            .op(Opcode::BlockHeight)
            .mload(DL)
            .op(Opcode::Gt)
            .jumpi(f);
        self.a.mark(ok);
    }
    fn require_tokens(&mut self) {
        let f = self.fail;
        self.a
            .sload_key(&tokens_configured_key())
            .op(Opcode::IsZero)
            .jumpi(f);
    }
    /// `mem[dst] = min(mem[x], mem[y])`.
    fn min(&mut self, x: u64, y: u64, dst: u64) {
        let lx = self.a.label();
        let done = self.a.label();
        self.a.mload(x).mload(y).op(Opcode::Lt).jumpi(lx);
        self.a.mcopy(y, dst).jump(done);
        self.a.mark(lx);
        self.a.mcopy(x, dst);
        self.a.mark(done);
    }
    /// Pull `mem[amount]` of token `mem[token]` from `CALLER` into the pool; fault unless it returns `1`.
    fn pull(&mut self, token: u64, amount: u64) {
        let f = self.fail;
        let a = &mut self.a;
        a.push(&w_sel(SELECTOR_TRANSFER_FROM)).mstore(CB);
        a.op(Opcode::Caller).mstore(CB + 32);
        a.op(Opcode::Address).mstore(CB + 64);
        a.mcopy(amount, CB + 96);
        a.push_u(0).mstore(RET);
        a.mload(token)
            .push_u(CB)
            .push_u(128)
            .push_u(RET)
            .push_u(32)
            .op(Opcode::Call);
        a.op(Opcode::IsZero).jumpi(f);
        a.mload(RET)
            .push_u(1)
            .op(Opcode::Eq)
            .op(Opcode::IsZero)
            .jumpi(f);
    }
    /// Pay `mem[amount]` of token `mem[token]` to `mem[to]` via reference `transfer` (skipped if 0).
    fn pay(&mut self, token: u64, to: u64, amount: u64) {
        let f = self.fail;
        let skip = self.a.label();
        let a = &mut self.a;
        a.mload(amount).op(Opcode::IsZero).jumpi(skip);
        a.push(&w_sel(SELECTOR_TRANSFER)).mstore(CB);
        a.mcopy(to, CB + 32).mcopy(amount, CB + 64);
        a.mload(token)
            .push_u(CB)
            .push_u(96)
            .push_u(RET)
            .push_u(0)
            .op(Opcode::Call);
        a.op(Opcode::IsZero).jumpi(f);
        a.mark(skip);
    }
    fn log3(&mut self, topic: &[u8; 32], x: u64, y: u64, z: u64) {
        let a = &mut self.a;
        a.mcopy(x, LOGB).mcopy(y, LOGB + 32).mcopy(z, LOGB + 64);
        a.push(topic)
            .op(Opcode::Caller)
            .push_u(96)
            .push_u(LOGB)
            .op(Opcode::Log2);
    }
    fn unlock(&mut self) {
        self.a.push_u(0).sstore_key(&pool_v8_lock_key());
    }
    fn unlock_stop(&mut self) {
        self.unlock();
        self.a.op(Opcode::Stop);
    }
    /// `FEE` = stored fee (0 -> default); fault if > 10_000.
    fn load_fee(&mut self) {
        let set = self.a.label();
        let f = self.fail;
        self.a.sload_key(&swap_fee_bps_key()).mstore(FEE);
        self.a
            .mload(FEE)
            .op(Opcode::IsZero)
            .op(Opcode::IsZero)
            .jumpi(set);
        self.a.push_u(u64::from(NATIVE_CP_SWAP_FEE_BPS)).mstore(FEE);
        self.a.mark(set);
        self.a.mload(FEE).push_u(10_000).op(Opcode::Gt).jumpi(f);
    }
}

/// Custody-safe native CP pool (**v8**). CREATE2 salt: [`NATIVE_CP_POOL_CREATE2_SALT_V8`].
#[must_use]
pub fn constant_product_pool_bytecode_v8() -> Vec<u8> {
    pool_v8_family(false)
}

/// CREATE2 salt for **v9** pool bytecode ([`constant_product_pool_bytecode_v9`]).
pub const NATIVE_CP_POOL_CREATE2_SALT_V9: [u8; 32] =
    *b"BOING_NATIVECP_C2V9\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// **v9** = v8 plus `swap` / `swap_to` **return `amount_out`** as one 32-byte word, so a router can
/// chain hops ([`crate::native_dex_custody_router`]). Everything else is byte-for-byte v8 logic.
#[must_use]
pub fn constant_product_pool_bytecode_v9() -> Vec<u8> {
    pool_v8_family(true)
}

fn pool_v8_family(swap_returns_out: bool) -> Vec<u8> {
    let mut a = Asm::new();
    let fail = a.label();
    let mut p = P { a, fail };
    let (swap, swap_to, swap_common) = (p.a.label(), p.a.label(), p.a.label());
    let add = p.a.label();
    let (rm, rm_to, rm_common) = (p.a.label(), p.a.label(), p.a.label());
    let (set_tokens, set_fee, set_admin) = (p.a.label(), p.a.label(), p.a.label());

    // Reentrancy lock.
    p.a.sload_key(&pool_v8_lock_key()).jumpi(fail);
    p.a.push_u(1).sstore_key(&pool_v8_lock_key());
    for (s, l) in [
        (SELECTOR_SWAP, swap),
        (SELECTOR_SWAP_TO, swap_to),
        (SELECTOR_ADD_LIQUIDITY, add),
        (SELECTOR_REMOVE_LIQUIDITY, rm),
        (SELECTOR_REMOVE_LIQUIDITY_TO, rm_to),
        (SELECTOR_SET_TOKENS, set_tokens),
        (SELECTOR_SET_SWAP_FEE_BPS, set_fee),
        (SELECTOR_SET_FEE_ADMIN, set_admin),
    ] {
        p.a.mload(0).push(&w_sel(s)).op(Opcode::Eq).jumpi(l);
    }
    p.a.jump(fail);

    // --- set_tokens(a, b): once, before liquidity, both non-zero and distinct.
    p.a.mark(set_tokens);
    p.a.sload_key(&tokens_configured_key()).jumpi(fail);
    p.a.mcopy(32, TA).mcopy(64, TB);
    p.nonzero(TA);
    p.nonzero(TB);
    p.a.mload(TA).mload(TB).op(Opcode::Eq).jumpi(fail);
    p.a.mload(TA).sstore_key(&token_a_key());
    p.a.mload(TB).sstore_key(&token_b_key());
    p.a.push_u(1).sstore_key(&tokens_configured_key());
    p.unlock_stop();

    // --- set_swap_fee_bps(fee): LP == 0 or caller == fee admin; 1..=10_000.
    p.a.mark(set_fee);
    let auth = p.a.label();
    p.a.mcopy(32, FEE);
    p.a.sload_key(&total_lp_supply_key())
        .op(Opcode::IsZero)
        .jumpi(auth);
    p.a.op(Opcode::Caller)
        .sload_key(&fee_admin_key())
        .op(Opcode::Eq)
        .jumpi(auth);
    p.a.jump(fail);
    p.a.mark(auth);
    p.nonzero(FEE);
    p.a.mload(FEE).push_u(10_000).op(Opcode::Gt).jumpi(fail);
    p.a.mload(FEE).sstore_key(&swap_fee_bps_key());
    p.unlock_stop();

    // --- set_fee_admin(admin): LP == 0.
    p.a.mark(set_admin);
    p.a.sload_key(&total_lp_supply_key()).jumpi(fail);
    p.a.mload(32).sstore_key(&fee_admin_key());
    p.unlock_stop();

    // --- add_liquidity(a, b, min_liquidity, [deadline]).
    p.a.mark(add);
    let (first, lp_done) = (p.a.label(), p.a.label());
    p.a.mcopy(128, DL);
    p.deadline();
    p.require_tokens();
    p.a.mcopy(32, A).mcopy(64, B).mcopy(96, MIN);
    p.nonzero(A);
    p.nonzero(B);
    p.below_2_64(A);
    p.below_2_64(B);
    p.a.sload_key(&total_lp_supply_key()).mstore(T);
    p.a.sload_key(&reserve_a_key()).mstore(RA);
    p.a.sload_key(&reserve_b_key()).mstore(RB);
    p.a.mload(RA).mload(A).op(Opcode::Add).mstore(LP1);
    p.below_2_64(LP1);
    p.a.mload(RB).mload(B).op(Opcode::Add).mstore(LP1);
    p.below_2_64(LP1);
    p.a.mload(T).op(Opcode::IsZero).jumpi(first);
    p.nonzero(RA);
    p.nonzero(RB);
    p.a.mload(A)
        .mload(T)
        .op(Opcode::Mul)
        .mload(RA)
        .op(Opcode::Div)
        .mstore(LP1);
    p.a.mload(B)
        .mload(T)
        .op(Opcode::Mul)
        .mload(RB)
        .op(Opcode::Div)
        .mstore(LP2);
    p.min(LP1, LP2, LP);
    p.a.jump(lp_done);
    p.a.mark(first);
    p.min(A, B, LP);
    // First mint: default fee if unset (v7 behaviour).
    let fee_set = p.a.label();
    p.a.sload_key(&swap_fee_bps_key()).jumpi(fee_set);
    p.a.push_u(u64::from(NATIVE_CP_SWAP_FEE_BPS))
        .sstore_key(&swap_fee_bps_key());
    p.a.mark(fee_set);
    p.a.mark(lp_done);
    p.nonzero(LP);
    p.ge(LP, MIN);
    // Pull both sides before any reserve / LP write.
    p.a.sload_key(&token_a_key()).mstore(TA);
    p.a.sload_key(&token_b_key()).mstore(TB);
    p.pull(TA, A);
    p.pull(TB, B);
    p.a.mload(RA)
        .mload(A)
        .op(Opcode::Add)
        .sstore_key(&reserve_a_key());
    p.a.mload(RB)
        .mload(B)
        .op(Opcode::Add)
        .sstore_key(&reserve_b_key());
    p.a.mload(T)
        .mload(LP)
        .op(Opcode::Add)
        .sstore_key(&total_lp_supply_key());
    p.a.op(Opcode::Caller)
        .push(&LP_BALANCE_STORAGE_XOR)
        .op(Opcode::Xor)
        .mstore(KEY);
    p.a.mload(KEY)
        .op(Opcode::SLoad)
        .mload(LP)
        .op(Opcode::Add)
        .mload(KEY)
        .op(Opcode::SStore);
    p.log3(&NATIVE_AMM_TOPIC_ADD_LIQUIDITY, A, B, LP);
    p.unlock();
    p.a.return_word(LP);

    // --- remove_liquidity / remove_liquidity_to.
    p.a.mark(rm);
    p.a.op(Opcode::Caller).mstore(RECA);
    p.a.op(Opcode::Caller).mstore(RECB);
    p.a.mcopy(128, DL).jump(rm_common);
    p.a.mark(rm_to);
    p.a.mcopy(128, RECA).mcopy(160, RECB).mcopy(192, DL);
    p.nonzero(RECA);
    p.nonzero(RECB);
    p.a.mark(rm_common);
    p.deadline();
    p.a.mcopy(32, BURN).mcopy(64, MINA).mcopy(96, MINB);
    p.nonzero(BURN);
    p.a.op(Opcode::Caller)
        .push(&LP_BALANCE_STORAGE_XOR)
        .op(Opcode::Xor)
        .mstore(KEY);
    p.a.mload(KEY).op(Opcode::SLoad).mstore(ULP);
    p.ge(ULP, BURN);
    p.a.sload_key(&total_lp_supply_key()).mstore(T);
    p.a.sload_key(&reserve_a_key()).mstore(RA);
    p.a.sload_key(&reserve_b_key()).mstore(RB);
    p.a.mload(BURN)
        .mload(RA)
        .op(Opcode::Mul)
        .mload(T)
        .op(Opcode::Div)
        .mstore(OA);
    p.a.mload(BURN)
        .mload(RB)
        .op(Opcode::Mul)
        .mload(T)
        .op(Opcode::Div)
        .mstore(OB);
    p.ge(OA, MINA);
    p.ge(OB, MINB);
    p.a.mload(OA)
        .mload(OB)
        .op(Opcode::Or)
        .op(Opcode::IsZero)
        .jumpi(fail);
    p.a.mload(ULP)
        .mload(BURN)
        .op(Opcode::Sub)
        .mload(KEY)
        .op(Opcode::SStore);
    p.a.mload(T)
        .mload(BURN)
        .op(Opcode::Sub)
        .sstore_key(&total_lp_supply_key());
    p.a.mload(RA)
        .mload(OA)
        .op(Opcode::Sub)
        .sstore_key(&reserve_a_key());
    p.a.mload(RB)
        .mload(OB)
        .op(Opcode::Sub)
        .sstore_key(&reserve_b_key());
    p.a.sload_key(&token_a_key()).mstore(TA);
    p.a.sload_key(&token_b_key()).mstore(TB);
    p.pay(TA, RECA, OA);
    p.pay(TB, RECB, OB);
    p.log3(&NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY, BURN, OA, OB);
    p.unlock_stop();

    // --- swap / swap_to.
    p.a.mark(swap);
    p.a.op(Opcode::Caller).mstore(RECIP);
    p.a.mcopy(128, DL).jump(swap_common);
    p.a.mark(swap_to);
    p.a.mcopy(128, RECIP).mcopy(160, DL);
    p.nonzero(RECIP);
    p.a.mark(swap_common);
    let (a2b, b2a, priced) = (p.a.label(), p.a.label(), p.a.label());
    let (store_b2a, stored) = (p.a.label(), p.a.label());
    p.deadline();
    p.require_tokens();
    p.a.mcopy(32, DIR).mcopy(64, AIN).mcopy(96, MIN);
    p.a.mload(DIR).op(Opcode::IsZero).jumpi(a2b);
    p.a.mload(DIR).push_u(1).op(Opcode::Eq).jumpi(b2a);
    p.a.jump(fail);
    p.a.mark(a2b);
    p.a.sload_key(&reserve_a_key()).mstore(RI);
    p.a.sload_key(&reserve_b_key()).mstore(RO);
    p.a.sload_key(&token_a_key()).mstore(TIN);
    p.a.sload_key(&token_b_key()).mstore(TOUT);
    p.a.jump(priced);
    p.a.mark(b2a);
    p.a.sload_key(&reserve_b_key()).mstore(RI);
    p.a.sload_key(&reserve_a_key()).mstore(RO);
    p.a.sload_key(&token_b_key()).mstore(TIN);
    p.a.sload_key(&token_a_key()).mstore(TOUT);
    p.a.mark(priced);
    p.nonzero(AIN);
    p.nonzero(RI);
    p.nonzero(RO);
    p.below_2_64(AIN);
    p.a.mload(RI).mload(AIN).op(Opcode::Add).mstore(LP1);
    p.below_2_64(LP1);
    p.load_fee();
    p.a.mload(AIN)
        .push_u(10_000)
        .mload(FEE)
        .op(Opcode::Sub)
        .op(Opcode::Mul)
        .push_u(10_000)
        .op(Opcode::Div)
        .mstore(AINF);
    p.nonzero(AINF);
    p.a.mload(RO).mload(AINF).op(Opcode::Mul);
    p.a.mload(RI).mload(AINF).op(Opcode::Add);
    p.a.op(Opcode::Div).mstore(OUT);
    p.nonzero(OUT);
    p.ge(OUT, MIN);
    p.pull(TIN, AIN);
    p.a.mload(DIR).jumpi(store_b2a);
    p.a.mload(RI)
        .mload(AIN)
        .op(Opcode::Add)
        .sstore_key(&reserve_a_key());
    p.a.mload(RO)
        .mload(OUT)
        .op(Opcode::Sub)
        .sstore_key(&reserve_b_key());
    p.a.jump(stored);
    p.a.mark(store_b2a);
    p.a.mload(RI)
        .mload(AIN)
        .op(Opcode::Add)
        .sstore_key(&reserve_b_key());
    p.a.mload(RO)
        .mload(OUT)
        .op(Opcode::Sub)
        .sstore_key(&reserve_a_key());
    p.a.mark(stored);
    p.pay(TOUT, RECIP, OUT);
    p.log3(&NATIVE_AMM_TOPIC_SWAP, DIR, AIN, OUT);
    if swap_returns_out {
        p.unlock();
        p.a.return_word(OUT);
    } else {
        p.unlock_stop();
    }

    p.a.mark(fail);
    p.a.fault();
    p.a.finish()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::{Interpreter, VmExecutionContext};
    use crate::native_amm::{
        constant_product_amount_out_fee_on_input_with_bps, encode_set_fee_admin_calldata,
        encode_set_swap_fee_bps_calldata, encode_set_tokens_calldata, lp_balance_storage_key,
    };
    use crate::reference_token::{encode_mint_first_calldata, REF_FUNGIBLE_BALANCE_XOR};
    use crate::reference_token_v2::{
        encode_approve_calldata, reference_fungible_template_v2_bytecode,
    };
    use boing_primitives::Account;
    use boing_qa::RuleRegistry;
    use boing_state::StateStore;

    const LP1: AccountId = AccountId([0x01; 32]);
    const LP2: AccountId = AccountId([0x02; 32]);
    const TRADER: AccountId = AccountId([0x03; 32]);
    const ATTACKER: AccountId = AccountId([0x04; 32]);
    const ADMIN: AccountId = AccountId([0x05; 32]);
    const TA_ID: AccountId = AccountId([0xa0; 32]);
    const TB_ID: AccountId = AccountId([0xb0; 32]);
    const POOL: AccountId = AccountId([0xcc; 32]);

    fn u(w: [u8; 32]) -> u128 {
        assert!(w[..16].iter().all(|b| *b == 0));
        u128::from_be_bytes(w[16..].try_into().unwrap())
    }

    struct Env {
        st: StateStore,
        height: u64,
    }

    /// Outcome of one top-level call, executed like a transaction: a VM error reverts all state.
    struct Out {
        ok: bool,
        logs: usize,
        ret: Option<Vec<u8>>,
    }

    impl Env {
        fn deploy(&mut self, id: AccountId, code: Vec<u8>) {
            self.st.insert(Account {
                id,
                state: Default::default(),
            });
            self.st.set_contract_code(id, code);
        }
        fn call(&mut self, who: AccountId, to: AccountId, cd: Vec<u8>) -> Out {
            let cp = self.st.checkpoint();
            let code = self.st.get_contract_code(&to).unwrap().clone();
            let mut it = Interpreter::new(code, 10_000_000);
            let ctx = VmExecutionContext {
                block_height: self.height,
                block_timestamp: 0,
            };
            match it.run_with_qa(who, to, &cd, &mut self.st, &RuleRegistry::new(), ctx) {
                Ok(_) => Out {
                    ok: true,
                    logs: it.logs.len(),
                    ret: it.return_data.clone(),
                },
                Err(_) => {
                    self.st.revert(cp);
                    Out {
                        ok: false,
                        logs: 0,
                        ret: None,
                    }
                }
            }
        }
        fn pool(&mut self, who: AccountId, cd: Vec<u8>) -> Out {
            self.call(who, POOL, cd)
        }
        fn bal(&self, tok: AccountId, who: AccountId) -> u128 {
            let mut k = [0u8; 32];
            for i in 0..32 {
                k[i] = who.0[i] ^ REF_FUNGIBLE_BALANCE_XOR[i];
            }
            u(self.st.get_contract_storage(&tok, &k))
        }
        fn w(&self, k: [u8; 32]) -> u128 {
            u(self.st.get_contract_storage(&POOL, &k))
        }
        fn reserves(&self) -> (u128, u128, u128) {
            (
                self.w(reserve_a_key()),
                self.w(reserve_b_key()),
                self.w(total_lp_supply_key()),
            )
        }
        fn lp(&self, who: AccountId) -> u128 {
            self.w(lp_balance_storage_key(&who.0))
        }
        fn approve(&mut self, who: AccountId, tok: AccountId, amt: u128) {
            assert!(self.call(who, tok, encode_approve_calldata(&POOL, amt)).ok);
        }
        /// Custody invariant: ledger reserves equal the pool's real token balances.
        fn assert_custody(&self) {
            let (ra, rb, _) = self.reserves();
            assert_eq!(ra, self.bal(TA_ID, POOL), "reserve A != pool balance A");
            assert_eq!(rb, self.bal(TB_ID, POOL), "reserve B != pool balance B");
        }
        fn snapshot(&self) -> Vec<u128> {
            let (ra, rb, t) = self.reserves();
            let mut v = vec![ra, rb, t];
            for who in [LP1, LP2, TRADER, ATTACKER, POOL] {
                v.push(self.bal(TA_ID, who));
                v.push(self.bal(TB_ID, who));
                v.push(self.lp(who));
            }
            v.push(self.w(pool_v8_lock_key()));
            v
        }
    }

    /// Two v2 tokens (admin mints 1e9 each, then funds LP1/LP2/TRADER) + v8 pool with tokens set.
    fn setup() -> Env {
        let mut e = Env {
            st: StateStore::new(),
            height: 100,
        };
        for t in [TA_ID, TB_ID] {
            e.deploy(t, reference_fungible_template_v2_bytecode());
            assert!(
                e.call(ADMIN, t, encode_mint_first_calldata(&ADMIN, 1_000_000_000))
                    .ok
            );
            for who in [LP1, LP2, TRADER] {
                assert!(
                    e.call(
                        ADMIN,
                        t,
                        crate::reference_token::encode_transfer_calldata(&who, 1_000_000)
                    )
                    .ok
                );
            }
        }
        e.deploy(POOL, constant_product_pool_bytecode_v8());
        assert!(e.pool(ADMIN, encode_set_tokens_calldata(&TA_ID, &TB_ID)).ok);
        e
    }

    fn seed(e: &mut Env) {
        e.approve(LP1, TA_ID, 100_000);
        e.approve(LP1, TB_ID, 400_000);
        let r = e.pool(
            LP1,
            encode_v8_add_liquidity_calldata(100_000, 400_000, 100_000, 0),
        );
        assert!(r.ok);
        assert_eq!(u(r.ret.unwrap().try_into().unwrap()), 100_000);
    }

    #[test]
    fn add_liquidity_pulls_tokens_and_mints_lp() {
        let mut e = setup();
        seed(&mut e);
        assert_eq!(e.reserves(), (100_000, 400_000, 100_000));
        assert_eq!((e.bal(TA_ID, LP1), e.bal(TB_ID, LP1)), (900_000, 600_000));
        assert_eq!(e.lp(LP1), 100_000);
        assert_custody_and_unlocked(&e);
        // Allowances were consumed exactly.
        assert!(!e.pool(LP1, encode_v8_add_liquidity_calldata(1, 4, 0, 0)).ok);
    }

    fn assert_custody_and_unlocked(e: &Env) {
        e.assert_custody();
        assert_eq!(e.w(pool_v8_lock_key()), 0);
    }

    #[test]
    fn no_deposit_without_pull() {
        let mut e = setup();
        let before = e.snapshot();
        // No approval at all.
        assert!(
            !e.pool(LP1, encode_v8_add_liquidity_calldata(1_000, 1_000, 0, 0))
                .ok
        );
        assert_eq!(e.snapshot(), before);
        // Only token A approved: pull A succeeds, pull B fails -> whole call reverted (A refunded).
        e.approve(LP1, TA_ID, 1_000);
        let before = e.snapshot();
        assert!(
            !e.pool(LP1, encode_v8_add_liquidity_calldata(1_000, 1_000, 0, 0))
                .ok
        );
        assert_eq!(e.snapshot(), before);
        assert_eq!(e.bal(TA_ID, LP1), 1_000_000);
        // Approved but balance short.
        e.approve(LP1, TB_ID, 5_000_000);
        e.approve(LP1, TA_ID, 5_000_000);
        let before = e.snapshot();
        assert!(
            !e.pool(
                LP1,
                encode_v8_add_liquidity_calldata(1_000, 2_000_000, 0, 0)
            )
            .ok
        );
        assert_eq!(e.snapshot(), before);
    }

    #[test]
    fn requires_configured_tokens_and_set_tokens_once() {
        let mut e = Env {
            st: StateStore::new(),
            height: 1,
        };
        e.deploy(POOL, constant_product_pool_bytecode_v8());
        assert!(!e.pool(LP1, encode_v8_add_liquidity_calldata(1, 1, 0, 0)).ok);
        assert!(!e.pool(LP1, encode_v8_swap_calldata(0, 1, 0, 0)).ok);
        assert!(!e.pool(LP1, encode_set_tokens_calldata(&TA_ID, &TA_ID)).ok);
        assert!(
            !e.pool(LP1, encode_set_tokens_calldata(&TA_ID, &AccountId([0; 32])))
                .ok
        );
        assert!(e.pool(LP1, encode_set_tokens_calldata(&TA_ID, &TB_ID)).ok);
        assert!(!e.pool(LP1, encode_set_tokens_calldata(&TB_ID, &TA_ID)).ok);
        assert!(!e.pool(LP1, vec![0u8; 32]).ok, "unknown selector faults");
    }

    #[test]
    fn swap_pulls_input_matches_quote_and_keeps_k() {
        let mut e = setup();
        seed(&mut e);
        let mut k = 100_000u128 * 400_000;
        for (i, (dir, amt)) in [
            (0u128, 5_000u128),
            (1, 30_000),
            (0, 777),
            (1, 1),
            (0, 50_000),
            (1, 123_456),
        ]
        .into_iter()
        .enumerate()
        {
            let (ra, rb, _) = e.reserves();
            let (ri, ro, tin, tout) = if dir == 0 {
                (ra, rb, TA_ID, TB_ID)
            } else {
                (rb, ra, TB_ID, TA_ID)
            };
            let q = u128::from(constant_product_amount_out_fee_on_input_with_bps(
                ri as u64, ro as u64, amt as u64, 30,
            ));
            let (bi, bo) = (e.bal(tin, TRADER), e.bal(tout, TRADER));
            if q == 0 {
                e.approve(TRADER, tin, amt);
                assert!(
                    !e.pool(TRADER, encode_v8_swap_calldata(dir, amt, 0, 0)).ok,
                    "dust swap {i}"
                );
                continue;
            }
            e.approve(TRADER, tin, amt);
            let r = e.pool(TRADER, encode_v8_swap_calldata(dir, amt, q, 0));
            assert!(r.ok, "swap {i}");
            assert!(r.logs >= 1);
            assert_eq!(
                (e.bal(tin, TRADER), e.bal(tout, TRADER)),
                (bi - amt, bo + q)
            );
            let (ra2, rb2, _) = e.reserves();
            assert!(ra2 * rb2 >= k, "k decreased at swap {i}");
            k = ra2 * rb2;
            assert_custody_and_unlocked(&e);
        }
    }

    #[test]
    fn swap_without_approval_or_with_slippage_reverts_cleanly() {
        let mut e = setup();
        seed(&mut e);
        let before = e.snapshot();
        assert!(!e.pool(TRADER, encode_v8_swap_calldata(0, 1_000, 1, 0)).ok);
        assert_eq!(e.snapshot(), before);
        let q = u128::from(constant_product_amount_out_fee_on_input_with_bps(
            100_000, 400_000, 1_000, 30,
        ));
        e.approve(TRADER, TA_ID, 1_000);
        let before = e.snapshot();
        assert!(
            !e.pool(TRADER, encode_v8_swap_calldata(0, 1_000, q + 1, 0))
                .ok
        );
        assert_eq!(e.snapshot(), before);
        assert!(
            !e.pool(TRADER, encode_v8_swap_calldata(2, 1_000, 0, 0)).ok,
            "bad direction"
        );
        assert!(e.pool(TRADER, encode_v8_swap_calldata(0, 1_000, q, 0)).ok);
    }

    #[test]
    fn swap_to_pays_recipient() {
        let mut e = setup();
        seed(&mut e);
        let q = u128::from(constant_product_amount_out_fee_on_input_with_bps(
            400_000, 100_000, 4_000, 30,
        ));
        e.approve(TRADER, TB_ID, 4_000);
        assert!(
            e.pool(TRADER, encode_v8_swap_to_calldata(1, 4_000, q, &LP2, 0))
                .ok
        );
        assert_eq!(e.bal(TA_ID, LP2), 1_000_000 + q);
        assert_custody_and_unlocked(&e);
    }

    #[test]
    fn deadline_is_enforced() {
        let mut e = setup();
        e.approve(LP1, TA_ID, 10);
        e.approve(LP1, TB_ID, 10);
        e.height = 500;
        assert!(
            !e.pool(LP1, encode_v8_add_liquidity_calldata(10, 10, 0, 499))
                .ok
        );
        assert!(
            e.pool(LP1, encode_v8_add_liquidity_calldata(10, 10, 0, 500))
                .ok
        );
        e.approve(TRADER, TA_ID, 5);
        assert!(!e.pool(TRADER, encode_v8_swap_calldata(0, 5, 1, 1)).ok);
        assert!(
            !e.pool(LP1, encode_v8_remove_liquidity_calldata(1, 0, 0, 2))
                .ok
        );
        assert!(
            e.pool(LP1, encode_v8_remove_liquidity_calldata(1, 0, 0, 0))
                .ok
        );
    }

    #[test]
    fn lp_share_math_min_liquidity_and_no_overdraft() {
        let mut e = setup();
        seed(&mut e);
        // Proportional second deposit; extra B beyond the ratio is not rewarded.
        e.approve(LP2, TA_ID, 25_000);
        e.approve(LP2, TB_ID, 120_000);
        assert!(
            !e.pool(
                LP2,
                encode_v8_add_liquidity_calldata(25_000, 120_000, 25_001, 0)
            )
            .ok,
            "min_liquidity"
        );
        let r = e.pool(
            LP2,
            encode_v8_add_liquidity_calldata(25_000, 120_000, 25_000, 0),
        );
        assert!(r.ok);
        assert_eq!(e.lp(LP2), 25_000);
        assert_eq!(e.reserves(), (125_000, 520_000, 125_000));
        assert_custody_and_unlocked(&e);
        // Overdraft: burn more than held / burn with none held.
        let before = e.snapshot();
        assert!(
            !e.pool(LP2, encode_v8_remove_liquidity_calldata(25_001, 0, 0, 0))
                .ok
        );
        assert!(
            !e.pool(ATTACKER, encode_v8_remove_liquidity_calldata(1, 0, 0, 0))
                .ok
        );
        assert_eq!(e.snapshot(), before);
        // Slippage on removal.
        let (oa, ob) = (
            25_000u128 * 125_000 / 125_000,
            25_000u128 * 520_000 / 125_000,
        );
        assert!(
            !e.pool(
                LP2,
                encode_v8_remove_liquidity_calldata(25_000, oa + 1, 0, 0)
            )
            .ok
        );
        let (a0, b0) = (e.bal(TA_ID, LP2), e.bal(TB_ID, LP2));
        assert!(
            e.pool(LP2, encode_v8_remove_liquidity_calldata(25_000, oa, ob, 0))
                .ok
        );
        assert_eq!((e.bal(TA_ID, LP2), e.bal(TB_ID, LP2)), (a0 + oa, b0 + ob));
        assert_eq!(e.lp(LP2), 0);
        assert_custody_and_unlocked(&e);
        // remove_liquidity_to routes payouts.
        assert!(
            e.pool(
                LP1,
                encode_v8_remove_liquidity_to_calldata(10_000, 0, 0, &LP2, &TRADER, 0)
            )
            .ok
        );
        assert_eq!(e.bal(TA_ID, LP2), a0 + oa + 10_000);
        assert_custody_and_unlocked(&e);
    }

    /// The v1..v7 "fake deposit" drain: deposit nothing, withdraw real tokens. Fails on v8.
    #[test]
    fn fake_deposit_drain_attack_fails() {
        let mut e = setup();
        seed(&mut e);
        let before = e.snapshot();
        assert!(
            !e.pool(
                ATTACKER,
                encode_v8_add_liquidity_calldata(1_000_000, 1_000_000, 0, 0)
            )
            .ok
        );
        assert!(
            !e.pool(ATTACKER, encode_v8_swap_calldata(0, 90_000, 1, 0))
                .ok
        );
        // Approving without balance doesn't help either.
        e.approve(ATTACKER, TA_ID, u128::MAX);
        e.approve(ATTACKER, TB_ID, u128::MAX);
        let before2 = e.snapshot();
        assert!(
            !e.pool(
                ATTACKER,
                encode_v8_add_liquidity_calldata(1_000_000, 1_000_000, 0, 0)
            )
            .ok
        );
        assert!(
            !e.pool(ATTACKER, encode_v8_swap_calldata(1, 300_000, 1, 0))
                .ok
        );
        assert!(
            !e.pool(ATTACKER, encode_v8_remove_liquidity_calldata(1, 0, 0, 0))
                .ok
        );
        assert_eq!(e.snapshot(), before2);
        assert_eq!(e.reserves(), (before[0], before[1], before[2]));
        assert_eq!(e.bal(TA_ID, ATTACKER) + e.bal(TB_ID, ATTACKER), 0);
    }

    /// A malicious "token" that re-enters the pool from inside `transfer_from` and then reports
    /// success. The reentrant call hits the lock and faults, so the whole tx reverts: no credit.
    fn evil_token_env(reenter: bool) -> (Env, AccountId) {
        let evil = AccountId([0xee; 32]);
        let mut e = Env {
            st: StateStore::new(),
            height: 1,
        };
        e.deploy(TB_ID, reference_fungible_template_v2_bytecode());
        assert!(
            e.call(ADMIN, TB_ID, encode_mint_first_calldata(&LP1, 1_000_000))
                .ok
        );
        let mut a = Asm::new();
        if reenter {
            let cd = encode_v8_add_liquidity_calldata(1, 1, 0, 0);
            for (i, ch) in cd.chunks(32).enumerate() {
                let mut w = [0u8; 32];
                w.copy_from_slice(ch);
                a.push(&w).mstore(4096 + 32 * i as u64);
            }
            a.push(&POOL.0)
                .push_u(4096)
                .push_u(cd.len() as u64)
                .push_u(8192)
                .push_u(32)
                .op(Opcode::Call);
        }
        a.push_u(1).mstore(8192).return_word(8192);
        e.deploy(evil, a.finish());
        (e, evil)
    }

    /// Control: the same fake token *without* re-entry is accepted (it "pays" by fiat), proving
    /// the failure below is caused by the reentrancy lock.
    #[test]
    fn non_reentrant_fake_token_control() {
        let (mut e, evil) = evil_token_env(false);
        e.deploy(POOL, constant_product_pool_bytecode_v8());
        assert!(e.pool(ADMIN, encode_set_tokens_calldata(&evil, &TB_ID)).ok);
        e.approve(LP1, TB_ID, 1_000);
        assert!(
            e.pool(LP1, encode_v8_add_liquidity_calldata(1_000, 1_000, 0, 0))
                .ok
        );
        assert_eq!(e.lp(LP1), 1_000);
    }

    #[test]
    fn reentrant_callback_cannot_double_credit() {
        let (mut e, evil) = evil_token_env(true);
        e.deploy(POOL, constant_product_pool_bytecode_v8());
        assert!(e.pool(ADMIN, encode_set_tokens_calldata(&evil, &TB_ID)).ok);
        e.approve(LP1, TB_ID, 1_000);
        let before = (e.reserves(), e.lp(LP1), e.bal(TB_ID, LP1));
        assert!(
            !e.pool(LP1, encode_v8_add_liquidity_calldata(1_000, 1_000, 0, 0))
                .ok
        );
        assert_eq!((e.reserves(), e.lp(LP1), e.bal(TB_ID, LP1)), before);
        assert_eq!(e.w(pool_v8_lock_key()), 0);
    }

    #[test]
    fn fee_and_fee_admin_rules() {
        let mut e = setup();
        assert!(e.pool(ADMIN, encode_set_fee_admin_calldata(&ADMIN)).ok);
        let raw_fee = |n: u128| {
            let mut v = w_sel(SELECTOR_SET_SWAP_FEE_BPS).to_vec();
            v.extend_from_slice(&amount_word(n));
            v
        };
        assert!(!e.pool(ADMIN, raw_fee(0)).ok);
        assert!(!e.pool(ADMIN, raw_fee(10_001)).ok);
        assert!(e.pool(ADMIN, encode_set_swap_fee_bps_calldata(100)).ok);
        seed(&mut e);
        assert_eq!(e.w(swap_fee_bps_key()), 100);
        assert!(
            !e.pool(LP1, encode_set_swap_fee_bps_calldata(50)).ok,
            "only fee admin after LP"
        );
        assert!(
            !e.pool(ADMIN, encode_set_fee_admin_calldata(&LP1)).ok,
            "admin fixed after LP"
        );
        assert!(e.pool(ADMIN, encode_set_swap_fee_bps_calldata(50)).ok);
        let q = u128::from(constant_product_amount_out_fee_on_input_with_bps(
            100_000, 400_000, 10_000, 50,
        ));
        e.approve(TRADER, TA_ID, 10_000);
        assert!(e.pool(TRADER, encode_v8_swap_calldata(0, 10_000, q, 0)).ok);
        assert_eq!(e.bal(TB_ID, TRADER), 1_000_000 + q);
    }

    #[test]
    fn default_fee_set_on_first_mint() {
        let mut e = setup();
        seed(&mut e);
        assert_eq!(e.w(swap_fee_bps_key()), u128::from(NATIVE_CP_SWAP_FEE_BPS));
    }

    #[test]
    fn pool_v8_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult};
        let code = constant_product_pool_bytecode_v8();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &RuleRegistry::new());
        assert!(matches!(r, QaResult::Allow | QaResult::Unsure), "{r:?}");
        assert_ne!(code, crate::native_amm::constant_product_pool_bytecode_v7());
    }
}
