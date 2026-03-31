//! Minimal **constant-product pool** bytecode (in-contract reserves) + calldata encoders.
//!
//! Matches `docs/NATIVE-AMM-CALLDATA.md` v0. **`Mul` on the Boing VM** only uses the **low 64 bits**
//! of each operand ([`Interpreter`](crate::interpreter::Interpreter)); keep reserves and amounts
//! ≤ `u64::MAX` (still encoded as reference-style u128 words with value in the low 16 bytes).
//!
//! No `CALL` opcode — ledger reserves only. See `docs/NATIVE-AMM-INTEGRATION-CHECKLIST.md`.

use crate::bytecode::Opcode;
use crate::reference_token::{amount_word, selector_word};

/// `swap` — word1 = direction (0 = A→B, 1 = B→A), word2 = amount_in, word3 = min_out.
pub const SELECTOR_SWAP: u8 = 0x10;
/// `add_liquidity` — word1 = amount_a, word2 = amount_b, word3 = min_liquidity (ignored in MVP).
pub const SELECTOR_ADD_LIQUIDITY: u8 = 0x11;
/// `remove_liquidity` — MVP: `STOP` (no state change).
pub const SELECTOR_REMOVE_LIQUIDITY: u8 = 0x12;

/// Storage key for reserve A.
#[must_use]
pub fn reserve_a_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x01;
    k
}

/// Storage key for reserve B.
#[must_use]
pub fn reserve_b_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x02;
    k
}

#[must_use]
pub fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

/// 128 bytes: `swap` calldata per NATIVE-AMM-CALLDATA.md.
#[must_use]
pub fn encode_swap_calldata(direction: u128, amount_in: u128, min_out: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_SWAP).to_vec();
    v.extend_from_slice(&amount_word(direction));
    v.extend_from_slice(&amount_word(amount_in));
    v.extend_from_slice(&amount_word(min_out));
    debug_assert_eq!(v.len(), 128);
    v
}

/// 128 bytes: `add_liquidity`.
#[must_use]
pub fn encode_add_liquidity_calldata(amount_a: u128, amount_b: u128, min_liquidity: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_ADD_LIQUIDITY).to_vec();
    v.extend_from_slice(&amount_word(amount_a));
    v.extend_from_slice(&amount_word(amount_b));
    v.extend_from_slice(&amount_word(min_liquidity));
    debug_assert_eq!(v.len(), 128);
    v
}

/// 128 bytes: `remove_liquidity` (MVP pool: no-op entry).
#[must_use]
pub fn encode_remove_liquidity_calldata(liquidity_burn: u128, min_a: u128, min_b: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_REMOVE_LIQUIDITY).to_vec();
    v.extend_from_slice(&amount_word(liquidity_burn));
    v.extend_from_slice(&amount_word(min_a));
    v.extend_from_slice(&amount_word(min_b));
    debug_assert_eq!(v.len(), 128);
    v
}

/// Integer constant-product out (no fee): \( \Delta_{out} = \lfloor r_{out} \cdot \Delta_{in} / (r_{in} + \Delta_{in}) \rfloor \).
#[must_use]
pub const fn constant_product_amount_out(reserve_in: u64, reserve_out: u64, amount_in: u64) -> u64 {
    let rin = reserve_in as u128;
    let rout = reserve_out as u128;
    let dx = amount_in as u128;
    let denom = rin.saturating_add(dx);
    if denom == 0 {
        return 0;
    }
    let num = rout.saturating_mul(dx);
    (num / denom) as u64
}

fn push32(code: &mut Vec<u8>, w: &[u8; 32]) {
    code.push(Opcode::Push32 as u8);
    code.extend_from_slice(w);
}

fn patch_push32_dest(code: &mut Vec<u8>, push32_opcode_at: usize, dest: usize) {
    code[push32_opcode_at + 1..push32_opcode_at + 33].copy_from_slice(&word_u64(dest as u64));
}

