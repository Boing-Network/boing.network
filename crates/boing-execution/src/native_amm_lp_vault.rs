//! **Native AMM LP vault:** one-time **`configure`**, then **`deposit_add`** — [`Opcode::Call`] pool
//! **`add_liquidity`**, read **32-byte** return data (LP minted), enforce **`min_lp`**, then
//! [`Opcode::Call`] [`crate::native_lp_share_token::lp_share_token_bytecode`] **`mint`** to the
//! transaction caller.
//!
//! **Atomicity note:** The Boing VM does not roll back nested calls. If the pool call succeeds but
//! the vault aborts (e.g. LP return word zero or `LP < min_lp`), pool reserves and pool LP credited
//! to this vault **remain updated** while share **`mint`** is skipped. Callers should align
//! **`min_lp`** with the inner **`add_liquidity`** `min_liquidity` word and keep slippage checks
//! consistent.
//!
//! See `docs/NATIVE-AMM-LP-VAULT.md`.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::native_lp_share_token::SELECTOR_LP_SHARE_MINT;
use crate::reference_token::{amount_word, selector_word};

/// `configure(pool, share_token)` — **96** bytes; succeeds only while configured slot is zero.
pub const SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE: u8 = 0xC0;
/// `deposit_add(inner_add_liquidity_128, min_lp)` — **192** bytes.
pub const SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD: u8 = 0xC1;

/// Storage: **1** after successful [`SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE`] (`k[31] == 0xd1`).
pub const NATIVE_AMM_LP_VAULT_KEY_CONFIGURED: [u8; 32] = {
    let mut k = [0u8; 32];
    k[31] = 0xd1;
    k
};
/// Storage: configured pool [`AccountId`] (`k[31] == 0xd2`).
pub const NATIVE_AMM_LP_VAULT_KEY_POOL: [u8; 32] = {
    let mut k = [0u8; 32];
    k[31] = 0xd2;
    k
};
/// Storage: LP share token [`AccountId`] (`k[31] == 0xd3`).
pub const NATIVE_AMM_LP_VAULT_KEY_SHARE_TOKEN: [u8; 32] = {
    let mut k = [0u8; 32];
    k[31] = 0xd3;
    k
};

/// CREATE2 salt for [`native_amm_lp_vault_bytecode`].
pub const NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_AMM_LP_VAULT_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

const MEM_SCRATCH_SEL: u64 = 384;
const MEM_POOL_TMP: u64 = 512;
const MEM_LP_SAVED: u64 = 576;
const MEM_RET_LP: u64 = 640;
const MEM_MINT_BASE: u64 = 704;

fn push32(out: &mut Vec<u8>, w: &[u8; 32]) {
    out.push(Opcode::Push32 as u8);
    out.extend_from_slice(w);
}

fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

fn patch_push32_dest(code: &mut [u8], push32_opcode_at: usize, dest: usize) {
    code[push32_opcode_at + 1..push32_opcode_at + 33].copy_from_slice(&word_u64(dest as u64));
}

fn mask_low_byte() -> [u8; 32] {
    let mut m = [0u8; 32];
    m[31] = 0xff;
    m
}

fn word_one() -> [u8; 32] {
    let mut w = [0u8; 32];
    w[31] = 1;
    w
}

fn append_copy_calldata_slice(out: &mut Vec<u8>, src_base: u64, dst_base: u64, num_words: u64) {
    for i in 0u64..num_words {
        let src = src_base + i * 32;
        let dst = dst_base + i * 32;
        push32(out, &word_u64(src));
        out.push(Opcode::MLoad as u8);
        push32(out, &word_u64(dst));
        out.push(Opcode::MStore as u8);
    }
}

/// Encode **96** bytes: [`SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE`] + `pool` + `share_token`.
#[must_use]
pub fn encode_native_amm_lp_vault_configure_calldata(pool: &AccountId, share_token: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE).to_vec();
    v.extend_from_slice(&pool.0);
    v.extend_from_slice(&share_token.0);
    debug_assert_eq!(v.len(), 96);
    v
}

