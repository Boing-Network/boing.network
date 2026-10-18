//! **Wrapped BOING (wBOING)**: native BOING as a reference-fungible-v2 compatible token, 1:1 in base
//! units (18 decimals). Needs the native-value rules (`CALLVALUE`, `SELFBALANCE`,
//! `NATIVE_TRANSFER`; testnet height 1150).
//!
//! Token-v2 compatible: balances and allowances use the reference fungible slots
//! (`holder ^ REF_FUNGIBLE_BALANCE_XOR`, [`ref_fungible_allowance_key`]) and `transfer` /
//! `approve` / `transfer_from` / `allowance` keep their selectors and calldata, so v8 / v9 / v10
//! custody pools, the custody router and every SDK balance / allowance reader work unchanged.
//!
//! | Selector | Calldata words | Effect |
//! |---|---|---|
//! | `0x01` `transfer` | `to`, `amount` | Move `amount` from `CALLER` (no-op for `0` or `to == CALLER`); faults if short. |
//! | `0x08` `approve` | `spender`, `amount` | Overwrite the allowance; returns `1`. |
//! | `0x09` `transfer_from` | `from`, `to`, `amount` | As token v2: needs allowance and balance, `amount > 0`, `from != to`; returns `1`, else stops with no return data. |
//! | `0x0a` `allowance` | `owner`, `spender` | Returns the allowance. |
//! | `0x20` `deposit` | (payable) | Mints `CALLVALUE` to `CALLER` (faults on `0`); returns the minted amount. |
//! | `0x21` `withdraw` | `amount` | Burns `amount` from `CALLER` and pays `amount` native BOING to `CALLER`. |
//! | `0x22` `withdraw_to` | `amount`, `recipient` | Same, paying `recipient`. |
//! | `0x23` `withdraw_all` | | Burns the caller's whole balance and pays it out (unwrap after a swap in one batch); returns the amount. |
//! | `0x24` `total_supply` | | Returns the supply (always `<= SELFBALANCE`). |
//! | `0x25` `balance_of` | `holder` | Returns the balance. |
//! | `0x26` `decimals` | | Returns `18`. |
//!
//! Burns happen before the payout and `NATIVE_TRANSFER` never runs code at the recipient, so a
//! withdrawal cannot re-enter. Every rejection faults (whole tx reverts, value refunded) except
//! `transfer_from`'s token-v2 "no return data" failure. Logs: deposit `Log2(WBOING_TOPIC_DEPOSIT,
//! caller)`, withdrawals `Log3(WBOING_TOPIC_WITHDRAW, caller, recipient)`, data = amount.

use boing_primitives::AccountId;

use crate::asm::{w_sel, w_u64, Asm, Label};
use crate::bytecode::Opcode;
use crate::reference_token::{amount_word, REF_FUNGIBLE_BALANCE_XOR, SELECTOR_TRANSFER};
use crate::reference_token_v2::{
    REF_FUNGIBLE_ALLOWANCE_XOR, SELECTOR_ALLOWANCE, SELECTOR_APPROVE, SELECTOR_TRANSFER_FROM,
};

pub const WBOING_SELECTOR_DEPOSIT: u8 = 0x20;
pub const WBOING_SELECTOR_WITHDRAW: u8 = 0x21;
pub const WBOING_SELECTOR_WITHDRAW_TO: u8 = 0x22;
pub const WBOING_SELECTOR_WITHDRAW_ALL: u8 = 0x23;
pub const WBOING_SELECTOR_TOTAL_SUPPLY: u8 = 0x24;
pub const WBOING_SELECTOR_BALANCE_OF: u8 = 0x25;
pub const WBOING_SELECTOR_DECIMALS: u8 = 0x26;
/// wBOING decimals (= native BOING base units).
pub const WBOING_DECIMALS: u8 = 18;