/// Assembled pool: dispatch + `add_liquidity` + `swap` (direction 0/1) + slippage + `remove` no-op.
///
/// Scratch memory (after calldata): see `MEM_*` in source.
#[must_use]
pub fn constant_product_pool_bytecode() -> Vec<u8> {
    // Memory scratch (offsets ≥ 128 to stay past 128-byte calldata).
    const MEM_DIR: u64 = 128;
    const MEM_RA: u64 = 160;
    const MEM_RB: u64 = 192;
    const MEM_DX: u64 = 224;
    const MEM_MIN: u64 = 256;
    const MEM_RIN: u64 = 288;
    const MEM_ROUT: u64 = 320;
    const MEM_RIN_P: u64 = 352;
    const MEM_DY: u64 = 384;

    let mut c: Vec<u8> = Vec::new();

    // --- dispatch: compare word0 to selectors ---
    // SWAP
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_SWAP));
    c.push(Opcode::Eq as u8);
    let fix_j_swap = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // ADD_LIQ
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_ADD_LIQUIDITY));
    c.push(Opcode::Eq as u8);
    let fix_j_add = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // REMOVE
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_REMOVE_LIQUIDITY));
    c.push(Opcode::Eq as u8);
    let fix_j_rm = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    let off_stop_unknown = c.len();
    c.push(Opcode::Stop as u8);

    // --- add_liquidity ---
    let off_add = c.len();
    patch_push32_dest(&mut c, fix_j_add, off_add);

    // ra += amount_a @ 32, rb += amount_b @ 64
    push32(&mut c, &reserve_a_key());
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &reserve_a_key());
    c.push(Opcode::SStore as u8);

    push32(&mut c, &reserve_b_key());
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &reserve_b_key());
    c.push(Opcode::SStore as u8);

    c.push(Opcode::Stop as u8);

    // --- remove: no-op ---
    let off_rm = c.len();
    patch_push32_dest(&mut c, fix_j_rm, off_rm);
    c.push(Opcode::Stop as u8);

    // --- swap ---
    let off_swap = c.len();
    patch_push32_dest(&mut c, fix_j_swap, off_swap);

    // Load ra, rb, dx, min; store scratch
    push32(&mut c, &reserve_a_key());
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_RA));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &reserve_b_key());
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_RB));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DX));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(96));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_MIN));
    c.push(Opcode::MStore as u8);

    // dx == 0 → abort
    push32(&mut c, &word_u64(MEM_DX));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_dx0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // dir at calldata 32 → mem MEM_DIR
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DIR));
    c.push(Opcode::MStore as u8);

    // if dir == 1 → B→A path (direction word equals `amount_word(1)`)
    push32(&mut c, &word_u64(MEM_DIR));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &amount_word(1));
    c.push(Opcode::Eq as u8);
    let fix_j_b2a = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // A→B: rin=ra, rout=rb
    push32(&mut c, &word_u64(MEM_RA));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_RIN));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(MEM_RB));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MStore as u8);
    let fix_after_dir = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    // B→A: rin=rb, rout=ra
    let off_b2a = c.len();
    patch_push32_dest(&mut c, fix_j_b2a, off_b2a);
    push32(&mut c, &word_u64(MEM_RB));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_RIN));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(MEM_RA));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MStore as u8);

    let off_swap_math = c.len();
    patch_push32_dest(&mut c, fix_after_dir, off_swap_math);

    // rin_p = rin + dx
    push32(&mut c, &word_u64(MEM_RIN));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DX));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MStore as u8);

    // rin_p == 0 → abort
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_rp0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // dy = rout * dx / rin_p
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DX));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Mul as u8);
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Div as u8);
    push32(&mut c, &word_u64(MEM_DY));
    c.push(Opcode::MStore as u8);

    // slippage: if dy < min_out → abort (`Lt`: top = b, next = a → a < b)
    push32(&mut c, &word_u64(MEM_DY));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_MIN));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_slip = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // rin_new = rin_p (already in MEM_RIN_P)
    // rout_new = rout - dy
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DY));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MStore as u8);

    // Write back: if we came from A→B, rin was ra, rout was rb. From B→A, rin was rb, rout was ra.
    // Re-dispatch using dir word: if dir==1, MEM_RA gets rout_new, MEM_RB gets rin_new? Actually after math:
    // MEM_RIN_P is new "in" reserve, MEM_ROUT holds new "out" reserve (we overwrote rout in MEM_ROUT with rout_new)

    // Reload dir
    push32(&mut c, &word_u64(MEM_DIR));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &amount_word(1));
    c.push(Opcode::Eq as u8);
    let fix_store_b2a = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // A→B store: ra = MEM_RIN_P, rb = MEM_ROUT
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &reserve_a_key());
    c.push(Opcode::SStore as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &reserve_b_key());
    c.push(Opcode::SStore as u8);
    let fix_after_store = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    // B→A store: rb = MEM_RIN_P, ra = MEM_ROUT
    let off_store_b2a = c.len();
    patch_push32_dest(&mut c, fix_store_b2a, off_store_b2a);
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &reserve_b_key());
    c.push(Opcode::SStore as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &reserve_a_key());
    c.push(Opcode::SStore as u8);

    let off_swap_done = c.len();
    patch_push32_dest(&mut c, fix_after_store, off_swap_done);
    c.push(Opcode::Stop as u8);

    // abort labels → STOP
    patch_push32_dest(&mut c, fix_dx0, off_stop_unknown);
    patch_push32_dest(&mut c, fix_rp0, off_stop_unknown);
    patch_push32_dest(&mut c, fix_slip, off_stop_unknown);

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{Account, AccountId};
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;

    #[test]
    fn encode_swap_matches_doc_example_structure() {
        let v = encode_swap_calldata(0, 1_000_000, 900_000);
        assert_eq!(v.len(), 128);
        assert_eq!(v[31], SELECTOR_SWAP);
    }

    #[test]
    fn constant_product_math_matches_swap_bytecode_a_to_b() {
        let ra = 1000u64;
        let rb = 2000u64;
        let dx = 100u64;
        let dy = constant_product_amount_out(ra, rb, dx);
        assert_eq!(dy, 181);

        let sender = AccountId([0xabu8; 32]);
        let contract = AccountId([0xcd; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });
        state.merge_contract_storage(contract, reserve_a_key(), amount_word(u128::from(ra)));
        state.merge_contract_storage(contract, reserve_b_key(), amount_word(u128::from(rb)));

        let code = constant_product_pool_bytecode();
        let calldata = encode_swap_calldata(0, u128::from(dx), u128::from(dy)); // min_out = exact

        let mut it = Interpreter::new(code, 5_000_000);
        it.run(sender, contract, &calldata, &mut state).unwrap();

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, u128::from(ra + dx));
        assert_eq!(rb2, u128::from(rb - dy));
    }

    #[test]
    fn swap_b_to_a_symmetric() {
        let ra = 500u64;
        let rb = 800u64;
        let dx = 50u64;
        let dy = constant_product_amount_out(rb, ra, dx); // in=B, out=A
        let sender = AccountId([1u8; 32]);
        let contract = AccountId([2u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });
        state.merge_contract_storage(contract, reserve_a_key(), amount_word(u128::from(ra)));
        state.merge_contract_storage(contract, reserve_b_key(), amount_word(u128::from(rb)));

        let code = constant_product_pool_bytecode();
        let calldata = encode_swap_calldata(1, u128::from(dx), u128::from(dy));

        let mut it = Interpreter::new(code, 5_000_000);
        it.run(sender, contract, &calldata, &mut state).unwrap();

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(rb2, u128::from(rb + dx));
        assert_eq!(ra2, u128::from(ra - dy));
    }

    #[test]
    fn add_liquidity_increases_reserves() {
        let sender = AccountId([3u8; 32]);
        let contract = AccountId([4u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });
        state.merge_contract_storage(contract, reserve_a_key(), amount_word(100));
        state.merge_contract_storage(contract, reserve_b_key(), amount_word(200));

        let code = constant_product_pool_bytecode();
        let calldata = encode_add_liquidity_calldata(10, 20, 0);
        let mut it = Interpreter::new(code, 5_000_000);
        it.run(sender, contract, &calldata, &mut state).unwrap();

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, 110);
        assert_eq!(rb2, 220);
    }

    /// Deploy-shaped bootstrap: empty pool → add liquidity → swap A→B; assert reserves.
    #[test]
    fn add_liquidity_then_swap_integration() {
        let sender = AccountId([0x11u8; 32]);
        let contract = AccountId([0x22u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });

        let code = constant_product_pool_bytecode();

        let add_calldata = encode_add_liquidity_calldata(1_000, 2_000, 0);
        let mut it = Interpreter::new(code.clone(), 5_000_000);
        it.run(sender, contract, &add_calldata, &mut state).unwrap();

        let ra = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra, 1_000);
        assert_eq!(rb, 2_000);

        let dx = 100u128;
        let dy = constant_product_amount_out(1_000u64, 2_000u64, dx as u64);
        let swap_calldata = encode_swap_calldata(0, dx, u128::from(dy));
        let mut it2 = Interpreter::new(code, 5_000_000);
        it2.run(sender, contract, &swap_calldata, &mut state).unwrap();

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, 1_000 + dx);
        assert_eq!(rb2, 2_000 - u128::from(dy));
    }

    /// `boing_qa` / `boing_qaCheck` must not reject canonical pool bytecode (checklist **A1.4**).
    #[test]
    fn constant_product_pool_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = constant_product_pool_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for native CP pool bytecode, got {r:?}"
        );
    }
}