/// Encode **192** bytes: [`SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD`] + **128-byte** pool
/// `add_liquidity` calldata + `min_lp` word.
#[must_use]
pub fn encode_native_amm_lp_vault_deposit_add_calldata(inner_add_liquidity_128: &[u8], min_lp: u128) -> Vec<u8> {
    assert_eq!(
        inner_add_liquidity_128.len(),
        128,
        "inner calldata must be 128 bytes (native CP add_liquidity layout)"
    );
    let mut v = selector_word(SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD).to_vec();
    v.extend_from_slice(inner_add_liquidity_128);
    v.extend_from_slice(&amount_word(min_lp));
    debug_assert_eq!(v.len(), 192);
    v
}

/// LP vault bytecode. CREATE2: [`NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1`].
#[must_use]
pub fn native_amm_lp_vault_bytecode() -> Vec<u8> {
    let mut c: Vec<u8> = Vec::new();

    // scratch selector low byte
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &mask_low_byte());
    c.push(Opcode::And as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MStore as u8);

    // -- configure (0xC0)
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_cfg = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &NATIVE_AMM_LP_VAULT_KEY_CONFIGURED);
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_cfg_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);

    let off_cfg_ok = c.len();
    patch_push32_dest(&mut c, fix_cfg_ok, off_cfg_ok);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &NATIVE_AMM_LP_VAULT_KEY_POOL);
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &NATIVE_AMM_LP_VAULT_KEY_SHARE_TOKEN);
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_one());
    push32(&mut c, &NATIVE_AMM_LP_VAULT_KEY_CONFIGURED);
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let off_after_cfg = c.len();
    patch_push32_dest(&mut c, fix_skip_cfg, off_after_cfg);

    // -- deposit_add (0xC1)
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_dep = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &NATIVE_AMM_LP_VAULT_KEY_CONFIGURED);
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    c.push(Opcode::IsZero as u8);
    let fix_dep_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);

    let off_dep_ok = c.len();
    patch_push32_dest(&mut c, fix_dep_ok, off_dep_ok);

    append_copy_calldata_slice(&mut c, 32, 0, 4);

    push32(&mut c, &NATIVE_AMM_LP_VAULT_KEY_POOL);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(128));
    push32(&mut c, &word_u64(MEM_RET_LP));
    push32(&mut c, &word_u64(32));
    c.push(Opcode::Call as u8);

    // drop Call success flag
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_RET_LP));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_LP_SAVED));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_LP_SAVED));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_abort_lp0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_LP_SAVED));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(160));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_abort_slip = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &selector_word(SELECTOR_LP_SHARE_MINT));
    push32(&mut c, &word_u64(MEM_MINT_BASE));
    c.push(Opcode::MStore as u8);

    c.push(Opcode::Caller as u8);
    push32(&mut c, &word_u64(MEM_MINT_BASE + 32));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_LP_SAVED));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_MINT_BASE + 64));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &NATIVE_AMM_LP_VAULT_KEY_SHARE_TOKEN);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_MINT_BASE));
    push32(&mut c, &word_u64(96));
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(0));
    c.push(Opcode::Call as u8);

    c.push(Opcode::Stop as u8);

    let off_abort = c.len();
    patch_push32_dest(&mut c, fix_abort_lp0, off_abort);
    patch_push32_dest(&mut c, fix_abort_slip, off_abort);
    c.push(Opcode::Stop as u8);

    let off_after_dep = c.len();
    patch_push32_dest(&mut c, fix_skip_dep, off_after_dep);

    c.push(Opcode::Stop as u8);

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::Account;
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;
    use crate::native_amm::{
        constant_product_pool_bytecode, encode_add_liquidity_calldata, lp_balance_storage_key,
        reserve_a_key, reserve_b_key, total_lp_supply_key,
    };
    use crate::native_lp_share_token::{
        encode_lp_share_set_minter_once_calldata, lp_share_token_bytecode, LP_SHARE_BALANCE_XOR,
    };

    fn tail_u128(w: &[u8; 32]) -> u128 {
        u128::from_be_bytes(w[16..32].try_into().unwrap())
    }

    fn share_bal_key(holder: &AccountId) -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = holder.0[i] ^ LP_SHARE_BALANCE_XOR[i];
        }
        k
    }

    #[test]
    fn vault_configure_deposit_mints_shares_to_caller() {
        let alice = AccountId([0xa1; 32]);
        let pool = AccountId([0xb2; 32]);
        let vault = AccountId([0xc3; 32]);
        let share = AccountId([0xd4; 32]);

        let pool_code = constant_product_pool_bytecode();
        let vault_code = native_amm_lp_vault_bytecode();
        let share_code = lp_share_token_bytecode();

        let mut state = StateStore::new();
        for id in [alice, pool, vault, share] {
            state.insert(Account {
                id,
                state: Default::default(),
            });
        }
        state.set_contract_code(pool, pool_code.clone());
        state.set_contract_code(vault, vault_code.clone());
        state.set_contract_code(share, share_code.clone());

        let mut it0 = Interpreter::new(vault_code.clone(), 20_000_000);
        it0
            .run(
                alice,
                vault,
                &encode_native_amm_lp_vault_configure_calldata(&pool, &share),
                &mut state,
            )
            .unwrap();

        let mut it1 = Interpreter::new(share_code.clone(), 20_000_000);
        it1
            .run(alice, share, &encode_lp_share_set_minter_once_calldata(&vault), &mut state)
            .unwrap();

        let amount_a = 1_000u128;
        let amount_b = 2_000u128;
        let inner = encode_add_liquidity_calldata(amount_a, amount_b, 0);
        let mut it2 = Interpreter::new(vault_code, 30_000_000);
        it2
            .run(
                alice,
                vault,
                &encode_native_amm_lp_vault_deposit_add_calldata(&inner, 0),
                &mut state,
            )
            .unwrap();

        let lp_minted = tail_u128(&state.get_contract_storage(&pool, &lp_balance_storage_key(&vault.0)));
        assert!(lp_minted > 0);
        assert_eq!(
            tail_u128(&state.get_contract_storage(&share, &share_bal_key(&alice))),
            lp_minted
        );
        assert_eq!(
            tail_u128(&state.get_contract_storage(&pool, &total_lp_supply_key())),
            lp_minted
        );
        assert_eq!(
            tail_u128(&state.get_contract_storage(&pool, &reserve_a_key())),
            amount_a
        );
        assert_eq!(
            tail_u128(&state.get_contract_storage(&pool, &reserve_b_key())),
            amount_b
        );
    }

    #[test]
    fn vault_configure_twice_is_noop_second_time() {
        let alice = AccountId([0xe1; 32]);
        let pool_a = AccountId([0xe2; 32]);
        let pool_b = AccountId([0xe3; 32]);
        let share_a = AccountId([0xe4; 32]);
        let share_b = AccountId([0xe5; 32]);
        let vault = AccountId([0xe6; 32]);

        let mut state = StateStore::new();
        for id in [alice, pool_a, pool_b, share_a, share_b, vault] {
            state.insert(Account {
                id,
                state: Default::default(),
            });
        }
        state.set_contract_code(vault, native_amm_lp_vault_bytecode());

        let code = native_amm_lp_vault_bytecode();
        Interpreter::new(code.clone(), 5_000_000)
            .run(
                alice,
                vault,
                &encode_native_amm_lp_vault_configure_calldata(&pool_a, &share_a),
                &mut state,
            )
            .unwrap();
        Interpreter::new(code, 5_000_000)
            .run(
                alice,
                vault,
                &encode_native_amm_lp_vault_configure_calldata(&pool_b, &share_b),
                &mut state,
            )
            .unwrap();

        assert_eq!(state.get_contract_storage(&vault, &NATIVE_AMM_LP_VAULT_KEY_POOL), pool_a.0);
        assert_eq!(
            state.get_contract_storage(&vault, &NATIVE_AMM_LP_VAULT_KEY_SHARE_TOKEN),
            share_a.0
        );
    }

    #[test]
    fn native_amm_lp_vault_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = native_amm_lp_vault_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for LP vault bytecode, got {r:?}"
        );
    }
}
