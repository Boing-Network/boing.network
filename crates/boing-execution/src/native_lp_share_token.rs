//! **LP share token:** minter-gated **`mint`** + **`transfer`** for transferable pool share bookkeeping.
//!
//! The **minter** (typically an [`crate::native_amm_lp_vault::native_amm_lp_vault_bytecode`]) is set **once** via
//! [`SELECTOR_LP_SHARE_SET_MINTER_ONCE`]. See `docs/NATIVE-LP-SHARE-TOKEN.md`.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::{amount_word, selector_word};

/// `transfer(to, amount)` — **96** bytes (same layout as reference fungible).
pub const SELECTOR_LP_SHARE_TRANSFER: u8 = 0x01;
/// `mint(to, amount)` — **96** bytes; only [`LP_SHARE_MINTER_KEY`] may call.
pub const SELECTOR_LP_SHARE_MINT: u8 = 0x06;
/// `set_minter_once(minter)` — **64** bytes; succeeds only while minter slot is zero.
pub const SELECTOR_LP_SHARE_SET_MINTER_ONCE: u8 = 0x07;

/// Storage key: designated minter [`AccountId`] (`k[31] == 0xb1`).
pub const LP_SHARE_MINTER_KEY: [u8; 32] = {
    let mut k = [0u8; 32];
    k[31] = 0xb1;
    k
};

/// Same as [`LP_SHARE_MINTER_KEY`].
#[must_use]
pub fn lp_share_minter_key() -> [u8; 32] {
    LP_SHARE_MINTER_KEY
}

/// XOR mask for balance slots: `storage_key = account_id ^ LP_SHARE_BALANCE_XOR`.
pub const LP_SHARE_BALANCE_XOR: [u8; 32] =
    *b"BOING_LP_SHARE_BAL_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for [`lp_share_token_bytecode`].
pub const NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_LP_SHARE_TOKEN_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00";

const MEM_SCRATCH_SEL: u64 = 384;
const MEM_SCRATCH_TO: u64 = 352;
const MEM_SCRATCH_AMT: u64 = 416;
const MEM_OLD_BAL: u64 = 448;
const MEM_BAL_TO: u64 = 480;
const MEM_NEW_TO: u64 = 512;
const MEM_NEW_C: u64 = 544;
const MEM_KEY_CALLER: u64 = 576;
const MEM_KEY_TO: u64 = 608;

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

fn mask_low_byte() -> [u8; 32] {
    let mut m = [0u8; 32];
    m[31] = 0xff;
    m
}

#[must_use]
pub fn encode_lp_share_transfer_calldata(to: &AccountId, amount: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_LP_SHARE_TRANSFER).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&amount_word(amount));
    v
}

#[must_use]
pub fn encode_lp_share_mint_calldata(to: &AccountId, amount: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_LP_SHARE_MINT).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&amount_word(amount));
    v
}

#[must_use]
pub fn encode_lp_share_set_minter_once_calldata(minter: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_LP_SHARE_SET_MINTER_ONCE).to_vec();
    v.extend_from_slice(&minter.0);
    v
}

