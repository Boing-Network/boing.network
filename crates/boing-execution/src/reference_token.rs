//! Reference **fungible** calldata layout (Boing-defined), smoke contract for tests, and the
//! canonical **balances** template ([`reference_fungible_template_bytecode`]).
//!
//! See `docs/BOING-REFERENCE-TOKEN.md`, `docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md`, task **C6** in
//! `docs/EXECUTION-PARITY-TASK-LIST.md`.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;

/// Low byte of the first 32-byte calldata word (`MLoad(0)` & mask `0xff`).
pub const SELECTOR_TRANSFER: u8 = 0x01;
/// First successful call may establish treasury / supply (contract-specific; documented in spec).
pub const SELECTOR_MINT_FIRST: u8 = 0x02;

/// First calldata word: 31 zero bytes + selector in the low byte.
pub fn selector_word(selector: u8) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[31] = selector;
    w
}

/// `amount` as unsigned 256-bit word (big-endian; value in low 16 bytes for u128).
pub fn amount_word(amount: u128) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[16..32].copy_from_slice(&amount.to_be_bytes());
    w
}

/// Reference `transfer(to, amount)` calldata (96 bytes after layout): selector word + `to` + amount word.
pub fn encode_transfer_calldata(to: &AccountId, amount: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_TRANSFER).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&amount_word(amount));
    v
}

/// Reference first-mint style calldata (same 96-byte layout, different selector).
pub fn encode_mint_first_calldata(to: &AccountId, amount: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_MINT_FIRST).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&amount_word(amount));
    v
}

/// Smoke bytecode: `SSTORE` caller at key `0x01…01`, `LOG0` first 4 bytes of calldata, `RETURN` 32-byte caller id.
pub fn smoke_contract_bytecode() -> Vec<u8> {
    let mut v = Vec::new();
    v.push(Opcode::Caller as u8);
    v.push(Opcode::Dup1 as u8);
    v.push(Opcode::Push32 as u8);
    v.extend(std::iter::repeat_n(0x01u8, 32));
    v.push(Opcode::SStore as u8);
    // Calldata is already copied to memory [0..) by the interpreter; do not MSTORE at 0 before LOG0.
    v.push(Opcode::Push1 as u8);
    v.push(4);
    v.push(Opcode::Push1 as u8);
    v.push(0);
    v.push(Opcode::Log0 as u8);
    // RETURN 32-byte caller word from memory offset 32 (avoids clobbering calldata at 0).
    v.push(Opcode::Caller as u8);
    v.push(Opcode::Push1 as u8);
    v.push(32);
    v.push(Opcode::MStore as u8);
    v.push(Opcode::Push1 as u8);
    v.push(32);
    v.push(Opcode::Push1 as u8);
    v.push(32);
    v.push(Opcode::Return as u8);
    v.push(Opcode::Stop as u8);
    v
}

// --- Canonical fungible template (scratch memory, 256-bit balances) ---

const MEM_SCRATCH_SEL: u64 = 384;
const MEM_SCRATCH_TO: u64 = 352;
const MEM_SCRATCH_AMT: u64 = 416;
const MEM_OLD_BAL: u64 = 448;
/// Recipient balance before credit (must not overlap other 32-byte slots).
const MEM_BAL_TO: u64 = 480;
const MEM_NEW_TO: u64 = 512;
const MEM_NEW_C: u64 = 544;
const MEM_KEY_CALLER: u64 = 576;
const MEM_KEY_TO: u64 = 608;

/// Singleton storage key: **lazy admin** — first caller becomes admin when this slot is zero.
#[must_use]
pub fn ref_fungible_admin_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xf0;
    k
}

/// When non-zero, [`SELECTOR_MINT_FIRST`] has already succeeded once.
#[must_use]
pub fn ref_fungible_mint_once_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xf1;
    k
}

/// XOR mask for **balance** slots: `storage_key = account_id ^ REF_FUNGIBLE_BALANCE_XOR`.
pub const REF_FUNGIBLE_BALANCE_XOR: [u8; 32] =
    *b"BOING_REFFUNGIBLE_BAL1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

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

fn word_one() -> [u8; 32] {
    let mut w = [0u8; 32];
    w[31] = 1;
    w
}

