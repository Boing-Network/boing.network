//! **Ledger-router** bytecode: forwards native CP pool calldata via [`Opcode::Call`].
//!
//! - **v1** ([`native_dex_ledger_router_bytecode`]): **192-byte** outer calldata, **128-byte** inner (swap / add / remove).
//! - **v2** ([`native_dex_ledger_router_bytecode_v2`]): **224-byte** outer calldata, **160-byte** inner (e.g. v5 **`swap_to`**).
//! - **v3** ([`native_dex_ledger_router_bytecode_v3`]): **256-byte** outer calldata, **192-byte** inner (v5 **`remove_liquidity_to`**).
//!
//! # Caller semantics
//!
//! The callee pool sees **`Caller` = this router contract**, not the transaction signer. Use **only** with
//! pools where that is safe — typically **v1** ledger-only pools with **v1** router. **Do not** use **v1** router with **v2 / v4**
//! pool token hooks when reference-token hooks pay **`Caller`**, or payouts accrue to the router.
//!
//! **v5** pools: **v2** router for **`swap_to`**; **v3** router for **`remove_liquidity_to`**; or call the pool directly.
//!
//! See `docs/NATIVE-DEX-LEDGER-ROUTER.md`.

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// Forwarding entry (v1): **192-byte** calldata — word0 = this selector, word1 = pool `AccountId`, words 2–4 = inner pool calldata (**128** bytes).
pub const SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL: u8 = 0xE0;

/// Forwarding entry (v2): **224-byte** calldata — word0 = **`0xE1`**, word1 = pool, words 2–6 = inner (**160** bytes).
pub const SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V2: u8 = 0xE1;

/// Forwarding entry (v3): **256-byte** calldata — word0 = **`0xE2`**, word1 = pool, words 2–7 = inner (**192** bytes).
pub const SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V3: u8 = 0xE2;

/// CREATE2 salt for [`native_dex_ledger_router_bytecode`].
pub const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_NATIVEDEX_LROUTER_V1\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for [`native_dex_ledger_router_bytecode_v2`].
pub const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2: [u8; 32] =
    *b"BOING_NATIVEDEX_LROUTER_V2\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for [`native_dex_ledger_router_bytecode_v3`].
pub const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3: [u8; 32] =
    *b"BOING_NATIVEDEX_LROUTER_V3\x00\x00\x00\x00\x00\x00";

const MEM_POOL_TMP: u64 = 512;

fn push32(code: &mut Vec<u8>, w: &[u8; 32]) {
    code.push(Opcode::Push32 as u8);
    code.extend_from_slice(w);
}

fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

/// Encode **192** bytes: router selector + `pool` + `inner` (**128** bytes, e.g. [`crate::encode_swap_calldata`]).
#[must_use]
pub fn encode_ledger_router_forward_calldata(pool: &[u8; 32], inner_128: &[u8]) -> Vec<u8> {
    assert_eq!(
        inner_128.len(),
        128,
        "inner calldata must be 128 bytes (native CP pool swap/add/remove layout)"
    );
    let mut v = selector_word(SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL).to_vec();
    v.extend_from_slice(pool);
    v.extend_from_slice(inner_128);
    v
}

/// Encode **224** bytes: router selector **`0xE1`** + `pool` + `inner` (**160** bytes, e.g. [`crate::encode_swap_to_calldata`]).
#[must_use]
pub fn encode_ledger_router_forward_calldata_v2(pool: &[u8; 32], inner_160: &[u8]) -> Vec<u8> {
    assert_eq!(
        inner_160.len(),
        160,
        "inner calldata must be 160 bytes (e.g. native CP pool v5 swap_to layout)"
    );
    let mut v = selector_word(SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V2).to_vec();
    v.extend_from_slice(pool);
    v.extend_from_slice(inner_160);
    v
}

/// Encode **256** bytes: router selector **`0xE2`** + `pool` + `inner` (**192** bytes, e.g. [`crate::encode_remove_liquidity_to_calldata`]).
#[must_use]
pub fn encode_ledger_router_forward_calldata_v3(pool: &[u8; 32], inner_192: &[u8]) -> Vec<u8> {
    assert_eq!(
        inner_192.len(),
        192,
        "inner calldata must be 192 bytes (e.g. native CP pool v5 remove_liquidity_to layout)"
    );
    let mut v = selector_word(SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V3).to_vec();
    v.extend_from_slice(pool);
    v.extend_from_slice(inner_192);
    v
}

