//! Reference **fungible** calldata layout (Boing-defined) and a tiny smoke contract for tests.
//!
//! Full token logic bytecode is not shipped here — see `docs/BOING-REFERENCE-TOKEN.md`. Deployed
//! contracts must still pass QA.

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
    v.extend(std::iter::repeat(0x01u8).take(32));
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
}
