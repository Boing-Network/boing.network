//! **Create-and-seed factory** — CREATE2 a custody CP pool, `set_tokens`, optional directory
//! `register_pair`, in one contract call.
//!
//! Pool **runtime** bytecode is passed in calldata (not embedded), so the factory stays small under
//! the 32 KiB QA cap while still deploying the same v8–v11 templates Finance already ships.
//!
//! Seeding liquidity stays with the **tx sender** as `CALLER` (custody pools pull via
//! `transfer_from(CALLER, …)` and mint LP to `CALLER`). dApps compose one Express / protocol
//! **Batch**: approve predicted pool → `create_and_seed` → `add_liquidity` (plus optional wrap).
//! That is the one-approval create-and-seed product; see `docs/NATIVE-DEX-CREATE-AND-SEED.md`.

use boing_primitives::AccountId;

use crate::asm::{w_sel, Asm};
use crate::bytecode::Opcode;
use crate::native_amm::SELECTOR_SET_TOKENS;
use crate::native_dex_factory::SELECTOR_REGISTER_PAIR;
use crate::reference_token::amount_word;

/// `create_and_seed(salt, token_a, token_b, directory, bytecode_len, bytecode…)` — selector **`0xC0`**.
pub const SELECTOR_CREATE_AND_SEED: u8 = 0xC0;

/// CREATE2 salt for deploying [`native_dex_create_and_seed_bytecode`].
pub const NATIVE_DEX_CREATE_AND_SEED_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_NATIVEDEX_CREATESEED_V1\x00\x00\x00";

/// `Log3` topic0 after a successful create (topic1 = token_a, topic2 = token_b, data = pool).
pub const NATIVE_DEX_CREATE_AND_SEED_TOPIC: [u8; 32] =
    *b"BOING_NATIVE_DEX_CREATE_SEED1\x00\x00\x00";

/// Absolute max pool init/runtime bytes accepted in one create (matches default QA bytecode cap).
pub const NATIVE_DEX_CREATE_AND_SEED_MAX_POOL_BYTES: u64 = 32 * 1024;

// Calldata layout (32-byte words, then raw bytecode).
const SALT: u64 = 32;
const TOKEN_A: u64 = 64;
const TOKEN_B: u64 = 96;
const DIRECTORY: u64 = 128;
const BYTECODE_LEN: u64 = 160;
const BYTECODE_OFF: u64 = 192;

// Scratch (well above any pool template ≤ 32 KiB).
const POOL: u64 = 40_000;
const CB: u64 = 40_064;
const RET: u64 = 40_256;
const LOG_DATA: u64 = 40_288;

/// Encode [`SELECTOR_CREATE_AND_SEED`] calldata (fixed header + pool runtime bytes).
#[must_use]
pub fn encode_create_and_seed_calldata(
    salt: &[u8; 32],
    token_a: &AccountId,
    token_b: &AccountId,
    directory: Option<&AccountId>,
    pool_bytecode: &[u8],
) -> Vec<u8> {
    assert!(
        !pool_bytecode.is_empty()
            && pool_bytecode.len() as u64 <= NATIVE_DEX_CREATE_AND_SEED_MAX_POOL_BYTES,
        "pool bytecode length out of range"
    );
    let mut v = w_sel(SELECTOR_CREATE_AND_SEED).to_vec();
    v.extend_from_slice(salt);
    v.extend_from_slice(&token_a.0);
    v.extend_from_slice(&token_b.0);
    v.extend_from_slice(&directory.map(|d| d.0).unwrap_or([0u8; 32]));
    v.extend_from_slice(&amount_word(pool_bytecode.len() as u128));
    v.extend_from_slice(pool_bytecode);
    v
}

