//! **LP vault v2** for custody pools (**v8 / v9**): users deposit real tokens, the vault adds
//! liquidity and mints vault **shares**; withdrawing burns shares and removes liquidity straight
//! to the user. (The v1 vault, [`crate::native_amm_lp_vault`], only drives ledger-only v1..v7
//! pools, is configure-once and has no withdraw.)
//!
//! | Selector | Words after selector |
//! |---|---|
//! | `0xC0` configure | `pool`, `token_a`, `token_b` (once; non-zero, distinct tokens) |
//! | `0xC1` deposit | `amount_a`, `amount_b`, `min_shares`, `deadline` -> returns shares minted |
//! | `0xC2` withdraw | `shares`, `min_a`, `min_b`, `deadline` |
//! | `0xC3` transfer_shares | `to`, `amount` |
//!
//! **deposit:** `transfer_from(CALLER -> vault)` both amounts (user approves the **vault**), then
//! `approve(pool, amount)` on each token and `pool.add_liquidity(amount_a, amount_b, min_shares,
//! deadline)`. The pool pulls exactly those amounts from the vault, so the vault keeps no tokens and
//! no leftover allowance. Shares are minted **1:1** with the LP the pool returns (LP held by the vault
//! only changes through the vault, so 1:1 stays exact; swap fees accrue to the pool reserves and so
//! to every share). The pool consumes both amounts even when they are off-ratio (the excess is a
//! donation to all LPs, exactly as with a direct `add_liquidity`), so callers should size deposits
//! from live reserves and set `min_shares` (the SDK helper does both).
//!
//! **withdraw:** debits shares **before** the external call, then
//! `pool.remove_liquidity_to(shares, min_a, min_b, CALLER, CALLER, deadline)`: the pool pays the user
//! directly; the vault never touches the output tokens.
//!
//! **transfer_shares:** a self-transfer (`to == CALLER`) is a no-op (no balance doubling); the
//! recipient balance is read **after** the sender debit.
//!
//! Safety: every rejection is a VM **fault**, so the block executor reverts the whole transaction
//! (including token pulls and pool calls). Reentrancy lock (`k[31] = 0xe6`). A mis-configured token
//! pair can't lose funds: the pool pulls *its* tokens from the vault, which holds none, so deposits
//! fault; withdrawals are paid by the pool itself.

use boing_primitives::AccountId;

use crate::asm::{w_sel, w_u64, Asm, Label};
use crate::bytecode::Opcode;
use crate::native_amm::{SELECTOR_ADD_LIQUIDITY, SELECTOR_REMOVE_LIQUIDITY_TO};
use crate::reference_token::amount_word;
use crate::reference_token_v2::{SELECTOR_APPROVE, SELECTOR_TRANSFER_FROM};

pub const SELECTOR_LP_VAULT_V2_CONFIGURE: u8 = 0xC0;
pub const SELECTOR_LP_VAULT_V2_DEPOSIT: u8 = 0xC1;
pub const SELECTOR_LP_VAULT_V2_WITHDRAW: u8 = 0xC2;
pub const SELECTOR_LP_VAULT_V2_TRANSFER_SHARES: u8 = 0xC3;

/// CREATE2 salt for [`native_lp_vault_v2_bytecode`].
pub const NATIVE_LP_VAULT_V2_CREATE2_SALT: [u8; 32] =
    *b"BOING_LP_VAULT_V2\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Share balance slot: `holder ^ LP_VAULT_V2_SHARE_XOR`.
pub const LP_VAULT_V2_SHARE_XOR: [u8; 32] =
    *b"BOING_LP_VAULT_V2_SHARES\x00\x00\x00\x00\x00\x00\x00\x00";

pub const LP_VAULT_V2_TOPIC_DEPOSIT: [u8; 32] =
    *b"BOING_LP_VAULT_V2_DEPOSIT\x00\x00\x00\x00\x00\x00\x00";
pub const LP_VAULT_V2_TOPIC_WITHDRAW: [u8; 32] =
    *b"BOING_LP_VAULT_V2_WITHDRAW\x00\x00\x00\x00\x00\x00";
pub const LP_VAULT_V2_TOPIC_TRANSFER: [u8; 32] =
    *b"BOING_LP_VAULT_V2_TRANSFER\x00\x00\x00\x00\x00\x00";

