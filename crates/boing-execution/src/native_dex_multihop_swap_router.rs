//! **Multihop swap router:** **2–4** sequential [`Opcode::Call`]s to native CP pools in **one** transaction.
//!
//! Selectors **`0xE5`–`0xEA`** cover **128**-byte inners (ledger **`swap`**) and **160**-byte inners (v5 **`swap_to`**).
//! See `docs/NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md`.

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// **2-hop**, **128-byte** inners — **352-byte** outer calldata.
pub const SELECTOR_SWAP2_ROUTER_128: u8 = 0xE5;
/// **2-hop**, **160-byte** inners — **416-byte** outer calldata.
pub const SELECTOR_SWAP2_ROUTER_160: u8 = 0xE6;
/// **3-hop**, **128-byte** inners — **512-byte** outer calldata.
pub const SELECTOR_SWAP3_ROUTER_128: u8 = 0xE7;
/// **3-hop**, **160-byte** inners — **608-byte** outer calldata.
pub const SELECTOR_SWAP3_ROUTER_160: u8 = 0xE8;
/// **4-hop**, **128-byte** inners — **672-byte** outer calldata.
pub const SELECTOR_SWAP4_ROUTER_128: u8 = 0xE9;
/// **4-hop**, **160-byte** inners — **800-byte** outer calldata.
pub const SELECTOR_SWAP4_ROUTER_160: u8 = 0xEA;

/// CREATE2 salt for [`native_dex_multihop_swap_router_bytecode`].
pub const NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_NATIVEDEX_MHOP_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// @deprecated Use [`NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1`]; same bytes for compatibility.
pub const NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1: [u8; 32] =
    NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1;

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

fn patch_push32_dest(code: &mut [u8], push32_opcode_at: usize, dest: usize) {
    code[push32_opcode_at + 1..push32_opcode_at + 33].copy_from_slice(&word_u64(dest as u64));
}

fn stride(call_size: u64) -> u64 {
    32 + call_size
}

#[allow(dead_code)] // used by `#[cfg(test)]` assertions; keep near encoders for layout checks
fn outer_len(hops: u64, call_size: u64) -> usize {
    (32 + hops * stride(call_size)) as usize
}

fn encode_outer(selector: u8, pools_and_inners: &[(&[u8; 32], &[u8])]) -> Vec<u8> {
    let mut v = selector_word(selector).to_vec();
    for (pool, inner) in pools_and_inners {
        v.extend_from_slice(pool.as_slice());
        v.extend_from_slice(inner);
    }
    v
}

/// Encode **352** bytes: **`0xE5`** + two (**pool**, **128-byte** inner) pairs.
#[must_use]
pub fn encode_swap2_router_calldata_128(
    pool1: &[u8; 32],
    inner1_128: &[u8],
    pool2: &[u8; 32],
    inner2_128: &[u8],
) -> Vec<u8> {
    assert_eq!(inner1_128.len(), 128);
    assert_eq!(inner2_128.len(), 128);
    encode_outer(
        SELECTOR_SWAP2_ROUTER_128,
        &[(pool1, inner1_128), (pool2, inner2_128)],
    )
}

/// Encode **416** bytes: **`0xE6`** + two (**pool**, **160-byte** inner) pairs.
#[must_use]
pub fn encode_swap2_router_calldata_160(
    pool1: &[u8; 32],
    inner1_160: &[u8],
    pool2: &[u8; 32],
    inner2_160: &[u8],
) -> Vec<u8> {
    assert_eq!(inner1_160.len(), 160);
    assert_eq!(inner2_160.len(), 160);
    encode_outer(
        SELECTOR_SWAP2_ROUTER_160,
        &[(pool1, inner1_160), (pool2, inner2_160)],
    )
}