/// Canonical create-and-seed factory bytecode (v1).
#[must_use]
pub fn native_dex_create_and_seed_bytecode() -> Vec<u8> {
    let mut a = Asm::new();
    let fail = a.label();
    let skip_reg = a.label();
    let after_reg = a.label();

    // Selector must be create_and_seed.
    a.mload(0)
        .push(&w_sel(SELECTOR_CREATE_AND_SEED))
        .op(Opcode::Eq)
        .op(Opcode::IsZero)
        .jumpi(fail);

    // token_a / token_b non-zero and distinct.
    a.mload(TOKEN_A).op(Opcode::IsZero).jumpi(fail);
    a.mload(TOKEN_B).op(Opcode::IsZero).jumpi(fail);
    a.mload(TOKEN_A).mload(TOKEN_B).op(Opcode::Eq).jumpi(fail);

    // 0 < bytecode_len <= MAX.
    a.mload(BYTECODE_LEN).op(Opcode::IsZero).jumpi(fail);
    a.mload(BYTECODE_LEN)
        .push_u(NATIVE_DEX_CREATE_AND_SEED_MAX_POOL_BYTES)
        .op(Opcode::Gt)
        .jumpi(fail);

    // CREATE2: stack offset, size, salt (top) → pops salt, size, offset; pushes new address.
    a.push_u(BYTECODE_OFF)
        .mload(BYTECODE_LEN)
        .mload(SALT)
        .op(Opcode::Create2);
    a.mstore(POOL);

    // set_tokens(token_a, token_b) on the new pool.
    a.push(&w_sel(SELECTOR_SET_TOKENS)).mstore(CB);
    a.mcopy(TOKEN_A, CB + 32).mcopy(TOKEN_B, CB + 64);
    a.push_u(0).mstore(RET);
    a.mload(POOL)
        .push_u(CB)
        .push_u(96)
        .push_u(RET)
        .push_u(0)
        .op(Opcode::Call);
    a.op(Opcode::IsZero).jumpi(fail);

    // Optional directory.register_pair(token_a, token_b, pool).
    a.mload(DIRECTORY).op(Opcode::IsZero).jumpi(skip_reg);
    a.push(&w_sel(SELECTOR_REGISTER_PAIR)).mstore(CB);
    a.mcopy(TOKEN_A, CB + 32)
        .mcopy(TOKEN_B, CB + 64)
        .mcopy(POOL, CB + 96);
    a.push_u(0).mstore(RET);
    a.mload(DIRECTORY)
        .push_u(CB)
        .push_u(128)
        .push_u(RET)
        .push_u(0)
        .op(Opcode::Call);
    a.op(Opcode::IsZero).jumpi(fail);
    a.jump(after_reg);
    a.mark(skip_reg);
    a.mark(after_reg);

    // Log3: topic0 create, topic1 token_a, topic2 token_b, data = pool.
    a.mcopy(POOL, LOG_DATA);
    a.push(&NATIVE_DEX_CREATE_AND_SEED_TOPIC)
        .mload(TOKEN_A)
        .mload(TOKEN_B)
        .push_u(32)
        .push_u(LOG_DATA)
        .op(Opcode::Log3);

    // Return pool address word.
    a.return_word(POOL);

    a.mark(fail);
    a.fault();
    a.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::{Interpreter, VmExecutionContext};
    use crate::native_amm::{lp_balance_storage_key, reserve_a_key, reserve_b_key};
    use crate::native_amm_v8::{
        constant_product_pool_bytecode_v8, encode_v8_add_liquidity_calldata,
    };
    use crate::native_dex_factory::{native_dex_factory_bytecode, native_dex_factory_count_key};
    use crate::reference_token::encode_mint_first_calldata;
    use crate::reference_token_v2::{
        encode_approve_calldata, reference_fungible_template_v2_bytecode,
    };
    use boing_primitives::{create2_contract_address, Account, AccountId};
    use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
    use boing_state::StateStore;

    const USER: AccountId = AccountId([0x11; 32]);
    const FACTORY: AccountId = AccountId([0xf1; 32]);
    const DIRECTORY: AccountId = AccountId([0xd1; 32]);
    const TA: AccountId = AccountId([0xa1; 32]);
    const TB: AccountId = AccountId([0xb1; 32]);

    fn ctx() -> VmExecutionContext {
        VmExecutionContext {
            block_height: 1,
            block_timestamp: 1,
            native_value: false,
            vm_hardening: false,
            blake3_opcode: false,
            jump_strict: true,
            qa_canonical: false,
        }
    }

    fn call(state: &mut StateStore, who: AccountId, to: AccountId, cd: Vec<u8>) {
        let code = state.get_contract_code(&to).unwrap().clone();
        let mut it = Interpreter::new(code, 40_000_000);
        it.run_with_qa(who, to, &cd, state, &RuleRegistry::new(), ctx())
            .unwrap_or_else(|e| panic!("call failed: {e:?}"));
    }

    fn u128_word(w: [u8; 32]) -> u128 {
        assert!(w[..16].iter().all(|b| *b == 0));
        u128::from_be_bytes(w[16..].try_into().unwrap())
    }

    #[test]
    fn factory_bytecode_qa_allow_and_small() {
        let code = native_dex_create_and_seed_bytecode();
        assert!(
            code.len() < 4_000,
            "factory must stay tiny (got {})",
            code.len()
        );
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &RuleRegistry::new());
        assert!(matches!(r, QaResult::Allow | QaResult::Unsure), "{r:?}");
    }

    #[test]
    fn create_and_seed_deploys_binds_registers_then_user_seeds() {
        let mut state = StateStore::new();
        for (id, code) in [
            (FACTORY, native_dex_create_and_seed_bytecode()),
            (DIRECTORY, native_dex_factory_bytecode()),
            (TA, reference_fungible_template_v2_bytecode()),
            (TB, reference_fungible_template_v2_bytecode()),
        ] {
            state.insert(Account {
                id,
                state: Default::default(),
            });
            state.set_contract_code(id, code);
        }
        state.insert(Account {
            id: USER,
            state: Default::default(),
        });

        call(
            &mut state,
            USER,
            TA,
            encode_mint_first_calldata(&USER, 1_000_000),
        );
        call(
            &mut state,
            USER,
            TB,
            encode_mint_first_calldata(&USER, 1_000_000),
        );

        let pool_code = constant_product_pool_bytecode_v8();
        let salt = [7u8; 32];
        let predicted = create2_contract_address(&FACTORY, &salt, &pool_code);

        let cd = encode_create_and_seed_calldata(&salt, &TA, &TB, Some(&DIRECTORY), &pool_code);
        call(&mut state, USER, FACTORY, cd);

        let installed = state.get_contract_code(&predicted).expect("pool code");
        assert_eq!(installed.as_slice(), pool_code.as_slice());

        let cnt = state.get_contract_storage(&DIRECTORY, &native_dex_factory_count_key());
        assert_eq!(u128_word(cnt), 1);

        call(
            &mut state,
            USER,
            TA,
            encode_approve_calldata(&predicted, 1_000),
        );
        call(
            &mut state,
            USER,
            TB,
            encode_approve_calldata(&predicted, 2_000),
        );
        call(
            &mut state,
            USER,
            predicted,
            encode_v8_add_liquidity_calldata(1_000, 2_000, 1, 0),
        );

        assert_eq!(
            u128_word(state.get_contract_storage(&predicted, &reserve_a_key())),
            1_000
        );
        assert_eq!(
            u128_word(state.get_contract_storage(&predicted, &reserve_b_key())),
            2_000
        );
        assert_eq!(
            u128_word(state.get_contract_storage(&predicted, &lp_balance_storage_key(&USER.0))),
            1_000
        );
    }

    #[test]
    fn create_and_seed_skips_register_when_directory_zero() {
        let mut state = StateStore::new();
        state.insert(Account {
            id: FACTORY,
            state: Default::default(),
        });
        state.set_contract_code(FACTORY, native_dex_create_and_seed_bytecode());
        for id in [TA, TB] {
            state.insert(Account {
                id,
                state: Default::default(),
            });
            state.set_contract_code(id, reference_fungible_template_v2_bytecode());
        }

        let pool_code = constant_product_pool_bytecode_v8();
        let salt = [9u8; 32];
        let predicted = create2_contract_address(&FACTORY, &salt, &pool_code);
        let cd = encode_create_and_seed_calldata(&salt, &TA, &TB, None, &pool_code);
        call(&mut state, USER, FACTORY, cd);
        assert_eq!(
            state.get_contract_code(&predicted).unwrap().as_slice(),
            pool_code.as_slice()
        );
    }
}