#[must_use]
pub fn lp_vault_v2_configured_key() -> [u8; 32] {
    w_u64(0xe1)
}
#[must_use]
pub fn lp_vault_v2_pool_key() -> [u8; 32] {
    w_u64(0xe2)
}
#[must_use]
pub fn lp_vault_v2_token_a_key() -> [u8; 32] {
    w_u64(0xe3)
}
#[must_use]
pub fn lp_vault_v2_token_b_key() -> [u8; 32] {
    w_u64(0xe4)
}
#[must_use]
pub fn lp_vault_v2_total_shares_key() -> [u8; 32] {
    w_u64(0xe5)
}
#[must_use]
pub fn lp_vault_v2_lock_key() -> [u8; 32] {
    w_u64(0xe6)
}
#[must_use]
pub fn lp_vault_v2_share_key(holder: &AccountId) -> [u8; 32] {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = holder.0[i] ^ LP_VAULT_V2_SHARE_XOR[i];
    }
    k
}

#[must_use]
pub fn encode_lp_vault_v2_configure_calldata(
    pool: &AccountId,
    token_a: &AccountId,
    token_b: &AccountId,
) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_LP_VAULT_V2_CONFIGURE).to_vec();
    for a in [pool, token_a, token_b] {
        v.extend_from_slice(&a.0);
    }
    v
}

#[must_use]
pub fn encode_lp_vault_v2_deposit_calldata(
    amount_a: u128,
    amount_b: u128,
    min_shares: u128,
    deadline: u64,
) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_LP_VAULT_V2_DEPOSIT).to_vec();
    for w in [amount_a, amount_b, min_shares, u128::from(deadline)] {
        v.extend_from_slice(&amount_word(w));
    }
    v
}

#[must_use]
pub fn encode_lp_vault_v2_withdraw_calldata(
    shares: u128,
    min_a: u128,
    min_b: u128,
    deadline: u64,
) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_LP_VAULT_V2_WITHDRAW).to_vec();
    for w in [shares, min_a, min_b, u128::from(deadline)] {
        v.extend_from_slice(&amount_word(w));
    }
    v
}

#[must_use]
pub fn encode_lp_vault_v2_transfer_shares_calldata(to: &AccountId, amount: u128) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_LP_VAULT_V2_TRANSFER_SHARES).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&amount_word(amount));
    v
}

// Scratch memory (calldata < 256 bytes).
const A: u64 = 1024;
const B: u64 = 1056;
const MIN: u64 = 1088;
const DL: u64 = 1120;
const POOL: u64 = 1152;
const TA: u64 = 1184;
const TB: u64 = 1216;
const KEY: u64 = 1248;
const BAL: u64 = 1280;
const S: u64 = 1312;
const TO: u64 = 1344;
const KEY2: u64 = 1376;
const CB: u64 = 1536;
const RET: u64 = 1856;
const LOGB: u64 = 1920;

struct V {
    a: Asm,
    fail: Label,
}