pub const WBOING_TOPIC_DEPOSIT: [u8; 32] = *b"BOING_WBOING_DEPOSIT\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
pub const WBOING_TOPIC_WITHDRAW: [u8; 32] = *b"BOING_WBOING_WITHDRAW\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Storage key of the total supply.
#[must_use]
pub fn wboing_total_supply_key() -> [u8; 32] {
    *b"BOING_WBOING_TOTAL_SUPPLY\x00\x00\x00\x00\x00\x00\x00"
}

fn word1(sel: u8) -> Vec<u8> {
    w_sel(sel).to_vec()
}

pub fn encode_wboing_deposit_calldata() -> Vec<u8> {
    word1(WBOING_SELECTOR_DEPOSIT)
}
pub fn encode_wboing_withdraw_calldata(amount: u128) -> Vec<u8> {
    let mut v = word1(WBOING_SELECTOR_WITHDRAW);
    v.extend_from_slice(&amount_word(amount));
    v
}
pub fn encode_wboing_withdraw_to_calldata(amount: u128, recipient: &AccountId) -> Vec<u8> {
    let mut v = word1(WBOING_SELECTOR_WITHDRAW_TO);
    v.extend_from_slice(&amount_word(amount));
    v.extend_from_slice(&recipient.0);
    v
}
pub fn encode_wboing_withdraw_all_calldata() -> Vec<u8> {
    word1(WBOING_SELECTOR_WITHDRAW_ALL)
}
pub fn encode_wboing_total_supply_calldata() -> Vec<u8> {
    word1(WBOING_SELECTOR_TOTAL_SUPPLY)
}
pub fn encode_wboing_balance_of_calldata(holder: &AccountId) -> Vec<u8> {
    let mut v = word1(WBOING_SELECTOR_BALANCE_OF);
    v.extend_from_slice(&holder.0);
    v
}

// Scratch memory (calldata < 128 bytes).
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
const M_SUP: u64 = 896;
const M_LOG: u64 = 928;

/// `mem[dst] = ((owner >> 128) << 128 | spender >> 128) ^ ALLOWANCE_XOR` (token v2 key).
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
    a.push(&REF_FUNGIBLE_ALLOWANCE_XOR).op(Opcode::Xor).mstore(dst);
}

/// `mem[dst] = holder ^ BALANCE_XOR`.
fn balance_key_to(a: &mut Asm, holder_mem: Option<u64>, dst: u64) {
    match holder_mem {
        Some(m) => a.mload(m),
        None => a.op(Opcode::Caller),
    };
    a.push(&REF_FUNGIBLE_BALANCE_XOR).op(Opcode::Xor).mstore(dst);
}

/// Move `mem[M_AMT]` from `from` to `mem[to_mem]` (precondition `from != to`); jumps to `fail` before
/// any write when the balance is short or the credit overflows.
fn move_balance(a: &mut Asm, from_mem: Option<u64>, to_mem: u64, fail: Label) {
    balance_key_to(a, from_mem, M_KC);
    balance_key_to(a, Some(to_mem), M_KT);
    a.mload(M_KC).op(Opcode::SLoad).mstore(M_OLD);
    a.mload(M_OLD).mload(M_AMT).op(Opcode::Lt).jumpi(fail);
    a.mload(M_KT).op(Opcode::SLoad).mstore(M_BALTO);
    a.mload(M_BALTO).mload(M_AMT).op(Opcode::Add).mstore(M_NEW);
    a.mload(M_NEW).mload(M_BALTO).op(Opcode::Lt).jumpi(fail);
    a.mload(M_OLD).mload(M_AMT).op(Opcode::Sub).mload(M_KC).op(Opcode::SStore);
    a.mload(M_NEW).mload(M_KT).op(Opcode::SStore);
}