/// Share token bytecode. CREATE2: [`NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1`].
#[must_use]
pub fn lp_share_token_bytecode() -> Vec<u8> {
    let mut c = Vec::new();

    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &mask_low_byte());
    c.push(Opcode::And as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MStore as u8);

    // -- set_minter_once (0x07)
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_LP_SHARE_SET_MINTER_ONCE));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_sm = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &LP_SHARE_MINTER_KEY);
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_sm_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let off_sm_ok = c.len();
    patch_push32_dest(&mut c, fix_sm_ok, off_sm_ok);
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &LP_SHARE_MINTER_KEY);
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let off_after_sm = c.len();
    patch_push32_dest(&mut c, fix_skip_sm, off_after_sm);

    // -- mint (0x06)
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_LP_SHARE_MINT));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_mint = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    c.push(Opcode::Caller as u8);
    push32(&mut c, &LP_SHARE_MINTER_KEY);
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    let fix_mint_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let off_mint_ok = c.len();
    patch_push32_dest(&mut c, fix_mint_ok, off_mint_ok);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_mint_amt = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &LP_SHARE_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_mint_ovf = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &LP_SHARE_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let off_mint_abort = c.len();
    patch_push32_dest(&mut c, fix_mint_amt, off_mint_abort);
    patch_push32_dest(&mut c, fix_mint_ovf, off_mint_abort);
    c.push(Opcode::Stop as u8);

    let off_after_mint = c.len();
    patch_push32_dest(&mut c, fix_skip_mint, off_after_mint);

    // -- transfer (0x01)
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_LP_SHARE_TRANSFER));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_xfer = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_xfer_zero = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MStore as u8);

    c.push(Opcode::Caller as u8);
    push32(&mut c, &LP_SHARE_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    push32(&mut c, &word_u64(MEM_KEY_CALLER));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &LP_SHARE_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    push32(&mut c, &word_u64(MEM_KEY_TO));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_KEY_CALLER));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_xfer_under = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_KEY_TO));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_BAL_TO));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_BAL_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_BAL_TO));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_xfer_ovf = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(&mut c, &word_u64(MEM_NEW_C));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_NEW_C));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_KEY_CALLER));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_KEY_TO));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let off_xfer_abort = c.len();
    patch_push32_dest(&mut c, fix_xfer_under, off_xfer_abort);
    patch_push32_dest(&mut c, fix_xfer_ovf, off_xfer_abort);
    c.push(Opcode::Stop as u8);

    let off_xfer_zero_done = c.len();
    patch_push32_dest(&mut c, fix_xfer_zero, off_xfer_zero_done);
    c.push(Opcode::Stop as u8);

    let off_after_xfer = c.len();
    patch_push32_dest(&mut c, fix_skip_xfer, off_after_xfer);
    c.push(Opcode::Stop as u8);

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::Account;
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;

    fn bal_key(holder: &AccountId) -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = holder.0[i] ^ LP_SHARE_BALANCE_XOR[i];
        }
        k
    }

    fn tail_u128(w: &[u8; 32]) -> u128 {
        u128::from_be_bytes(w[16..32].try_into().unwrap())
    }

    #[test]
    fn lp_share_set_minter_mint_and_transfer() {
        let admin = AccountId([0xa1; 32]);
        let minter = AccountId([0xb2; 32]);
        let alice = AccountId([0xc3; 32]);
        let bob = AccountId([0xd4; 32]);
        let token = AccountId([0xe5; 32]);

        let mut state = StateStore::new();
        for id in [admin, minter, alice, bob, token] {
            state.insert(Account {
                id,
                state: Default::default(),
            });
        }
        let code = lp_share_token_bytecode();
        state.set_contract_code(token, code.clone());

        let mut it = Interpreter::new(code.clone(), 10_000_000);
        it.run(admin, token, &encode_lp_share_set_minter_once_calldata(&minter), &mut state)
            .unwrap();

        let mut it2 = Interpreter::new(code.clone(), 10_000_000);
        it2
            .run(
                minter,
                token,
                &encode_lp_share_mint_calldata(&alice, 1_000),
                &mut state,
            )
            .unwrap();
        assert_eq!(
            tail_u128(&state.get_contract_storage(&token, &bal_key(&alice))),
            1_000
        );

        let mut it3 = Interpreter::new(code.clone(), 10_000_000);
        it3
            .run(
                alice,
                token,
                &encode_lp_share_transfer_calldata(&bob, 400),
                &mut state,
            )
            .unwrap();
        assert_eq!(tail_u128(&state.get_contract_storage(&token, &bal_key(&alice))), 600);
        assert_eq!(tail_u128(&state.get_contract_storage(&token, &bal_key(&bob))), 400);
    }

    #[test]
    fn lp_share_token_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = lp_share_token_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("token"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for LP share token bytecode, got {r:?}"
        );
    }
}