/// Encode **512** bytes: **`0xE7`** + three (**pool**, **128-byte** inner) triples.
#[must_use]
pub fn encode_swap3_router_calldata_128(
    pool1: &[u8; 32],
    inner1: &[u8],
    pool2: &[u8; 32],
    inner2: &[u8],
    pool3: &[u8; 32],
    inner3: &[u8],
) -> Vec<u8> {
    assert_eq!(inner1.len(), 128);
    assert_eq!(inner2.len(), 128);
    assert_eq!(inner3.len(), 128);
    encode_outer(
        SELECTOR_SWAP3_ROUTER_128,
        &[(pool1, inner1), (pool2, inner2), (pool3, inner3)],
    )
}

/// Encode **608** bytes: **`0xE8`** + three (**pool**, **160-byte** inner) triples.
#[must_use]
pub fn encode_swap3_router_calldata_160(
    pool1: &[u8; 32],
    inner1: &[u8],
    pool2: &[u8; 32],
    inner2: &[u8],
    pool3: &[u8; 32],
    inner3: &[u8],
) -> Vec<u8> {
    assert_eq!(inner1.len(), 160);
    assert_eq!(inner2.len(), 160);
    assert_eq!(inner3.len(), 160);
    encode_outer(
        SELECTOR_SWAP3_ROUTER_160,
        &[(pool1, inner1), (pool2, inner2), (pool3, inner3)],
    )
}

/// Encode **672** bytes: **`0xE9`** + four (**pool**, **128-byte** inner) pairs.
#[must_use]
pub fn encode_swap4_router_calldata_128(
    pool1: &[u8; 32],
    inner1: &[u8],
    pool2: &[u8; 32],
    inner2: &[u8],
    pool3: &[u8; 32],
    inner3: &[u8],
    pool4: &[u8; 32],
    inner4: &[u8],
) -> Vec<u8> {
    for i in [inner1, inner2, inner3, inner4] {
        assert_eq!(i.len(), 128);
    }
    encode_outer(
        SELECTOR_SWAP4_ROUTER_128,
        &[(pool1, inner1), (pool2, inner2), (pool3, inner3), (pool4, inner4)],
    )
}

/// Encode **800** bytes: **`0xEA`** + four (**pool**, **160-byte** inner) pairs.
#[must_use]
pub fn encode_swap4_router_calldata_160(
    pool1: &[u8; 32],
    inner1: &[u8],
    pool2: &[u8; 32],
    inner2: &[u8],
    pool3: &[u8; 32],
    inner3: &[u8],
    pool4: &[u8; 32],
    inner4: &[u8],
) -> Vec<u8> {
    for i in [inner1, inner2, inner3, inner4] {
        assert_eq!(i.len(), 160);
    }
    encode_outer(
        SELECTOR_SWAP4_ROUTER_160,
        &[(pool1, inner1), (pool2, inner2), (pool3, inner3), (pool4, inner4)],
    )
}

fn append_copy_inner_to_scratch(code: &mut Vec<u8>, inner_offset: u64, num_words: u64) {
    for i in 0u64..num_words {
        let src = inner_offset + i * 32;
        let dst = i * 32;
        push32(code, &word_u64(src));
        code.push(Opcode::MLoad as u8);
        push32(code, &word_u64(dst));
        code.push(Opcode::MStore as u8);
    }
}

fn append_call_pool(
    code: &mut Vec<u8>,
    pool_calldata_offset: u64,
    inner_offset: u64,
    inner_words: u64,
    call_size: u64,
) {
    push32(code, &word_u64(pool_calldata_offset));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(MEM_POOL_TMP));
    code.push(Opcode::MStore as u8);
    append_copy_inner_to_scratch(code, inner_offset, inner_words);
    push32(code, &word_u64(MEM_POOL_TMP));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(0));
    push32(code, &word_u64(call_size));
    push32(code, &word_u64(0));
    push32(code, &word_u64(0));
    code.push(Opcode::Call as u8);
}

fn append_multihop_body(code: &mut Vec<u8>, hops: u64, inner_words: u64, call_size: u64) {
    let s = stride(call_size);
    for k in 0..hops {
        let pool_off = 32 + k * s;
        let inner_off = 64 + k * s;
        append_call_pool(code, pool_off, inner_off, inner_words, call_size);
    }
    code.push(Opcode::Stop as u8);
}