/// Minimal **reference fungible** bytecode for `contract_deploy_meta` with purpose **`token`**.
///
/// Semantics:
/// - **Admin:** first successful call initializes [`ref_fungible_admin_key`] to `CALLER` (same idea
///   as the NFT collection template).
/// - **`mint_first`:** only if [`ref_fungible_mint_once_key`] is zero and `CALLER` is admin; mints
///   `amount` to `to` (256-bit balance with overflow checks). Then sets mint-once to `1`.
/// - **`transfer`:** moves `amount` from `CALLER` to `to` with balance and overflow checks.
/// - Balances live at `account_id ^ [`REF_FUNGIBLE_BALANCE_XOR`]`.
#[must_use]
pub fn reference_fungible_template_bytecode() -> Vec<u8> {
    let mut c = Vec::new();

    // Lazy admin
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_fungible_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_init_jumpi = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    let fix_skip_init = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    let off_init_admin = c.len();
    patch_push32_dest(&mut c, fix_init_jumpi, off_init_admin);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_fungible_admin_key());
    c.push(Opcode::SStore as u8);

    let off_dispatch = c.len();
    patch_push32_dest(&mut c, fix_skip_init, off_dispatch);

    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &mask_low_byte());
    c.push(Opcode::And as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MStore as u8);

    // -- mint_first (0x02)
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_MINT_FIRST));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_mint = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &ref_fungible_mint_once_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    c.push(Opcode::IsZero as u8);
    let fix_mint_used = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_fungible_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_mint_not_admin = c.len();
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

    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_mint_zero_amt = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_FUNGIBLE_BALANCE_XOR);
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
    push32(&mut c, &REF_FUNGIBLE_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_one());
    push32(&mut c, &ref_fungible_mint_once_key());
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let off_mint_abort = c.len();
    patch_push32_dest(&mut c, fix_mint_used, off_mint_abort);
    patch_push32_dest(&mut c, fix_mint_not_admin, off_mint_abort);
    patch_push32_dest(&mut c, fix_mint_zero_amt, off_mint_abort);
    patch_push32_dest(&mut c, fix_mint_ovf, off_mint_abort);
    c.push(Opcode::Stop as u8);

    let off_after_mint = c.len();
    patch_push32_dest(&mut c, fix_skip_mint, off_after_mint);

    // -- transfer (0x01)
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_TRANSFER));
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
    push32(&mut c, &REF_FUNGIBLE_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    push32(&mut c, &word_u64(MEM_KEY_CALLER));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_FUNGIBLE_BALANCE_XOR);
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

    #[test]
    fn smoke_contract_returns_caller_and_emits_log() {
        let sender = AccountId([0xabu8; 32]);
        let contract = AccountId([0xcd; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });
        let mut it = Interpreter::new(smoke_contract_bytecode(), 500_000);
        let calldata = b"ping";
        it.run(sender, contract, calldata, &mut state).unwrap();
        assert_eq!(it.return_data.as_deref(), Some(sender.0.as_slice()));
        assert_eq!(it.logs.len(), 1);
        assert!(it.logs[0].topics.is_empty());
        assert_eq!(it.logs[0].data.as_slice(), b"ping");
    }

    fn xor_balance_key(holder: &AccountId) -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = holder.0[i] ^ REF_FUNGIBLE_BALANCE_XOR[i];
        }
        k
    }

    fn word_u128(amount: u128) -> [u8; 32] {
        let mut w = [0u8; 32];
        w[16..32].copy_from_slice(&amount.to_be_bytes());
        w
    }

    #[test]
    fn reference_fungible_bytecode_contains_balance_xor_constant() {
        let code = reference_fungible_template_bytecode();
        assert!(
            code
                .windows(REF_FUNGIBLE_BALANCE_XOR.len())
                .any(|w| w == REF_FUNGIBLE_BALANCE_XOR),
            "template bytecode should embed REF_FUNGIBLE_BALANCE_XOR for SLoad keys"
        );
    }

    #[test]
    fn reference_fungible_transfer_with_prefunded_balance() {
        let deployer = AccountId([0xadu8; 32]);
        let bob = AccountId([0xcfu8; 32]);
        let token = AccountId([0x11u8; 32]);

        let mut state = StateStore::new();
        state.insert(Account {
            id: token,
            state: Default::default(),
        });
        let code = reference_fungible_template_bytecode();
        state.set_contract_code(token, code.clone());
        state.merge_contract_storage(token, xor_balance_key(&deployer), word_u128(1_000));

        let mut it = Interpreter::new(code.clone(), 5_000_000);
        it.run(
            deployer,
            token,
            &encode_transfer_calldata(&bob, 300),
            &mut state,
        )
        .unwrap();

        assert_eq!(
            state.get_contract_storage(&token, &xor_balance_key(&deployer)),
            word_u128(700)
        );
        assert_eq!(
            state.get_contract_storage(&token, &xor_balance_key(&bob)),
            word_u128(300)
        );
    }

    #[test]
    fn reference_fungible_template_mint_and_transfer() {
        let deployer = AccountId([0xadu8; 32]);
        let alice = AccountId([0xbeu8; 32]);
        let bob = AccountId([0xcfu8; 32]);
        let token = AccountId([0x11u8; 32]);

        let mut state = StateStore::new();
        state.insert(Account {
            id: token,
            state: Default::default(),
        });
        let code = reference_fungible_template_bytecode();
        state.set_contract_code(token, code.clone());

        {
            let mut it = Interpreter::new(code.clone(), 5_000_000);
            it.run(
                deployer,
                token,
                &encode_mint_first_calldata(&alice, 1_000),
                &mut state,
            )
            .unwrap();
        }

        assert_eq!(
            state.get_contract_storage(&token, &xor_balance_key(&alice)),
            word_u128(1_000)
        );

        {
            let mut it = Interpreter::new(code.clone(), 5_000_000);
            it.run(
                alice,
                token,
                &encode_transfer_calldata(&bob, 300),
                &mut state,
            )
            .unwrap();
        }

        assert_eq!(
            state.get_contract_storage(&token, &xor_balance_key(&alice)),
            word_u128(700)
        );
        assert_eq!(
            state.get_contract_storage(&token, &xor_balance_key(&bob)),
            word_u128(300)
        );
    }

    #[test]
    fn reference_fungible_second_mint_fails() {
        let deployer = AccountId([0x22u8; 32]);
        let other = AccountId([0x33u8; 32]);
        let token = AccountId([0x44u8; 32]);

        let mut state = StateStore::new();
        state.insert(Account {
            id: token,
            state: Default::default(),
        });
        let code = reference_fungible_template_bytecode();
        state.set_contract_code(token, code.clone());

        for calldata in [
            encode_mint_first_calldata(&other, 50),
            encode_mint_first_calldata(&other, 10),
        ] {
            let mut it = Interpreter::new(code.clone(), 5_000_000);
            it.run(deployer, token, &calldata, &mut state).unwrap();
        }

        assert_eq!(
            state.get_contract_storage(&token, &xor_balance_key(&other)),
            word_u128(50)
        );
    }

    #[test]
    fn reference_fungible_non_admin_cannot_mint() {
        let deployer = AccountId([0x55u8; 32]);
        let stranger = AccountId([0x66u8; 32]);
        let token = AccountId([0x77u8; 32]);

        let mut state = StateStore::new();
        state.insert(Account {
            id: token,
            state: Default::default(),
        });
        let code = reference_fungible_template_bytecode();
        state.set_contract_code(token, code.clone());

        let mut it = Interpreter::new(code.clone(), 5_000_000);
        it.run(
            deployer,
            token,
            &encode_transfer_calldata(&deployer, 0),
            &mut state,
        )
        .unwrap();

        let mut it = Interpreter::new(code.clone(), 5_000_000);
        it.run(
            stranger,
            token,
            &encode_mint_first_calldata(&stranger, 100),
            &mut state,
        )
        .unwrap();

        assert_eq!(
            state.get_contract_storage(&token, &xor_balance_key(&stranger)),
            [0u8; 32]
        );
    }

    #[test]
    fn reference_fungible_template_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = reference_fungible_template_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("token"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for reference fungible bytecode, got {r:?}"
        );
    }
}