/// Assembled ledger router (v1). CREATE2: [`NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1`].
#[must_use]
pub fn native_dex_ledger_router_bytecode() -> Vec<u8> {
    let mut c: Vec<u8> = Vec::new();

    // Dispatch: only 0xE0; unknown → STOP
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL));
    c.push(Opcode::Eq as u8);
    let fix_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);

    let off_body = c.len();
    patch_push32_dest(&mut c, fix_ok, off_body);

    // pool word at calldata 32 → MEM_POOL_TMP
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MStore as u8);

    // Copy inner 128 bytes from offset 64 to 0..128
    for i in 0u64..4 {
        let src = 64 + i * 32;
        let dst = i * 32;
        push32(&mut c, &word_u64(src));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(dst));
        c.push(Opcode::MStore as u8);
    }

    // CALL pops **ret_size** first (stack top). Order bottom→top: target, args_off, args_size, ret_off, ret_size.
    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(128));
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(0));
    c.push(Opcode::Call as u8);

    c.push(Opcode::Stop as u8);

    c
}

/// Assembled ledger router (v2). CREATE2: [`NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2`].
#[must_use]
pub fn native_dex_ledger_router_bytecode_v2() -> Vec<u8> {
    let mut c: Vec<u8> = Vec::new();

    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V2));
    c.push(Opcode::Eq as u8);
    let fix_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);

    let off_body = c.len();
    patch_push32_dest(&mut c, fix_ok, off_body);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MStore as u8);

    for i in 0u64..5 {
        let src = 64 + i * 32;
        let dst = i * 32;
        push32(&mut c, &word_u64(src));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(dst));
        c.push(Opcode::MStore as u8);
    }

    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(160));
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(0));
    c.push(Opcode::Call as u8);

    c.push(Opcode::Stop as u8);

    c
}

/// Assembled ledger router (v3). CREATE2: [`NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3`].
#[must_use]
pub fn native_dex_ledger_router_bytecode_v3() -> Vec<u8> {
    let mut c: Vec<u8> = Vec::new();

    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V3));
    c.push(Opcode::Eq as u8);
    let fix_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);

    let off_body = c.len();
    patch_push32_dest(&mut c, fix_ok, off_body);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MStore as u8);

    for i in 0u64..6 {
        let src = 64 + i * 32;
        let dst = i * 32;
        push32(&mut c, &word_u64(src));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(dst));
        c.push(Opcode::MStore as u8);
    }

    push32(&mut c, &word_u64(MEM_POOL_TMP));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(192));
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(0));
    c.push(Opcode::Call as u8);

    c.push(Opcode::Stop as u8);

    c
}