impl V {
    fn nonzero(&mut self, x: u64) {
        let f = self.fail;
        self.a.mload(x).op(Opcode::IsZero).jumpi(f);
    }
    fn ge(&mut self, x: u64, y: u64) {
        let f = self.fail;
        self.a.mload(x).mload(y).op(Opcode::Lt).jumpi(f);
    }
    /// `CALL mem[to]` with `len` bytes at `CB`, 32-byte return at `RET` (zeroed first); fault if the call fails.
    fn call(&mut self, to: u64, len: u64) {
        let f = self.fail;
        self.a.push_u(0).mstore(RET);
        self.a
            .mload(to)
            .push_u(CB)
            .push_u(len)
            .push_u(RET)
            .push_u(32)
            .op(Opcode::Call);
        self.a.op(Opcode::IsZero).jumpi(f);
    }
    fn ret_is_one(&mut self) {
        let f = self.fail;
        self.a
            .mload(RET)
            .push_u(1)
            .op(Opcode::Eq)
            .op(Opcode::IsZero)
            .jumpi(f);
    }
    /// `transfer_from(CALLER -> vault, mem[amount])` on token `mem[token]`; must return 1.
    fn pull(&mut self, token: u64, amount: u64) {
        self.a.push(&w_sel(SELECTOR_TRANSFER_FROM)).mstore(CB);
        self.a.op(Opcode::Caller).mstore(CB + 32);
        self.a.op(Opcode::Address).mstore(CB + 64);
        self.a.mcopy(amount, CB + 96);
        self.call(token, 128);
        self.ret_is_one();
    }
    /// `approve(mem[spender], mem[amount])` on token `mem[token]`; must return 1.
    fn approve(&mut self, token: u64, spender: u64, amount: u64) {
        self.a.push(&w_sel(SELECTOR_APPROVE)).mstore(CB);
        self.a.mcopy(spender, CB + 32).mcopy(amount, CB + 64);
        self.call(token, 96);
        self.ret_is_one();
    }
    /// `mem[KEY] = holder ^ XOR` for the holder word on the stack.
    fn share_key(&mut self, dst: u64) {
        self.a
            .push(&LP_VAULT_V2_SHARE_XOR)
            .op(Opcode::Xor)
            .mstore(dst);
    }
    fn log(&mut self, topic: &[u8; 32], x: u64, y: u64, z: u64) {
        let a = &mut self.a;
        a.mcopy(x, LOGB).mcopy(y, LOGB + 32).mcopy(z, LOGB + 64);
        a.push(topic)
            .op(Opcode::Caller)
            .push_u(96)
            .push_u(LOGB)
            .op(Opcode::Log2);
    }
    fn unlock(&mut self) {
        self.a.push_u(0).sstore_key(&lp_vault_v2_lock_key());
    }
    fn require_configured(&mut self) {
        let f = self.fail;
        self.a
            .sload_key(&lp_vault_v2_configured_key())
            .op(Opcode::IsZero)
            .jumpi(f);
    }
}

