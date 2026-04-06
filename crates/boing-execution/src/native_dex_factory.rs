//! On-chain **pair directory** for Boing-native DEX workflows (see `docs/NATIVE-DEX-FACTORY.md`).
//!
//! The VM has **no in-contract deploy** (`ContractDeploy` is transaction-only). This program is a
//! **registry**: integrators deploy each native CP pool in a separate `ContractDeploy` (Create2), then
//! call [`SELECTOR_REGISTER_PAIR`] to record `(token_a, token_b) → pool` for indexers and UIs.
//!
//! This is **not** an EVM-style factory that deploys new pair contracts from bytecode.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// `register_pair(token_a, token_b, pool)` — **128-byte** calldata (selector + three `AccountId` words).
pub const SELECTOR_REGISTER_PAIR: u8 = 0xD0;
/// `pairs_count` — **32-byte** calldata; returns **one** 32-byte word (count, low **8** bytes).
pub const SELECTOR_PAIRS_COUNT: u8 = 0xD1;
/// `get_pair_at(index)` — **64-byte** calldata (selector + index word); returns **96** bytes (ta, tb, pool).
pub const SELECTOR_GET_PAIR_AT: u8 = 0xD2;

/// `Log3` **topic0** after a successful [`SELECTOR_REGISTER_PAIR`]; **topic1** = token_a, **topic2** = token_b, **data** = pool id.
pub const NATIVE_DEX_FACTORY_TOPIC_REGISTER: [u8; 32] =
    *b"BOING_NATIVE_DEX_FACTORY_REG1\x00\x00\x00";

/// CREATE2 salt (`create2_salt: Some(...)`) for the canonical pair-directory bytecode ([`native_dex_factory_bytecode`]).
pub const NATIVE_DEX_FACTORY_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_NATIVEDEX_FACTORY_V1\x00\x00\x00\x00\x00\x00";

/// Inclusive upper bound on stored pairs (**index** runs **0 .. count**).
pub const NATIVE_DEX_FACTORY_MAX_PAIRS: u64 = 4096;

fn push32(code: &mut Vec<u8>, w: &[u8; 32]) {
    code.push(Opcode::Push32 as u8);
    code.extend_from_slice(w);
}

fn patch_push32_dest(code: &mut [u8], push32_opcode_at: usize, dest: usize) {
    code[push32_opcode_at + 1..push32_opcode_at + 33].copy_from_slice(&word_u64(dest as u64));
}

fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

/// Storage key: total number of registered pairs (word with count in low **8** bytes).
#[must_use]
pub fn native_dex_factory_count_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[16..24].copy_from_slice(b"BOINGDEX");
    k[28..32].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
    k
}

/// Base word for triplet keys: `storage_key = BASE + (index * 4 + field)` (**field** 0 = token_a, 1 = token_b, 2 = pool).
#[must_use]
pub fn native_dex_factory_triplet_base_word() -> [u8; 32] {
    let mut w = [0u8; 32];
    w[0..11].copy_from_slice(b"BOINGDEXDIR");
    w
}

fn append_build_triplet_key(code: &mut Vec<u8>, mem_idx: u64, field: u8) {
    push32(code, &word_u64(mem_idx));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(2));
    code.push(Opcode::Shl as u8);
    push32(code, &word_u64(u64::from(field)));
    code.push(Opcode::Add as u8);
    push32(code, &native_dex_factory_triplet_base_word());
    code.push(Opcode::Add as u8);
}

/// `SStore` pops **key** then **value**. Stack top must be **key**; load value first, then build key.
fn append_sstore_triplet_field(code: &mut Vec<u8>, mem_idx: u64, field: u8, calldata_word_off: u64) {
    push32(code, &word_u64(calldata_word_off));
    code.push(Opcode::MLoad as u8);
    append_build_triplet_key(code, mem_idx, field);
    code.push(Opcode::SStore as u8);
}