/// Burn `mem[M_AMT]` (> 0) from `CALLER`, then pay it natively to `mem[M_TO]`, log, return the amount.
fn burn_and_pay(a: &mut Asm, fail: Label) {
    a.mload(M_AMT).op(Opcode::IsZero).jumpi(fail);
    balance_key_to(a, None, M_KC);
    a.mload(M_KC).op(Opcode::SLoad).mstore(M_OLD);
    a.mload(M_OLD).mload(M_AMT).op(Opcode::Lt).jumpi(fail);
    a.sload_key(&wboing_total_supply_key()).mstore(M_SUP);
    a.mload(M_SUP).mload(M_AMT).op(Opcode::Lt).jumpi(fail);
    a.mload(M_OLD).mload(M_AMT).op(Opcode::Sub).mload(M_KC).op(Opcode::SStore);
    a.mload(M_SUP).mload(M_AMT).op(Opcode::Sub).sstore_key(&wboing_total_supply_key());
    // Effects done; pay out (pure ledger move, never runs code at the recipient).
    a.mload(M_AMT).mload(M_TO).op(Opcode::NativeTransfer);
    a.mcopy(M_AMT, M_LOG);
    // Log topics: the deepest pushed word is topic0.
    a.push(&WBOING_TOPIC_WITHDRAW)
        .op(Opcode::Caller)
        .mload(M_TO)
        .push_u(32)
        .push_u(M_LOG)
        .op(Opcode::Log3);
    a.return_word(M_AMT);
}