/// Multihop router (**2–4** pools). CREATE2: [`NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1`].
#[must_use]
pub fn native_dex_multihop_swap_router_bytecode() -> Vec<u8> {
    let mut c: Vec<u8> = Vec::new();

    let selectors = [
        SELECTOR_SWAP2_ROUTER_128,
        SELECTOR_SWAP2_ROUTER_160,
        SELECTOR_SWAP3_ROUTER_128,
        SELECTOR_SWAP3_ROUTER_160,
        SELECTOR_SWAP4_ROUTER_128,
        SELECTOR_SWAP4_ROUTER_160,
    ];
    let mut fix_jumps = Vec::new();
    for sel in selectors {
        push32(&mut c, &word_u64(0));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &selector_word(sel));
        c.push(Opcode::Eq as u8);
        let at = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
        fix_jumps.push(at);
    }
    c.push(Opcode::Stop as u8);

    let bodies: [(u64, u64, u64); 6] = [
        (2, 4, 128),
        (2, 5, 160),
        (3, 4, 128),
        (3, 5, 160),
        (4, 4, 128),
        (4, 5, 160),
    ];
    for (i, &(hops, words, size)) in bodies.iter().enumerate() {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[i], off);
        append_multihop_body(&mut c, hops, words, size);
    }

    c
}

