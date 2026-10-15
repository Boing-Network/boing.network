//! **Reference fungible template v2** -- v1 semantics plus allowances.
//!
//! New, versioned bytecode ([`reference_fungible_template_v2_bytecode`]); the v1 template is unchanged.
//!
//! | Selector | Calldata (32-byte words) | Effect |
//! |---|---|---|
//! | `0x01` `transfer` | `to`, `amount` | As v1, except a self-transfer (`to == CALLER`) is a no-op (v1 double-credits it). |
//! | `0x02` `mint_first` | `to`, `amount` | As v1 (one-time, admin only; admin = first caller). |
//! | `0x08` `approve` | `spender`, `amount` | Sets (overwrites) `allowance[CALLER][spender] = amount`; returns word `1`. |
//! | `0x09` `transfer_from` | `from`, `to`, `amount` | Requires `allowance[from][CALLER] >= amount` and `balance[from] >= amount`, `amount > 0`, `from != to`; moves the balance, decrements the allowance, returns word `1`. Any failure stops with **no return data** (callers see `0`). |
//! | `0x0a` `allowance` | `owner`, `spender` | Returns the allowance word. |
//!
//! Balances use the v1 slots (`holder ^ REF_FUNGIBLE_BALANCE_XOR`). Allowance slot:
//! `(owner[0..16] ‖ spender[0..16]) ^ REF_FUNGIBLE_ALLOWANCE_XOR` -- the owner half always comes
//! from `CALLER`, so colliding with another slot needs a 2^128 search on account ids.

use boing_primitives::AccountId;

use crate::asm::{w_sel, w_u64, Asm, Label};
use crate::bytecode::Opcode;
use crate::reference_token::{
    amount_word, ref_fungible_admin_key, ref_fungible_mint_once_key, REF_FUNGIBLE_BALANCE_XOR,
    SELECTOR_MINT_FIRST, SELECTOR_TRANSFER,
};

pub const SELECTOR_APPROVE: u8 = 0x08;
pub const SELECTOR_TRANSFER_FROM: u8 = 0x09;
pub const SELECTOR_ALLOWANCE: u8 = 0x0a;

/// XOR mask for allowance slots (ASCII `BOING_REFFUNGIBLE_ALW2`, zero-padded).
pub const REF_FUNGIBLE_ALLOWANCE_XOR: [u8; 32] =
    *b"BOING_REFFUNGIBLE_ALW2\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Storage key of `allowance[owner][spender]`.
#[must_use]
pub fn ref_fungible_allowance_key(owner: &AccountId, spender: &AccountId) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[..16].copy_from_slice(&owner.0[..16]);
    k[16..].copy_from_slice(&spender.0[..16]);
    for (b, x) in k.iter_mut().zip(REF_FUNGIBLE_ALLOWANCE_XOR.iter()) {
        *b ^= x;
    }
    k
}

pub fn encode_approve_calldata(spender: &AccountId, amount: u128) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_APPROVE).to_vec();
    v.extend_from_slice(&spender.0);
    v.extend_from_slice(&amount_word(amount));
    v
}

pub fn encode_transfer_from_calldata(from: &AccountId, to: &AccountId, amount: u128) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_TRANSFER_FROM).to_vec();
    v.extend_from_slice(&from.0);
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&amount_word(amount));
    v
}

pub fn encode_allowance_calldata(owner: &AccountId, spender: &AccountId) -> Vec<u8> {
    let mut v = w_sel(SELECTOR_ALLOWANCE).to_vec();
    v.extend_from_slice(&owner.0);
    v.extend_from_slice(&spender.0);
    v
}

const M_SEL: u64 = 512;
const M_TO: u64 = 544;
const M_AMT: u64 = 576;
const M_OLD: u64 = 608;
const M_BALTO: u64 = 640;
const M_NEW: u64 = 672;
const M_KC: u64 = 704;
const M_KT: u64 = 736;
const M_FROM: u64 = 768;
const M_AKEY: u64 = 800;
const M_ALW: u64 = 832;
const M_RET: u64 = 864;

fn one() -> [u8; 32] {
    w_u64(1)
}

/// Pushes `((mem[owner] >> 128) << 128 | mem[spender] >> 128) ^ ALLOWANCE_XOR` then stores it at `dst`.
fn allowance_key_to(a: &mut Asm, owner_mem: Option<u64>, spender_mem: Option<u64>, dst: u64) {
    match owner_mem {
        Some(m) => a.mload(m),
        None => a.op(Opcode::Caller),
    };
    a.push_u(128).op(Opcode::Shr).push_u(128).op(Opcode::Shl);
    match spender_mem {
        Some(m) => a.mload(m),
        None => a.op(Opcode::Caller),
    };
    a.push_u(128).op(Opcode::Shr).op(Opcode::Or);
    a.push(&REF_FUNGIBLE_ALLOWANCE_XOR)
        .op(Opcode::Xor)
        .mstore(dst);
}