/// LP vault v2 bytecode (purpose `dapp`).
#[must_use]
pub fn native_lp_vault_v2_bytecode() -> Vec<u8> {
    let mut a = Asm::new();
    let fail = a.label();
    let mut v = V { a, fail };
    let (configure, deposit, withdraw, transfer) =
        (v.a.label(), v.a.label(), v.a.label(), v.a.label());

    v.a.sload_key(&lp_vault_v2_lock_key()).jumpi(fail);
    v.a.push_u(1).sstore_key(&lp_vault_v2_lock_key());
    for (s, l) in [
        (SELECTOR_LP_VAULT_V2_CONFIGURE, configure),
        (SELECTOR_LP_VAULT_V2_DEPOSIT, deposit),
        (SELECTOR_LP_VAULT_V2_WITHDRAW, withdraw),
        (SELECTOR_LP_VAULT_V2_TRANSFER_SHARES, transfer),
    ] {
        v.a.mload(0).push(&w_sel(s)).op(Opcode::Eq).jumpi(l);
    }
    v.a.jump(fail);

    // --- configure(pool, token_a, token_b): once.
    v.a.mark(configure);
    v.a.sload_key(&lp_vault_v2_configured_key()).jumpi(fail);
    v.nonzero(32);
    v.nonzero(64);
    v.nonzero(96);
    v.a.mload(64).mload(96).op(Opcode::Eq).jumpi(fail);
    v.a.mload(32).sstore_key(&lp_vault_v2_pool_key());
    v.a.mload(64).sstore_key(&lp_vault_v2_token_a_key());
    v.a.mload(96).sstore_key(&lp_vault_v2_token_b_key());
    v.a.push_u(1).sstore_key(&lp_vault_v2_configured_key());
    v.unlock();
    v.a.op(Opcode::Stop);

    // --- deposit(amount_a, amount_b, min_shares, deadline) -> shares.
    v.a.mark(deposit);
    v.require_configured();
    v.a.mcopy(32, A).mcopy(64, B).mcopy(96, MIN).mcopy(128, DL);
    v.nonzero(A);
    v.nonzero(B);
    v.a.sload_key(&lp_vault_v2_pool_key()).mstore(POOL);
    v.a.sload_key(&lp_vault_v2_token_a_key()).mstore(TA);
    v.a.sload_key(&lp_vault_v2_token_b_key()).mstore(TB);
    v.pull(TA, A);
    v.pull(TB, B);
    v.approve(TA, POOL, A);
    v.approve(TB, POOL, B);
    v.a.push(&w_sel(SELECTOR_ADD_LIQUIDITY)).mstore(CB);
    v.a.mcopy(A, CB + 32)
        .mcopy(B, CB + 64)
        .mcopy(MIN, CB + 96)
        .mcopy(DL, CB + 128);
    v.call(POOL, 160);
    v.a.mcopy(RET, S);
    v.nonzero(S);
    v.ge(S, MIN);
    v.a.op(Opcode::Caller);
    v.share_key(KEY);
    v.a.mload(KEY)
        .op(Opcode::SLoad)
        .mload(S)
        .op(Opcode::Add)
        .mload(KEY)
        .op(Opcode::SStore);
    v.a.sload_key(&lp_vault_v2_total_shares_key())
        .mload(S)
        .op(Opcode::Add)
        .sstore_key(&lp_vault_v2_total_shares_key());
    v.log(&LP_VAULT_V2_TOPIC_DEPOSIT, A, B, S);
    v.unlock();
    v.a.return_word(S);

    // --- withdraw(shares, min_a, min_b, deadline): burn first, then pool pays CALLER directly.
    v.a.mark(withdraw);
    v.require_configured();
    v.a.mcopy(32, S).mcopy(64, A).mcopy(96, B).mcopy(128, DL);
    v.nonzero(S);
    v.a.op(Opcode::Caller);
    v.share_key(KEY);
    v.a.mload(KEY).op(Opcode::SLoad).mstore(BAL);
    v.ge(BAL, S);
    v.a.mload(BAL)
        .mload(S)
        .op(Opcode::Sub)
        .mload(KEY)
        .op(Opcode::SStore);
    v.a.sload_key(&lp_vault_v2_total_shares_key())
        .mload(S)
        .op(Opcode::Sub)
        .sstore_key(&lp_vault_v2_total_shares_key());
    v.a.sload_key(&lp_vault_v2_pool_key()).mstore(POOL);
    v.a.push(&w_sel(SELECTOR_REMOVE_LIQUIDITY_TO)).mstore(CB);
    v.a.mcopy(S, CB + 32).mcopy(A, CB + 64).mcopy(B, CB + 96);
    v.a.op(Opcode::Caller).mstore(CB + 128);
    v.a.op(Opcode::Caller).mstore(CB + 160);
    v.a.mcopy(DL, CB + 192);
    v.call(POOL, 224);
    v.log(&LP_VAULT_V2_TOPIC_WITHDRAW, S, A, B);
    v.unlock();
    v.a.op(Opcode::Stop);

    // --- transfer_shares(to, amount): self-transfer is a no-op.
    v.a.mark(transfer);
    let done = v.a.label();
    v.a.mcopy(32, TO).mcopy(64, S);
    v.nonzero(TO);
    v.nonzero(S);
    v.a.op(Opcode::Caller);
    v.share_key(KEY);
    v.a.mload(KEY).op(Opcode::SLoad).mstore(BAL);
    v.ge(BAL, S);
    v.a.mload(TO).op(Opcode::Caller).op(Opcode::Eq).jumpi(done);
    v.a.mload(BAL)
        .mload(S)
        .op(Opcode::Sub)
        .mload(KEY)
        .op(Opcode::SStore);
    v.a.mload(TO);
    v.share_key(KEY2);
    v.a.mload(KEY2)
        .op(Opcode::SLoad)
        .mload(S)
        .op(Opcode::Add)
        .mload(KEY2)
        .op(Opcode::SStore);
    v.log(&LP_VAULT_V2_TOPIC_TRANSFER, TO, S, S);
    v.a.mark(done);
    v.unlock();
    v.a.op(Opcode::Stop);

    v.a.mark(fail);
    v.a.fault();
    v.a.finish()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::{Interpreter, VmExecutionContext};
    use crate::native_amm::{
        encode_set_tokens_calldata, lp_balance_storage_key, reserve_a_key, reserve_b_key,
        total_lp_supply_key,
    };
    use crate::native_amm_v8::{
        constant_product_pool_bytecode_v8, constant_product_pool_bytecode_v9,
        encode_v8_add_liquidity_calldata, encode_v8_swap_calldata,
    };
    use crate::reference_token::{
        encode_mint_first_calldata, encode_transfer_calldata, REF_FUNGIBLE_BALANCE_XOR,
    };
    use crate::reference_token_v2::{
        encode_approve_calldata, ref_fungible_allowance_key,
        reference_fungible_template_v2_bytecode,
    };
    use boing_primitives::Account;
    use boing_qa::RuleRegistry;
    use boing_state::StateStore;

    const ALICE: AccountId = AccountId([0xa1; 32]);
    const BOB: AccountId = AccountId([0xb2; 32]);
    const CAROL: AccountId = AccountId([0xc3; 32]);
    const VAULT: AccountId = AccountId([0xd0; 32]);
    const VAULT8: AccountId = AccountId([0xd8; 32]);
    const TA: AccountId = AccountId([0x0a; 32]);
    const TB: AccountId = AccountId([0x0b; 32]);
    const TC: AccountId = AccountId([0x0c; 32]);
    const P9: AccountId = AccountId([0x91; 32]);
    const P8: AccountId = AccountId([0x81; 32]);

    struct Env {
        st: StateStore,
        height: u64,
    }

    fn tail(w: &[u8; 32]) -> u128 {
        u128::from_be_bytes(w[16..32].try_into().unwrap())
    }

    impl Env {
        fn call(&mut self, who: AccountId, to: AccountId, cd: &[u8]) -> Option<Vec<u8>> {
            let cp = self.st.checkpoint();
            let code = self.st.get_contract_code(&to).unwrap().clone();
            let mut it = Interpreter::new(code, 40_000_000);
            let ctx = VmExecutionContext {
                block_height: self.height,
                block_timestamp: 0,
                native_value: false,
                vm_hardening: false,
                blake3_opcode: false,
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
        fn shares(&self, vault: AccountId, who: AccountId) -> u128 {
            tail(
                &self
                    .st
                    .get_contract_storage(&vault, &lp_vault_v2_share_key(&who)),
            )
        }
        fn total_shares(&self, vault: AccountId) -> u128 {
            tail(
                &self
                    .st
                    .get_contract_storage(&vault, &lp_vault_v2_total_shares_key()),
            )
        }
        fn pool_lp(&self, pool: AccountId, who: AccountId) -> u128 {
            tail(
                &self
                    .st
                    .get_contract_storage(&pool, &lp_balance_storage_key(&who.0)),
            )
        }
        fn reserves(&self, p: AccountId) -> (u128, u128) {
            (
                tail(&self.st.get_contract_storage(&p, &reserve_a_key())),
                tail(&self.st.get_contract_storage(&p, &reserve_b_key())),
            )
        }
        /// Vault holds no tokens and no allowances; its pool LP equals total shares; lock released.
        fn vault_custody_ok(&self, vault: AccountId, pool: AccountId) {
            for t in [TA, TB, TC] {
                assert_eq!(self.bal(t, vault), 0, "vault keeps no tokens");
                let w = self
                    .st
                    .get_contract_storage(&t, &ref_fungible_allowance_key(&vault, &pool));
                assert_eq!(tail(&w), 0, "no leftover vault allowance");
            }
            assert_eq!(self.pool_lp(pool, vault), self.total_shares(vault));
            assert_eq!(
                self.st
                    .get_contract_storage(&vault, &lp_vault_v2_lock_key()),
                [0u8; 32]
            );
        }
    }

    fn env() -> Env {
        let mut st = StateStore::new();
        let tok = reference_fungible_template_v2_bytecode();
        let mut code = vec![
            (VAULT, native_lp_vault_v2_bytecode()),
            (VAULT8, native_lp_vault_v2_bytecode()),
        ];
        for t in [TA, TB, TC] {
            code.push((t, tok.clone()));
        }
        code.push((P9, constant_product_pool_bytecode_v9()));
        code.push((P8, constant_product_pool_bytecode_v8()));
        for id in [ALICE, BOB, CAROL] {
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
        for t in [TA, TB, TC] {
            e.ok(ALICE, t, &encode_mint_first_calldata(&ALICE, 10_000_000));
        }
        for p in [P9, P8] {
            e.ok(ALICE, p, &encode_set_tokens_calldata(&TA, &TB));
            e.ok(ALICE, TA, &encode_approve_calldata(&p, 1_000_000));
            e.ok(ALICE, TB, &encode_approve_calldata(&p, 2_000_000));
            e.ok(
                ALICE,
                p,
                &encode_v8_add_liquidity_calldata(1_000_000, 2_000_000, 1, 0),
            );
        }
        for t in [TA, TB, TC] {
            e.ok(ALICE, t, &encode_transfer_calldata(&BOB, 100_000));
        }
        e.ok(
            ALICE,
            VAULT,
            &encode_lp_vault_v2_configure_calldata(&P9, &TA, &TB),
        );
        e.ok(
            ALICE,
            VAULT8,
            &encode_lp_vault_v2_configure_calldata(&P8, &TA, &TB),
        );
        e
    }

    fn approve_vault(e: &mut Env, who: AccountId, vault: AccountId, a: u128, b: u128) {
        e.ok(who, TA, &encode_approve_calldata(&vault, a));
        e.ok(who, TB, &encode_approve_calldata(&vault, b));
    }

    #[test]
    fn deposit_mints_shares_1_to_1_with_lp_and_vault_keeps_nothing() {
        for (vault, pool) in [(VAULT, P9), (VAULT8, P8)] {
            let mut e = env();
            // Pool LP supply = min(1e6, 2e6) = 1e6; balanced 10k:20k mints 10k LP.
            approve_vault(&mut e, BOB, vault, 10_000, 20_000);
            let r = e.ok(
                BOB,
                vault,
                &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 10_000, 0),
            );
            assert_eq!(r, amount_word(10_000).to_vec());
            assert_eq!(e.shares(vault, BOB), 10_000);
            assert_eq!(e.total_shares(vault), 10_000);
            assert_eq!((e.bal(TA, BOB), e.bal(TB, BOB)), (90_000, 80_000));
            assert_eq!(e.reserves(pool), (1_010_000, 2_020_000));
            assert_eq!((e.bal(TA, pool), e.bal(TB, pool)), e.reserves(pool));
            assert_eq!(
                e.pool_lp(pool, BOB),
                0,
                "LP is held by the vault, not the user"
            );
            e.vault_custody_ok(vault, pool);
        }
    }

    #[test]
    fn withdraw_burns_shares_and_pool_pays_the_user_directly() {
        let mut e = env();
        approve_vault(&mut e, BOB, VAULT, 10_000, 20_000);
        e.ok(
            BOB,
            VAULT,
            &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 1, 0),
        );
        // A swap accrues a fee to the pool; shares capture it.
        e.ok(ALICE, TA, &encode_approve_calldata(&P9, 50_000));
        e.ok(ALICE, P9, &encode_v8_swap_calldata(0, 50_000, 1, 0));
        let (ra, rb) = e.reserves(P9);
        let t = tail(&e.st.get_contract_storage(&P9, &total_lp_supply_key()));
        let (oa, ob) = (4_000 * ra / t, 4_000 * rb / t);
        e.ok(
            BOB,
            VAULT,
            &encode_lp_vault_v2_withdraw_calldata(4_000, oa, ob, 0),
        );
        assert_eq!(e.shares(VAULT, BOB), 6_000);
        assert_eq!(e.total_shares(VAULT), 6_000);
        assert_eq!((e.bal(TA, BOB), e.bal(TB, BOB)), (90_000 + oa, 80_000 + ob));
        e.vault_custody_ok(VAULT, P9);
        // Withdraw the rest; vault ends empty.
        e.ok(
            BOB,
            VAULT,
            &encode_lp_vault_v2_withdraw_calldata(6_000, 1, 1, 0),
        );
        assert_eq!(e.total_shares(VAULT), 0);
        e.vault_custody_ok(VAULT, P9);
    }

    #[test]
    fn share_transfers_and_self_transfer_does_not_mint() {
        let mut e = env();
        approve_vault(&mut e, BOB, VAULT, 10_000, 20_000);
        e.ok(
            BOB,
            VAULT,
            &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 1, 0),
        );
        e.ok(
            BOB,
            VAULT,
            &encode_lp_vault_v2_transfer_shares_calldata(&BOB, 10_000),
        );
        assert_eq!(e.shares(VAULT, BOB), 10_000, "self-transfer is a no-op");
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_transfer_shares_calldata(&BOB, 10_001)
            )
            .is_none());
        e.ok(
            BOB,
            VAULT,
            &encode_lp_vault_v2_transfer_shares_calldata(&CAROL, 3_000),
        );
        assert_eq!(
            (e.shares(VAULT, BOB), e.shares(VAULT, CAROL)),
            (7_000, 3_000)
        );
        assert_eq!(e.total_shares(VAULT), 10_000);
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_transfer_shares_calldata(&CAROL, 7_001)
            )
            .is_none());
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_transfer_shares_calldata(&AccountId([0; 32]), 1)
            )
            .is_none());
        // CAROL can withdraw her shares; BOB can't withdraw more than he has.
        e.ok(
            CAROL,
            VAULT,
            &encode_lp_vault_v2_withdraw_calldata(3_000, 1, 1, 0),
        );
        assert!(e.bal(TA, CAROL) > 0 && e.bal(TB, CAROL) > 0);
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_withdraw_calldata(7_001, 1, 1, 0)
            )
            .is_none());
        assert_eq!(e.shares(VAULT, BOB), 7_000);
        e.vault_custody_ok(VAULT, P9);
    }

    #[test]
    fn rejections_fault_and_revert_everything() {
        let mut e = env();
        let snap = |e: &Env| {
            (
                e.bal(TA, BOB),
                e.bal(TB, BOB),
                e.reserves(P9),
                e.shares(VAULT, BOB),
                e.total_shares(VAULT),
            )
        };
        let before = snap(&e);
        // No approval to the vault.
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 1, 0)
            )
            .is_none());
        // Only token A approved: the A pull is undone too.
        e.ok(BOB, TA, &encode_approve_calldata(&VAULT, 10_000));
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 1, 0)
            )
            .is_none());
        approve_vault(&mut e, BOB, VAULT, 10_000, 20_000);
        // min_shares too high, deadline passed, zero amount.
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 10_001, 0)
            )
            .is_none());
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 1, 99)
            )
            .is_none());
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_deposit_calldata(0, 20_000, 1, 0)
            )
            .is_none());
        // Withdraw without shares; unknown selector; reconfigure.
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_withdraw_calldata(1, 0, 0, 0)
            )
            .is_none());
        assert!(e.call(BOB, VAULT, &w_sel(0xC9)).is_none());
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_configure_calldata(&P8, &TA, &TB)
            )
            .is_none());
        assert_eq!(snap(&e), before);
        // Still works afterwards (lock released by every revert).
        e.ok(
            BOB,
            VAULT,
            &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 10_000, 100),
        );
        // Withdraw slippage too high reverts the share burn.
        assert!(e
            .call(
                BOB,
                VAULT,
                &encode_lp_vault_v2_withdraw_calldata(10_000, 1_000_000, 1, 0)
            )
            .is_none());
        assert_eq!(e.shares(VAULT, BOB), 10_000);
        e.vault_custody_ok(VAULT, P9);
    }

    #[test]
    fn unconfigured_or_misconfigured_vault_cannot_take_funds() {
        let mut e = env();
        let fresh = AccountId([0xd1; 32]);
        e.st.insert(Account {
            id: fresh,
            state: Default::default(),
        });
        e.st.set_contract_code(fresh, native_lp_vault_v2_bytecode());
        approve_vault(&mut e, BOB, fresh, 10_000, 20_000);
        assert!(e
            .call(
                BOB,
                fresh,
                &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 1, 0)
            )
            .is_none());
        assert!(e
            .call(
                ALICE,
                fresh,
                &encode_lp_vault_v2_configure_calldata(&P9, &TA, &TA)
            )
            .is_none());
        assert!(e
            .call(
                ALICE,
                fresh,
                &encode_lp_vault_v2_configure_calldata(&AccountId([0; 32]), &TA, &TB)
            )
            .is_none());
        // Wrong pair (A/C) for a TA/TB pool: the pool pulls TB from the vault, which has none -> fault.
        e.ok(
            ALICE,
            fresh,
            &encode_lp_vault_v2_configure_calldata(&P9, &TA, &TC),
        );
        e.ok(BOB, TC, &encode_approve_calldata(&fresh, 20_000));
        assert!(e
            .call(
                BOB,
                fresh,
                &encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 1, 0)
            )
            .is_none());
        assert_eq!((e.bal(TA, BOB), e.bal(TC, BOB)), (100_000, 100_000));
        assert_eq!(e.reserves(P9), (1_000_000, 2_000_000));
    }

    #[test]
    fn vault_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult};
        let r = check_contract_deploy_full(
            &native_lp_vault_v2_bytecode(),
            Some("dapp"),
            None,
            &RuleRegistry::new(),
        );
        assert!(matches!(r, QaResult::Allow), "{r:?}");
    }
}
