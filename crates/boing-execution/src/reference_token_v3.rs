//! **Reference fungible template v3** — v2 semantics with Blake3 allowance keys.
//!
//! Requires [`Opcode::Blake3`] (`BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT`). Allowance slot:
//! `BLAKE3(pad32(BOING_REF_FUNGIBLE_ALLOWANCE_V3) ‖ owner ‖ spender)` via
//! [`crate::ref_fungible_allowance_key_v3_host`].

use boing_primitives::AccountId;

use crate::asm::{w_sel, w_u64, Asm, Label};
use crate::bytecode::Opcode;
use crate::hash_opcode_scaffold::{
    ref_fungible_allowance_key_v3_host, ref_fungible_allowance_v3_domain_word,
};
use crate::reference_token::{
    ref_fungible_admin_key, ref_fungible_mint_once_key, REF_FUNGIBLE_BALANCE_XOR,
    SELECTOR_MINT_FIRST, SELECTOR_TRANSFER,
};
use crate::reference_token_v2::{SELECTOR_ALLOWANCE, SELECTOR_APPROVE, SELECTOR_TRANSFER_FROM};

/// Host twin of the on-chain v3 allowance key.
#[must_use]
pub fn ref_fungible_allowance_key_v3(owner: &AccountId, spender: &AccountId) -> [u8; 32] {
    ref_fungible_allowance_key_v3_host(owner, spender)
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
const M_PRE: u64 = 896; // domain‖owner‖spender (96 bytes)

fn one() -> [u8; 32] {
    w_u64(1)
}

/// `dst = BLAKE3(domain_word ‖ owner ‖ spender)` using Opcode::Blake3 over 96 bytes at `M_PRE`.
fn allowance_key_to(a: &mut Asm, owner_mem: Option<u64>, spender_mem: Option<u64>, dst: u64) {
    a.push(&ref_fungible_allowance_v3_domain_word())
        .mstore(M_PRE);
    match owner_mem {
        Some(m) => a.mload(m),
        None => a.op(Opcode::Caller),
    };
    a.mstore(M_PRE + 32);
    match spender_mem {
        Some(m) => a.mload(m),
        None => a.op(Opcode::Caller),
    };
    a.mstore(M_PRE + 64);
    // stack: size then offset so offset is on top (matches Return / Blake3 pop order)
    a.push_u(96).push_u(M_PRE).op(Opcode::Blake3).mstore(dst);
}

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

/// Reference fungible **v3** bytecode (purpose `token`). See the module docs.
#[must_use]
pub fn reference_fungible_template_v3_bytecode() -> Vec<u8> {
    let mut a = Asm::new();
    let stop = a.label();
    let init = a.label();
    let dispatch = a.label();
    let mint = a.label();
    let xfer = a.label();
    let approve = a.label();
    let xfrom = a.label();
    let allowance = a.label();

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

    a.mark(xfer);
    a.mload(64).op(Opcode::IsZero).jumpi(stop);
    a.mcopy(32, M_TO).mcopy(64, M_AMT);
    a.mload(M_TO).op(Opcode::Caller).op(Opcode::Eq).jumpi(stop);
    move_balance(&mut a, None, M_TO, stop);
    a.op(Opcode::Stop);

    a.mark(approve);
    a.mcopy(32, M_TO).mcopy(64, M_AMT);
    allowance_key_to(&mut a, None, Some(M_TO), M_AKEY);
    a.mload(M_AMT).mload(M_AKEY).op(Opcode::SStore);
    a.push(&one()).mstore(M_RET).return_word(M_RET);

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
    use crate::interpreter::{Interpreter, VmExecutionContext};
    use crate::reference_token::encode_mint_first_calldata;
    use crate::reference_token_v2::{
        encode_approve_calldata, encode_transfer_from_calldata, ref_fungible_allowance_key,
    };
    use boing_primitives::Account;
    use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
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
            let ctx = VmExecutionContext {
                block_height: 1,
                block_timestamp: 0,
                native_value: false,
                vm_hardening: false,
                blake3_opcode: true,
            };
            it.run_with_qa(
                who,
                self.tok,
                &cd,
                &mut self.st,
                &RuleRegistry::new().with_blake3_opcodes(true),
                ctx,
            )
            .unwrap();
            it.return_data
        }
        fn bal(&self, h: &AccountId) -> u128 {
            u(self.st.get_contract_storage(&self.tok, &bal_key(h)))
        }
        fn alw(&self, o: &AccountId, s: &AccountId) -> u128 {
            u(self
                .st
                .get_contract_storage(&self.tok, &ref_fungible_allowance_key_v3(o, s)))
        }
    }
    const ADMIN: AccountId = AccountId([1; 32]);
    const BOB: AccountId = AccountId([2; 32]);
    const SPENDER: AccountId = AccountId([3; 32]);

    fn funded() -> T {
        let mut t = T::new(reference_fungible_template_v3_bytecode());
        t.call(ADMIN, encode_mint_first_calldata(&ADMIN, 1_000));
        t
    }

    #[test]
    fn allowance_key_matches_host_helper() {
        let mut t = funded();
        t.call(ADMIN, encode_approve_calldata(&SPENDER, 9));
        assert_eq!(t.alw(&ADMIN, &SPENDER), 9);
        // v2 key must be empty for the same pair
        let v2 =
            t.st.get_contract_storage(&t.tok, &ref_fungible_allowance_key(&ADMIN, &SPENDER));
        assert_eq!(v2, [0u8; 32]);
        assert_ne!(
            ref_fungible_allowance_key_v3(&ADMIN, &SPENDER),
            ref_fungible_allowance_key_v3(&SPENDER, &ADMIN)
        );
    }

    #[test]
    fn transfer_from_works() {
        let mut t = funded();
        t.call(ADMIN, encode_approve_calldata(&SPENDER, 300));
        let r = t.call(SPENDER, encode_transfer_from_calldata(&ADMIN, &BOB, 120));
        assert_eq!(r.as_deref(), Some(w_u64(1).as_slice()));
        assert_eq!(
            (t.bal(&ADMIN), t.bal(&BOB), t.alw(&ADMIN, &SPENDER)),
            (880, 120, 180)
        );
    }

    #[test]
    fn v3_template_passes_qa_with_blake3_allowed() {
        let r = check_contract_deploy_full(
            &reference_fungible_template_v3_bytecode(),
            Some("token"),
            None,
            &RuleRegistry::new().with_blake3_opcodes(true),
        );
        assert!(matches!(r, QaResult::Allow | QaResult::Unsure), "{r:?}");
        let denied = check_contract_deploy_full(
            &reference_fungible_template_v3_bytecode(),
            Some("token"),
            None,
            &RuleRegistry::new(),
        );
        assert!(matches!(denied, QaResult::Reject(_)), "{denied:?}");
    }

    #[test]
    fn dump_v3_hex_for_sdk() {
        if std::env::var("BOING_WRITE_POOL_VECTORS").is_err() {
            return;
        }
        let hex = hex::encode(reference_fungible_template_v3_bytecode());
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = root.join("boing-sdk/src/defaultReferenceFungibleTemplateV3BytecodeHex.ts");
        let body = format!(
            "// GENERATED by BOING_WRITE_POOL_VECTORS=1 cargo test -p boing-execution --lib dump_v3_hex_for_sdk. Do not edit.\n\n/** Reference fungible template v3 (Blake3 allowance keys; needs Blake3 opcode activation). */\nexport const DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_V3_BYTECODE_HEX: `0x${{string}}` =\n  '0x{hex}';\n"
        );
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn blake3_inactive_rejects_v3_call() {
        let mut t = T::new(reference_fungible_template_v3_bytecode());
        let code = t.st.get_contract_code(&t.tok).unwrap().clone();
        let mut it = Interpreter::new(code, 5_000_000);
        let ctx = VmExecutionContext {
            block_height: 0,
            block_timestamp: 0,
            native_value: false,
            vm_hardening: false,
            blake3_opcode: false,
        };
        let err = it
            .run_with_qa(
                ADMIN,
                t.tok,
                &encode_approve_calldata(&SPENDER, 1),
                &mut t.st,
                &RuleRegistry::new(),
                ctx,
            )
            .unwrap_err();
        assert!(matches!(err, crate::vm::VmError::InvalidBytecode));
    }
}