/// Balance move `from -> to` of `mem[M_AMT]` (precondition `from != to`); jumps to `fail`
/// before any write if the balance is short or the credit overflows.
fn move_balance(a: &mut Asm, from_mem: Option<u64>, to_mem: u64, fail: Label) {
    match from_mem {
        Some(m) => a.mload(m),
        None => a.op(Opcode::Caller),
    };
    a.push(&REF_FUNGIBLE_BALANCE_XOR)
        .op(Opcode::Xor)
        .mstore(M_KC);
    a.mload(to_mem)
        .push(&REF_FUNGIBLE_BALANCE_XOR)
        .op(Opcode::Xor)
        .mstore(M_KT);
    a.mload(M_KC).op(Opcode::SLoad).mstore(M_OLD);
    a.mload(M_OLD).mload(M_AMT).op(Opcode::Lt).jumpi(fail);
    a.mload(M_KT).op(Opcode::SLoad).mstore(M_BALTO);
    a.mload(M_BALTO).mload(M_AMT).op(Opcode::Add).mstore(M_NEW);
    a.mload(M_NEW).mload(M_BALTO).op(Opcode::Lt).jumpi(fail);
    a.mload(M_OLD)
        .mload(M_AMT)
        .op(Opcode::Sub)
        .mload(M_KC)
        .op(Opcode::SStore);
    a.mload(M_NEW).mload(M_KT).op(Opcode::SStore);
}