/// @deprecated Alias for [`native_dex_multihop_swap_router_bytecode`].
#[must_use]
pub fn native_dex_swap2_router_bytecode() -> Vec<u8> {
    native_dex_multihop_swap_router_bytecode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{Account, AccountId};
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;
    use crate::native_amm::{
        constant_product_amount_out_after_fee, constant_product_pool_bytecode, encode_swap_calldata, reserve_a_key,
        reserve_b_key,
    };
    use crate::reference_token::amount_word;

    #[test]
    fn swap2_router_runs_two_v1_swaps_in_one_tx() {
        let p1 = AccountId([0x01; 32]);
        let p2 = AccountId([0x02; 32]);
        let router = AccountId([0xde; 32]);
        let sender = AccountId([0xab; 32]);

        let pool_code = constant_product_pool_bytecode();
        let router_code = native_dex_multihop_swap_router_bytecode();

        let mut state = StateStore::new();
        for id in [p1, p2, router] {
            state.insert(Account {
                id,
                state: Default::default(),
            });
        }
        state.set_contract_code(p1, pool_code.clone());
        state.set_contract_code(p2, pool_code.clone());
        state.set_contract_code(router, router_code);

        let r1a = 1_000u64;
        let r1b = 2_000u64;
        let r2a = 500u64;
        let r2b = 800u64;
        state.merge_contract_storage(p1, reserve_a_key(), amount_word(u128::from(r1a)));
        state.merge_contract_storage(p1, reserve_b_key(), amount_word(u128::from(r1b)));
        state.merge_contract_storage(p2, reserve_a_key(), amount_word(u128::from(r2a)));
        state.merge_contract_storage(p2, reserve_b_key(), amount_word(u128::from(r2b)));

        let dx1 = 50u64;
        let dy1 = constant_product_amount_out_after_fee(r1a, r1b, dx1);
        let inner1 = encode_swap_calldata(0, u128::from(dx1), u128::from(dy1));

        let dx2 = 40u64;
        let dy2 = constant_product_amount_out_after_fee(r2a, r2b, dx2);
        let inner2 = encode_swap_calldata(0, u128::from(dx2), u128::from(dy2));

        let outer = encode_swap2_router_calldata_128(&p1.0, &inner1, &p2.0, &inner2);

        let mut it = Interpreter::new(native_dex_multihop_swap_router_bytecode(), 20_000_000);
        it.run(sender, router, &outer, &mut state).unwrap();

        let e1a = r1a + dx1;
        let e1b = r1b - dy1;
        let e2a = r2a + dx2;
        let e2b = r2b - dy2;

        let p1a = u128::from_be_bytes(state.get_contract_storage(&p1, &reserve_a_key())[16..32].try_into().unwrap());
        let p1b = u128::from_be_bytes(state.get_contract_storage(&p1, &reserve_b_key())[16..32].try_into().unwrap());
        let p2a = u128::from_be_bytes(state.get_contract_storage(&p2, &reserve_a_key())[16..32].try_into().unwrap());
        let p2b = u128::from_be_bytes(state.get_contract_storage(&p2, &reserve_b_key())[16..32].try_into().unwrap());

        assert_eq!(p1a, u128::from(e1a));
        assert_eq!(p1b, u128::from(e1b));
        assert_eq!(p2a, u128::from(e2a));
        assert_eq!(p2b, u128::from(e2b));
    }

    #[test]
    fn swap3_router_runs_three_v1_swaps() {
        let pools = [AccountId([0x11; 32]), AccountId([0x22; 32]), AccountId([0x33; 32])];
        let router = AccountId([0xde; 32]);
        let sender = AccountId([0xab; 32]);
        let pool_code = constant_product_pool_bytecode();
        let router_code = native_dex_multihop_swap_router_bytecode();

        let mut state = StateStore::new();
        for id in pools.iter().chain([&router]) {
            state.insert(Account {
                id: *id,
                state: Default::default(),
            });
        }
        for p in pools {
            state.set_contract_code(p, pool_code.clone());
            state.merge_contract_storage(p, reserve_a_key(), amount_word(1_000));
            state.merge_contract_storage(p, reserve_b_key(), amount_word(2_000));
        }
        state.set_contract_code(router, router_code);

        let mut inners = Vec::new();
        for _ in 0..3 {
            let inner = encode_swap_calldata(0, 10u128, 1u128);
            inners.push(inner);
        }
        let outer = encode_swap3_router_calldata_128(
            &pools[0].0,
            &inners[0],
            &pools[1].0,
            &inners[1],
            &pools[2].0,
            &inners[2],
        );
        assert_eq!(outer.len(), outer_len(3, 128));

        let mut it = Interpreter::new(native_dex_multihop_swap_router_bytecode(), 30_000_000);
        it.run(sender, router, &outer, &mut state).unwrap();
    }

    #[test]
    fn swap4_router_runs_four_v1_swaps() {
        let pools = [
            AccountId([0x41; 32]),
            AccountId([0x42; 32]),
            AccountId([0x43; 32]),
            AccountId([0x44; 32]),
        ];
        let router = AccountId([0xde; 32]);
        let sender = AccountId([0xab; 32]);
        let pool_code = constant_product_pool_bytecode();
        let router_code = native_dex_multihop_swap_router_bytecode();

        let mut state = StateStore::new();
        for id in pools.iter().chain([&router]) {
            state.insert(Account {
                id: *id,
                state: Default::default(),
            });
        }
        for p in pools {
            state.set_contract_code(p, pool_code.clone());
            state.merge_contract_storage(p, reserve_a_key(), amount_word(900));
            state.merge_contract_storage(p, reserve_b_key(), amount_word(900));
        }
        state.set_contract_code(router, router_code);

        let inner = encode_swap_calldata(0, 5u128, 1u128);
        let outer = encode_swap4_router_calldata_128(
            &pools[0].0, &inner, &pools[1].0, &inner, &pools[2].0, &inner, &pools[3].0, &inner,
        );
        assert_eq!(outer.len(), outer_len(4, 128));

        let mut it = Interpreter::new(native_dex_multihop_swap_router_bytecode(), 40_000_000);
        it.run(sender, router, &outer, &mut state).unwrap();
    }

    #[test]
    fn native_dex_multihop_swap_router_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = native_dex_multihop_swap_router_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for multihop swap router bytecode, got {r:?}"
        );
    }
}