/// wBOING runtime bytecode (purpose `token`). See the module docs.
#[must_use]
pub fn wrapped_boing_bytecode() -> Vec<u8> {
    let mut a = Asm::new();
    let fail = a.label();
    let stop = a.label();
    let (xfer, approve, xfrom, allowance) = (a.label(), a.label(), a.label(), a.label());
    let (deposit, withdraw, withdraw_to, withdraw_all) = (a.label(), a.label(), a.label(), a.label());
    let (supply, balance_of, decimals) = (a.label(), a.label(), a.label());

    a.mload(0).push(&w_u64(0xff)).op(Opcode::And).mstore(M_SEL);
    // Only `deposit` accepts value: any other selector with value attached faults (value refunded).
    let no_value = a.label();
    a.op(Opcode::CallValue).op(Opcode::IsZero).jumpi(no_value);
    a.mload(M_SEL).push(&w_sel(WBOING_SELECTOR_DEPOSIT)).op(Opcode::Eq).jumpi(deposit);
    a.jump(fail);
    a.mark(no_value);
    for (s, l) in [
        (SELECTOR_TRANSFER, xfer),
        (SELECTOR_APPROVE, approve),
        (SELECTOR_TRANSFER_FROM, xfrom),
        (SELECTOR_ALLOWANCE, allowance),
        (WBOING_SELECTOR_WITHDRAW, withdraw),
        (WBOING_SELECTOR_WITHDRAW_TO, withdraw_to),
        (WBOING_SELECTOR_WITHDRAW_ALL, withdraw_all),
        (WBOING_SELECTOR_TOTAL_SUPPLY, supply),
        (WBOING_SELECTOR_BALANCE_OF, balance_of),
        (WBOING_SELECTOR_DECIMALS, decimals),
    ] {
        a.mload(M_SEL).push(&w_sel(s)).op(Opcode::Eq).jumpi(l);
    }
    // `deposit` without value, or an unknown selector.
    a.jump(fail);

    // deposit(): mint CALLVALUE to CALLER. The value was credited to this contract before the code ran.
    a.mark(deposit);
    a.op(Opcode::CallValue).mstore(M_AMT);
    a.mload(M_AMT).op(Opcode::IsZero).jumpi(fail);
    balance_key_to(&mut a, None, M_KC);
    a.mload(M_KC).op(Opcode::SLoad).mload(M_AMT).op(Opcode::Add).mload(M_KC).op(Opcode::SStore);
    a.sload_key(&wboing_total_supply_key()).mload(M_AMT).op(Opcode::Add).mstore(M_SUP);
    // Invariant: supply never exceeds the native balance backing it.
    a.mload(M_SUP).op(Opcode::SelfBalance).op(Opcode::Gt).jumpi(fail);
    a.mload(M_SUP).sstore_key(&wboing_total_supply_key());
    a.mcopy(M_AMT, M_LOG);
    a.push(&WBOING_TOPIC_DEPOSIT)
        .op(Opcode::Caller)
        .push_u(32)
        .push_u(M_LOG)
        .op(Opcode::Log2);
    a.return_word(M_AMT);

    // withdraw(amount) / withdraw_to(amount, recipient) / withdraw_all().
    a.mark(withdraw);
    a.mcopy(32, M_AMT);
    a.op(Opcode::Caller).mstore(M_TO);
    burn_and_pay(&mut a, fail);
    a.mark(withdraw_to);
    a.mcopy(32, M_AMT).mcopy(64, M_TO);
    a.mload(M_TO).op(Opcode::IsZero).jumpi(fail);
    burn_and_pay(&mut a, fail);
    a.mark(withdraw_all);
    balance_key_to(&mut a, None, M_KC);
    a.mload(M_KC).op(Opcode::SLoad).mstore(M_AMT);
    a.op(Opcode::Caller).mstore(M_TO);
    burn_and_pay(&mut a, fail);

    // transfer(to, amount): token v2 semantics, but a short balance faults.
    a.mark(xfer);
    a.mcopy(32, M_TO).mcopy(64, M_AMT);
    a.mload(M_AMT).op(Opcode::IsZero).jumpi(stop);
    a.mload(M_TO).op(Opcode::Caller).op(Opcode::Eq).jumpi(stop);
    move_balance(&mut a, None, M_TO, fail);
    a.op(Opcode::Stop);

    // approve(spender, amount).
    a.mark(approve);
    a.mcopy(32, M_TO).mcopy(64, M_AMT);
    allowance_key_to(&mut a, None, Some(M_TO), M_AKEY);
    a.mload(M_AMT).mload(M_AKEY).op(Opcode::SStore);
    a.push(&w_u64(1)).mstore(M_RET).return_word(M_RET);

    // transfer_from(from, to, amount): exactly token v2 (failure = stop with no return data).
    a.mark(xfrom);
    a.mcopy(32, M_FROM).mcopy(64, M_TO).mcopy(96, M_AMT);
    a.mload(M_AMT).op(Opcode::IsZero).jumpi(stop);
    a.mload(M_FROM).mload(M_TO).op(Opcode::Eq).jumpi(stop);
    allowance_key_to(&mut a, Some(M_FROM), None, M_AKEY);
    a.mload(M_AKEY).op(Opcode::SLoad).mstore(M_ALW);
    a.mload(M_ALW).mload(M_AMT).op(Opcode::Lt).jumpi(stop);
    move_balance(&mut a, Some(M_FROM), M_TO, stop);
    a.mload(M_ALW).mload(M_AMT).op(Opcode::Sub).mload(M_AKEY).op(Opcode::SStore);
    a.push(&w_u64(1)).mstore(M_RET).return_word(M_RET);

    // allowance(owner, spender).
    a.mark(allowance);
    a.mcopy(32, M_FROM).mcopy(64, M_TO);
    allowance_key_to(&mut a, Some(M_FROM), Some(M_TO), M_AKEY);
    a.mload(M_AKEY).op(Opcode::SLoad).mstore(M_RET).return_word(M_RET);

    // total_supply() / balance_of(holder) / decimals().
    a.mark(supply);
    a.sload_key(&wboing_total_supply_key()).mstore(M_RET).return_word(M_RET);
    a.mark(balance_of);
    balance_key_to(&mut a, Some(32), M_KC);
    a.mload(M_KC).op(Opcode::SLoad).mstore(M_RET).return_word(M_RET);
    a.mark(decimals);
    a.push_u(u64::from(WBOING_DECIMALS)).mstore(M_RET).return_word(M_RET);

    a.mark(stop);
    a.op(Opcode::Stop);
    a.mark(fail);
    a.fault();
    a.finish()
}