/// Reference fungible **v2** bytecode (purpose `token`). See the module docs.
#[must_use]
pub fn reference_fungible_template_v2_bytecode() -> Vec<u8> {
    let mut a = Asm::new();
    let stop = a.label();
    let init = a.label();
    let dispatch = a.label();
    let mint = a.label();
    let xfer = a.label();
    let approve = a.label();
    let xfrom = a.label();
    let allowance = a.label();

    // Lazy admin (as v1).
    a.op(Opcode::Caller)
        .sload_key(&ref_fungible_admin_key())
        .op(Opcode::IsZero)
        .jumpi(init);
    a.jump(dispatch);
    a.mark(init);
    a.op(Opcode::Caller).sstore_key(&ref_fungible_admin_key());
    a.mark(dispatch);
    a.mload(0).push(&w_u64(0xff)).op(Opcode::And).mstore(M_SEL);
    for (s, l) in [
        (SELECTOR_MINT_FIRST, mint),
        (SELECTOR_TRANSFER, xfer),
        (SELECTOR_APPROVE, approve),
        (SELECTOR_TRANSFER_FROM, xfrom),
        (SELECTOR_ALLOWANCE, allowance),
    ] {
        a.mload(M_SEL).push(&w_sel(s)).op(Opcode::Eq).jumpi(l);
    }
    a.mark(stop);
    a.op(Opcode::Stop);

    // mint_first (v1 semantics).
    a.mark(mint);
    a.sload_key(&ref_fungible_mint_once_key())
        .op(Opcode::IsZero)
        .op(Opcode::IsZero)
        .jumpi(stop);
    a.op(Opcode::Caller)
        .sload_key(&ref_fungible_admin_key())
        .op(Opcode::Eq)
        .op(Opcode::IsZero)
        .jumpi(stop);
    a.mcopy(32, M_TO).mcopy(64, M_AMT);
    a.mload(M_AMT).op(Opcode::IsZero).jumpi(stop);
    a.mload(M_TO)
        .push(&REF_FUNGIBLE_BALANCE_XOR)
        .op(Opcode::Xor)
        .op(Opcode::SLoad)
        .mstore(M_OLD);
    a.mload(M_AMT).mload(M_OLD).op(Opcode::Add).mstore(M_NEW);
    a.mload(M_NEW).mload(M_OLD).op(Opcode::Lt).jumpi(stop);
    a.mload(M_NEW)
        .mload(M_TO)
        .push(&REF_FUNGIBLE_BALANCE_XOR)
        .op(Opcode::Xor)
        .op(Opcode::SStore);
    a.push(&one()).sstore_key(&ref_fungible_mint_once_key());
    a.op(Opcode::Stop);

    // transfer (v1 semantics; self-transfer is a no-op).
    a.mark(xfer);
    a.mload(64).op(Opcode::IsZero).jumpi(stop);
    a.mcopy(32, M_TO).mcopy(64, M_AMT);
    a.mload(M_TO).op(Opcode::Caller).op(Opcode::Eq).jumpi(stop);
    move_balance(&mut a, None, M_TO, stop);
    a.op(Opcode::Stop);

    // approve(spender, amount): overwrite.
    a.mark(approve);
    a.mcopy(32, M_TO).mcopy(64, M_AMT);
    allowance_key_to(&mut a, None, Some(M_TO), M_AKEY);
    a.mload(M_AMT).mload(M_AKEY).op(Opcode::SStore);
    a.push(&one()).mstore(M_RET).return_word(M_RET);

    // transfer_from(from, to, amount).
    a.mark(xfrom);
    a.mcopy(32, M_FROM).mcopy(64, M_TO).mcopy(96, M_AMT);
    a.mload(M_AMT).op(Opcode::IsZero).jumpi(stop);
    a.mload(M_FROM).mload(M_TO).op(Opcode::Eq).jumpi(stop);
    allowance_key_to(&mut a, Some(M_FROM), None, M_AKEY);
    a.mload(M_AKEY).op(Opcode::SLoad).mstore(M_ALW);
    a.mload(M_ALW).mload(M_AMT).op(Opcode::Lt).jumpi(stop);
    move_balance(&mut a, Some(M_FROM), M_TO, stop);
    a.mload(M_ALW)
        .mload(M_AMT)
        .op(Opcode::Sub)
        .mload(M_AKEY)
        .op(Opcode::SStore);
    a.push(&one()).mstore(M_RET).return_word(M_RET);

    // allowance(owner, spender).
    a.mark(allowance);
    a.mcopy(32, M_FROM).mcopy(64, M_TO);
    allowance_key_to(&mut a, Some(M_FROM), Some(M_TO), M_AKEY);
    a.mload(M_AKEY)
        .op(Opcode::SLoad)
        .mstore(M_RET)
        .return_word(M_RET);

    a.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::Interpreter;
    use crate::reference_token::{
        encode_mint_first_calldata, encode_transfer_calldata, reference_fungible_template_bytecode,
    };
    use boing_primitives::Account;
    use boing_state::StateStore;

    fn bal_key(h: &AccountId) -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = h.0[i] ^ REF_FUNGIBLE_BALANCE_XOR[i];
        }
        k
    }
    fn u(w: [u8; 32]) -> u128 {
        u128::from_be_bytes(w[16..].try_into().unwrap())
    }
    struct T {
        st: StateStore,
        tok: AccountId,
    }
    impl T {
        fn new(code: Vec<u8>) -> Self {
            let tok = AccountId([0x77; 32]);
            let mut st = StateStore::new();
            st.insert(Account {
                id: tok,
                state: Default::default(),
            });
            st.set_contract_code(tok, code);
            Self { st, tok }
        }
        fn call(&mut self, who: AccountId, cd: Vec<u8>) -> Option<Vec<u8>> {
            let code = self.st.get_contract_code(&self.tok).unwrap().clone();
            let mut it = Interpreter::new(code, 5_000_000);
            it.run(who, self.tok, &cd, &mut self.st).unwrap();
            it.return_data
        }
        fn bal(&self, h: &AccountId) -> u128 {
            u(self.st.get_contract_storage(&self.tok, &bal_key(h)))
        }
        fn alw(&self, o: &AccountId, s: &AccountId) -> u128 {
            u(self
                .st
                .get_contract_storage(&self.tok, &ref_fungible_allowance_key(o, s)))
        }
    }
    const ADMIN: AccountId = AccountId([1; 32]);
    const BOB: AccountId = AccountId([2; 32]);
    const SPENDER: AccountId = AccountId([3; 32]);

    fn funded() -> T {
        let mut t = T::new(reference_fungible_template_v2_bytecode());
        t.call(ADMIN, encode_mint_first_calldata(&ADMIN, 1_000));
        t
    }

    #[test]
    fn v1_semantics_mint_once_and_transfer() {
        let mut t = funded();
        assert_eq!(t.bal(&ADMIN), 1_000);
        t.call(ADMIN, encode_mint_first_calldata(&ADMIN, 5)); // second mint no-op
        assert_eq!(t.bal(&ADMIN), 1_000);
        t.call(BOB, encode_mint_first_calldata(&BOB, 5)); // not admin
        assert_eq!(t.bal(&BOB), 0);
        t.call(ADMIN, encode_transfer_calldata(&BOB, 400));
        assert_eq!((t.bal(&ADMIN), t.bal(&BOB)), (600, 400));
        t.call(BOB, encode_transfer_calldata(&ADMIN, 401)); // overdraft no-op
        assert_eq!(t.bal(&BOB), 400);
    }

    #[test]
    fn self_transfer_does_not_inflate_in_v2() {
        let mut t = funded();
        t.call(ADMIN, encode_transfer_calldata(&ADMIN, 1_000));
        assert_eq!(t.bal(&ADMIN), 1_000);
    }

    /// Known v1 bug (bytecode intentionally unchanged): a self-transfer double-credits.
    #[test]
    fn v1_template_self_transfer_double_credits_known_bug() {
        let mut t = T::new(reference_fungible_template_bytecode());
        t.call(ADMIN, encode_mint_first_calldata(&ADMIN, 100));
        t.call(ADMIN, encode_transfer_calldata(&ADMIN, 100));
        assert_eq!(t.bal(&ADMIN), 200);
    }

    #[test]
    fn approve_overwrites_and_allowance_reads() {
        let mut t = funded();
        let r = t.call(ADMIN, encode_approve_calldata(&SPENDER, 300));
        assert_eq!(r.as_deref(), Some(w_u64(1).as_slice()));
        assert_eq!(t.alw(&ADMIN, &SPENDER), 300);
        t.call(ADMIN, encode_approve_calldata(&SPENDER, 50));
        assert_eq!(t.alw(&ADMIN, &SPENDER), 50);
        let r = t
            .call(BOB, encode_allowance_calldata(&ADMIN, &SPENDER))
            .unwrap();
        assert_eq!(u(r.try_into().unwrap()), 50);
        assert_eq!(t.alw(&SPENDER, &ADMIN), 0, "allowance is directional");
        t.call(ADMIN, encode_approve_calldata(&SPENDER, 0));
        assert_eq!(t.alw(&ADMIN, &SPENDER), 0);
    }

    #[test]
    fn transfer_from_decrements_and_rejects_shortfalls() {
        let mut t = funded();
        t.call(ADMIN, encode_approve_calldata(&SPENDER, 300));
        let r = t.call(SPENDER, encode_transfer_from_calldata(&ADMIN, &BOB, 120));
        assert_eq!(r.as_deref(), Some(w_u64(1).as_slice()));
        assert_eq!(
            (t.bal(&ADMIN), t.bal(&BOB), t.alw(&ADMIN, &SPENDER)),
            (880, 120, 180)
        );
        // Insufficient allowance.
        let r = t.call(SPENDER, encode_transfer_from_calldata(&ADMIN, &BOB, 181));
        assert!(r.is_none());
        assert_eq!((t.bal(&ADMIN), t.alw(&ADMIN, &SPENDER)), (880, 180));
        // Someone else's allowance can't be used.
        assert!(t
            .call(BOB, encode_transfer_from_calldata(&ADMIN, &BOB, 1))
            .is_none());
        // Insufficient balance (allowance larger than balance).
        t.call(ADMIN, encode_approve_calldata(&SPENDER, 10_000));
        assert!(t
            .call(SPENDER, encode_transfer_from_calldata(&ADMIN, &BOB, 881))
            .is_none());
        assert_eq!((t.bal(&ADMIN), t.alw(&ADMIN, &SPENDER)), (880, 10_000));
        // Zero amount and from == to are rejected.
        assert!(t
            .call(SPENDER, encode_transfer_from_calldata(&ADMIN, &BOB, 0))
            .is_none());
        assert!(t
            .call(SPENDER, encode_transfer_from_calldata(&ADMIN, &ADMIN, 1))
            .is_none());
        assert_eq!(t.bal(&ADMIN), 880);
        // Exact balance drains to zero.
        assert!(t
            .call(SPENDER, encode_transfer_from_calldata(&ADMIN, &BOB, 880))
            .is_some());
        assert_eq!(
            (t.bal(&ADMIN), t.bal(&BOB), t.alw(&ADMIN, &SPENDER)),
            (0, 1_000, 9_120)
        );
    }

    #[test]
    fn allowance_key_matches_bytecode_derivation() {
        let mut t = funded();
        let o = AccountId([0xa5; 32]);
        let mut s = [0u8; 32];
        s[..16].copy_from_slice(&[0x5a; 16]);
        s[16..].copy_from_slice(&[0x11; 16]);
        let s = AccountId(s);
        t.call(o, encode_approve_calldata(&s, 9));
        assert_eq!(t.alw(&o, &s), 9);
    }

    #[test]
    fn v2_template_passes_protocol_qa_as_token() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
        let r = check_contract_deploy_full(
            &reference_fungible_template_v2_bytecode(),
            Some("token"),
            None,
            &RuleRegistry::new(),
        );
        assert!(matches!(r, QaResult::Allow | QaResult::Unsure), "{r:?}");
        assert_ne!(
            reference_fungible_template_v2_bytecode(),
            reference_fungible_template_bytecode()
        );
    }
}