fn patch_push32_dest(code: &mut [u8], push32_opcode_at: usize, dest: usize) {
    code[push32_opcode_at + 1..push32_opcode_at + 33].copy_from_slice(&word_u64(dest as u64));
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{Account, AccountId};
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;
    use crate::native_amm::{
        constant_product_amount_out_after_fee, constant_product_pool_bytecode,
        constant_product_pool_bytecode_v5, encode_add_liquidity_calldata, encode_remove_liquidity_to_calldata,
        encode_set_tokens_calldata, encode_swap_calldata, encode_swap_to_calldata, total_lp_supply_key,
        reserve_a_key, reserve_b_key,
    };
    use crate::reference_token::{encode_mint_first_calldata, reference_fungible_template_bytecode, REF_FUNGIBLE_BALANCE_XOR};
    use crate::reference_token::amount_word;

    #[test]
    fn ledger_router_forwards_swap_to_v1_pool() {
        let pool_code = constant_product_pool_bytecode();
        let pool = AccountId([0xce; 32]);
        let router_code = native_dex_ledger_router_bytecode();
        let router = AccountId([0xde; 32]);
        let sender = AccountId([0xab; 32]);

        let mut state = StateStore::new();
        state.insert(Account {
            id: pool,
            state: Default::default(),
        });
        state.insert(Account {
            id: router,
            state: Default::default(),
        });
        state.set_contract_code(pool, pool_code);
        state.set_contract_code(router, router_code);

        let ra = 1_000u64;
        let rb = 2_000u64;
        let dx = 100u64;
        state.merge_contract_storage(pool, reserve_a_key(), amount_word(u128::from(ra)));
        state.merge_contract_storage(pool, reserve_b_key(), amount_word(u128::from(rb)));

        let dy = constant_product_amount_out_after_fee(ra, rb, dx);
        let inner = encode_swap_calldata(0, u128::from(dx), u128::from(dy));
        let outer = encode_ledger_router_forward_calldata(&pool.0, &inner);

        let mut it = Interpreter::new(native_dex_ledger_router_bytecode(), 10_000_000);
        it.run(sender, router, &outer, &mut state).unwrap();

        let exp_rb = rb - dy;
        let exp_ra = ra + dx;
        let ra2 = u128::from_be_bytes(state.get_contract_storage(&pool, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&pool, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, u128::from(exp_ra));
        assert_eq!(rb2, u128::from(exp_rb));
    }

    #[test]
    fn ledger_router_v2_forwards_swap_to_v5_pool() {
        let pool_code = constant_product_pool_bytecode_v5();
        let pool = AccountId([0xce; 32]);
        let router_code = native_dex_ledger_router_bytecode_v2();
        let router = AccountId([0xde; 32]);
        let sender = AccountId([0xab; 32]);
        let output_recipient = AccountId([0x11; 32]);

        let mut state = StateStore::new();
        state.insert(Account {
            id: pool,
            state: Default::default(),
        });
        state.insert(Account {
            id: router,
            state: Default::default(),
        });
        state.set_contract_code(pool, pool_code);
        state.set_contract_code(router, router_code);

        let ra = 1_000u64;
        let rb = 2_000u64;
        let dx = 100u64;
        state.merge_contract_storage(pool, reserve_a_key(), amount_word(u128::from(ra)));
        state.merge_contract_storage(pool, reserve_b_key(), amount_word(u128::from(rb)));

        let dy = constant_product_amount_out_after_fee(ra, rb, dx);
        let inner = encode_swap_to_calldata(0, u128::from(dx), u128::from(dy), &output_recipient);
        let outer = encode_ledger_router_forward_calldata_v2(&pool.0, &inner);

        let mut it = Interpreter::new(native_dex_ledger_router_bytecode_v2(), 10_000_000);
        it.run(sender, router, &outer, &mut state).unwrap();

        let exp_rb = rb - dy;
        let exp_ra = ra + dx;
        let ra2 = u128::from_be_bytes(state.get_contract_storage(&pool, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&pool, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, u128::from(exp_ra));
        assert_eq!(rb2, u128::from(exp_rb));
    }

    #[test]
    fn ledger_router_v3_forwards_remove_liquidity_to_v5_pool() {
        fn ref_balance_key(holder: &AccountId) -> [u8; 32] {
            let mut k = holder.0;
            for i in 0..32 {
                k[i] ^= REF_FUNGIBLE_BALANCE_XOR[i];
            }
            k
        }
        fn word128_tail(w: &[u8; 32]) -> u128 {
            u128::from_be_bytes(w[16..32].try_into().unwrap())
        }

        let user = AccountId([0xabu8; 32]);
        let beneficiary = AccountId([0x22u8; 32]);
        let pool = AccountId([0xcd; 32]);
        let token_b = AccountId([0x77; 32]);
        let token_a_zero = AccountId([0u8; 32]);
        // Holds LP and is `Caller` on the pool for the forwarded `remove_liquidity_to`.
        let ledger_router = AccountId([0xde; 32]);

        let mut state = StateStore::new();
        for id in [user, beneficiary, pool, token_b, ledger_router] {
            state.insert(Account {
                id,
                state: Default::default(),
            });
        }
        let tok_code = reference_fungible_template_bytecode();
        state.set_contract_code(token_b, tok_code.clone());
        state.set_contract_code(pool, constant_product_pool_bytecode_v5());
        state.set_contract_code(ledger_router, native_dex_ledger_router_bytecode_v3());

        let mut it_mint = Interpreter::new(tok_code.clone(), 10_000_000);
        it_mint
            .run(
                user,
                token_b,
                &encode_mint_first_calldata(&pool, 10_000_000),
                &mut state,
            )
            .unwrap();

        let pool_code = state.get_contract_code(&pool).unwrap().clone();
        let set_cd = encode_set_tokens_calldata(&token_a_zero, &token_b);
        let mut it = Interpreter::new(pool_code.clone(), 10_000_000);
        it.run(user, pool, &set_cd, &mut state).unwrap();

        let mut it_add = Interpreter::new(pool_code.clone(), 10_000_000);
        it_add
            .run(
                ledger_router,
                pool,
                &encode_add_liquidity_calldata(1_000, 2_000, 0),
                &mut state,
            )
            .unwrap();

        let lp_total =
            u128::from_be_bytes(state.get_contract_storage(&pool, &total_lp_supply_key())[16..32].try_into().unwrap());
        assert_eq!(lp_total, 1_000);

        let burn = 500u128;
        let inner = encode_remove_liquidity_to_calldata(burn, 0, 0, &user, &beneficiary);
        let outer = encode_ledger_router_forward_calldata_v3(&pool.0, inner.as_slice());

        let mut it_rm = Interpreter::new(native_dex_ledger_router_bytecode_v3(), 10_000_000);
        it_rm.run(user, ledger_router, &outer, &mut state).unwrap();

        let exp_b_out = 1_000u128;
        assert_eq!(
            word128_tail(&state.get_contract_storage(&token_b, &ref_balance_key(&beneficiary))),
            exp_b_out
        );
    }

    #[test]
    fn native_dex_ledger_router_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = native_dex_ledger_router_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for ledger router bytecode, got {r:?}"
        );
    }

    #[test]
    fn native_dex_ledger_router_bytecode_v2_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = native_dex_ledger_router_bytecode_v2();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for ledger router v2 bytecode, got {r:?}"
        );
    }

    #[test]
    fn native_dex_ledger_router_bytecode_v3_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = native_dex_ledger_router_bytecode_v3();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for ledger router v3 bytecode, got {r:?}"
        );
    }

}