/// Encode [`SELECTOR_REGISTER_PAIR`] calldata (**128** bytes).
#[must_use]
pub fn encode_register_pair_calldata(token_a: &AccountId, token_b: &AccountId, pool: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_REGISTER_PAIR).to_vec();
    v.extend_from_slice(&token_a.0);
    v.extend_from_slice(&token_b.0);
    v.extend_from_slice(&pool.0);
    v
}

/// Encode [`SELECTOR_PAIRS_COUNT`] calldata (**32** bytes).
#[must_use]
pub fn encode_pairs_count_calldata() -> Vec<u8> {
    selector_word(SELECTOR_PAIRS_COUNT).to_vec()
}

/// Encode [`SELECTOR_GET_PAIR_AT`] calldata (**64** bytes). `index` is encoded in the low **8** bytes of the second word.
#[must_use]
pub fn encode_get_pair_at_calldata(index: u64) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_GET_PAIR_AT).to_vec();
    v.extend_from_slice(&word_u64(index));
    v
}

/// Canonical pair-directory bytecode (v1). CREATE2: [`NATIVE_DEX_FACTORY_CREATE2_SALT_V1`].
#[must_use]
pub fn native_dex_factory_bytecode() -> Vec<u8> {
    const MEM_IDX: u64 = 128;
    const MEM_LOG_PAIR: u64 = 224;
    const MEM_RET_BASE: u64 = 256;

    let mut c: Vec<u8> = Vec::new();

    // --- dispatch ---
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_REGISTER_PAIR));
    c.push(Opcode::Eq as u8);
    let fix_reg = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_PAIRS_COUNT));
    c.push(Opcode::Eq as u8);
    let fix_cnt = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_GET_PAIR_AT));
    c.push(Opcode::Eq as u8);
    let fix_get = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    let off_abort = c.len();
    c.push(Opcode::Stop as u8);

    // --- register_pair ---
    let off_register = c.len();
    patch_push32_dest(&mut c, fix_reg, off_register);

    // cnt = SLOAD count; MStore cnt at MEM_IDX (Dup, push MEM_IDX, MStore)
    push32(&mut c, &native_dex_factory_count_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Dup1 as u8);
    push32(&mut c, &word_u64(MEM_IDX));
    c.push(Opcode::MStore as u8);

    // if !(cnt < MAX) → abort
    push32(&mut c, &word_u64(MEM_IDX));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(NATIVE_DEX_FACTORY_MAX_PAIRS));
    c.push(Opcode::Lt as u8);
    c.push(Opcode::IsZero as u8);
    let fix_reg_abort = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    append_sstore_triplet_field(&mut c, MEM_IDX, 0, 32);
    append_sstore_triplet_field(&mut c, MEM_IDX, 1, 64);
    append_sstore_triplet_field(&mut c, MEM_IDX, 2, 96);

    // count + 1 → SSTORE count
    push32(&mut c, &word_u64(MEM_IDX));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(1));
    c.push(Opcode::Add as u8);
    push32(&mut c, &native_dex_factory_count_key());
    c.push(Opcode::SStore as u8);

    // Log3: data = pool at calldata 96; topics topic0, ta, tb
    push32(&mut c, &word_u64(96));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_LOG_PAIR));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &NATIVE_DEX_FACTORY_TOPIC_REGISTER);
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(32));
    push32(&mut c, &word_u64(MEM_LOG_PAIR));
    c.push(Opcode::Log3 as u8);

    c.push(Opcode::Stop as u8);

    patch_push32_dest(&mut c, fix_reg_abort, off_abort);

    // --- pairs_count ---
    let off_cnt = c.len();
    patch_push32_dest(&mut c, fix_cnt, off_cnt);

    push32(&mut c, &native_dex_factory_count_key());
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_RET_BASE));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(32));
    push32(&mut c, &word_u64(MEM_RET_BASE));
    c.push(Opcode::Return as u8);
    c.push(Opcode::Stop as u8);

    // --- get_pair_at ---
    let off_get = c.len();
    patch_push32_dest(&mut c, fix_get, off_get);

    // ix = MLoad(32); store at MEM_IDX for key builder
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_IDX));
    c.push(Opcode::MStore as u8);

    // if !(ix < count) → abort (`Lt` pops b then a; need ix < count ⇒ a=ix, b=count ⇒ stack top=count)
    push32(&mut c, &word_u64(MEM_IDX));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &native_dex_factory_count_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Lt as u8);
    c.push(Opcode::IsZero as u8);
    let fix_get_abort = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // SLOAD triplet into MEM_RET_BASE + 0/32/64
    for (field, off) in [(0u8, 0u64), (1, 32), (2, 64)] {
        append_build_triplet_key(&mut c, MEM_IDX, field);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_RET_BASE + off));
        c.push(Opcode::MStore as u8);
    }

    push32(&mut c, &word_u64(96));
    push32(&mut c, &word_u64(MEM_RET_BASE));
    c.push(Opcode::Return as u8);
    c.push(Opcode::Stop as u8);

    patch_push32_dest(&mut c, fix_get_abort, off_abort);

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::AccountId;
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;

    #[test]
    fn encode_register_len() {
        let a = AccountId([1u8; 32]);
        let b = AccountId([2u8; 32]);
        let p = AccountId([3u8; 32]);
        assert_eq!(encode_register_pair_calldata(&a, &b, &p).len(), 128);
    }

    #[test]
    fn factory_register_count_get_roundtrip() {
        let code = native_dex_factory_bytecode();
        let factory = AccountId([0xfa; 32]);
        let ta = AccountId([0x11; 32]);
        let tb = AccountId([0x22; 32]);
        let pool = AccountId([0x33; 32]);

        let mut state = StateStore::new();
        state.set_contract_code(factory, code);

        let mut it = Interpreter::new(native_dex_factory_bytecode(), 5_000_000);
        it.run(AccountId([0xee; 32]), factory, &encode_pairs_count_calldata(), &mut state)
            .unwrap();
        assert_eq!(it.return_data.as_deref(), Some(&[0u8; 32][..]));

        let mut it2 = Interpreter::new(native_dex_factory_bytecode(), 5_000_000);
        it2
            .run(
                AccountId([0xee; 32]),
                factory,
                &encode_register_pair_calldata(&ta, &tb, &pool),
                &mut state,
            )
            .unwrap();
        assert_eq!(it2.logs.len(), 1);
        assert_eq!(it2.logs[0].topics[0], NATIVE_DEX_FACTORY_TOPIC_REGISTER);
        assert_eq!(it2.logs[0].topics[1], ta.0);
        assert_eq!(it2.logs[0].topics[2], tb.0);
        assert_eq!(it2.logs[0].data, pool.0.to_vec());

        let mut it3 = Interpreter::new(native_dex_factory_bytecode(), 5_000_000);
        it3
            .run(AccountId([0xee; 32]), factory, &encode_pairs_count_calldata(), &mut state)
            .unwrap();
        let mut exp = [0u8; 32];
        exp[31] = 1;
        assert_eq!(it3.return_data.as_deref(), Some(&exp[..]));

        let mut it4 = Interpreter::new(native_dex_factory_bytecode(), 5_000_000);
        it4
            .run(AccountId([0xee; 32]), factory, &encode_get_pair_at_calldata(0), &mut state)
            .unwrap();
        let mut want = Vec::new();
        want.extend_from_slice(&ta.0);
        want.extend_from_slice(&tb.0);
        want.extend_from_slice(&pool.0);
        assert_eq!(it4.return_data.as_deref(), Some(want.as_slice()));
    }

    #[test]
    fn native_dex_factory_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = native_dex_factory_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for native dex factory bytecode, got {r:?}"
        );
    }
}
