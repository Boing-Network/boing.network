//! Boing VM bytecode — minimal stack-based instruction set.
//!
//! The Boing VM is its own ISA. Opcode **bytes** and **semantics** are Boing-defined (`docs/TECHNICAL-SPECIFICATION.md` §7, `docs/BOING-VM-INDEPENDENCE.md`).
//! Any overlap with other instruction sets is incidental and does **not** imply compatibility.

/// Single-byte opcodes.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opcode {
    /// Halt execution (0x00)
    Stop = 0x00,
    /// Add top two stack values (0x01)
    Add = 0x01,
    /// Subtract (0x02)
    Sub = 0x02,
    /// Multiply (0x03)
    Mul = 0x03,
    /// Unsigned integer division (0x04). Divisor zero is a VM fault (`DivisionByZero`).
    Div = 0x04,
    /// Unsigned remainder (0x06). Divisor zero is a VM fault (`DivisionByZero`).
    Mod = 0x06,
    /// `(a + b) mod n` (0x08). Pops `n`, `b`, `a` (top first). If `n == 0`, pushes `0` (unlike `Div`/`Mod`).
    AddMod = 0x08,
    /// `(a * b) mod n` (0x09). Pops `n`, `b`, `a`. Full 512-bit product before reduce. If `n == 0`, pushes `0`.
    MulMod = 0x09,
    /// Less-than (unsigned, 0x10).
    Lt = 0x10,
    /// Greater-than (unsigned, 0x11)
    Gt = 0x11,
    /// Equal (0x14)
    Eq = 0x14,
    /// Is zero (0x15)
    IsZero = 0x15,
    /// Bitwise AND (0x16)
    And = 0x16,
    /// Bitwise OR (0x17)
    Or = 0x17,
    /// Bitwise XOR (0x18)
    Xor = 0x18,
    /// Bitwise NOT (0x19)
    Not = 0x19,
    /// Shift left (0x1b). Pops `shift`, then `value` (stack top = `shift`). Effective count = unsigned `shift` word mod 256 (big-endian low byte `shift[31]`). Push `(value << count) mod 2^256`.
    Shl = 0x1b,
    /// Logical shift right (0x1c). Pops `shift`, then `value`. Same effective count as [`Shl`](Opcode::Shl). Unsigned `value >> count`.
    Shr = 0x1c,
    /// Arithmetic shift right (0x1d). Pops `shift`, then `value`. Signed two's-complement 256-bit SAR; same effective count as [`Shl`](Opcode::Shl).
    Sar = 0x1d,
    /// Duplicate top stack word (0x80) — Boing VM `DUP1`-style (one slot).
    Dup1 = 0x80,
    /// Push this contract's `AccountId` as a 32-byte word (0x30).
    Address = 0x30,
    /// Push the transaction signer's `AccountId` (caller) as a 32-byte word (0x33).
    Caller = 0x33,
    /// Log with data only; pops `offset`, `size` (memory slice) (0xa0).
    Log0 = 0xa0,
    /// Log with one topic + data; pops `offset`, `size`, `topic0` (0xa1).
    Log1 = 0xa1,
    /// Log with two topics + data (0xa2).
    Log2 = 0xa2,
    /// Log with three topics + data (0xa3).
    Log3 = 0xa3,
    /// Log with four topics + data (0xa4).
    Log4 = 0xa4,
    /// Load from memory at offset (0x51)
    MLoad = 0x51,
    /// Store to memory (0x52)
    MStore = 0x52,
    /// Load from storage (0x54)
    SLoad = 0x54,
    /// Store to storage (0x55)
    SStore = 0x55,
    /// Push 1 byte immediate (0x60)
    Push1 = 0x60,
    /// Push 32 bytes (0x7f)
    Push32 = 0x7f,
    /// Pop and jump to offset (0x56)
    Jump = 0x56,
    /// Conditional jump (0x57)
    JumpI = 0x57,
    /// Return memory slice (0xf3)
    Return = 0xf3,
    /// Nested contract call (0xf1). Pops `ret_size`, `ret_offset`, `args_size`, `args_offset`, `target` (32-byte account id, stack top = `ret_size`). Runs callee with **caller** = current contract and **address** = `target`; merges callee logs; copies return data into caller memory (zero-pad). Pushes **`1`** on success. Uses remaining gas budget (minus `CALL` base). **`None` / empty code** → success, empty return. Errors from callee **propagate** (no partial snapshot rollback).
    Call = 0xf1,
    /// In-contract CREATE2 (0xf5). Pops `salt`, `size`, `offset` (stack top = `salt`, 32-byte word; then `size`, then `offset`). Deploys `memory[offset..offset+size)` as init/runtime bytecode with [`boing_primitives::create2_contract_address`]`(ADDRESS, salt, bytecode)`; init runs with `CALLER` = `ADDRESS`. Pushes new contract **address** word. Charges base + per-byte gas; init execution meters against **remaining** gas after that charge. Same QA path as in-tx deploy (`dapp` category). **Does not** increment any account nonce.
    Create2 = 0xf5,
}

impl Opcode {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Self::Stop),
            0x01 => Some(Self::Add),
            0x02 => Some(Self::Sub),
            0x03 => Some(Self::Mul),
            0x04 => Some(Self::Div),
            0x06 => Some(Self::Mod),
            0x08 => Some(Self::AddMod),
            0x09 => Some(Self::MulMod),
            0x10 => Some(Self::Lt),
            0x11 => Some(Self::Gt),
            0x14 => Some(Self::Eq),
            0x15 => Some(Self::IsZero),
            0x16 => Some(Self::And),
            0x17 => Some(Self::Or),
            0x18 => Some(Self::Xor),
            0x19 => Some(Self::Not),
            0x1b => Some(Self::Shl),
            0x1c => Some(Self::Shr),
            0x1d => Some(Self::Sar),
            0x30 => Some(Self::Address),
            0x33 => Some(Self::Caller),
            0x80 => Some(Self::Dup1),
            0xa0 => Some(Self::Log0),
            0xa1 => Some(Self::Log1),
            0xa2 => Some(Self::Log2),
            0xa3 => Some(Self::Log3),
            0xa4 => Some(Self::Log4),
            0x51 => Some(Self::MLoad),
            0x52 => Some(Self::MStore),
            0x54 => Some(Self::SLoad),
            0x55 => Some(Self::SStore),
            0x56 => Some(Self::Jump),
            0x57 => Some(Self::JumpI),
            0x60 => Some(Self::Push1),
            0x7f => Some(Self::Push32),
            0xf1 => Some(Self::Call),
            0xf3 => Some(Self::Return),
            0xf5 => Some(Self::Create2),
            _ => None,
        }
    }

    pub fn push_size(b: u8) -> Option<u8> {
        if (0x60..=0x7f).contains(&b) {
            Some(b - 0x5f) // PUSH1 = 1, PUSH32 = 32
        } else {
            None
        }
    }
}

/// Gas cost per opcode (base costs).
pub mod gas {
    pub const STOP: u64 = 0;
    pub const ADD: u64 = 3;
    pub const SUB: u64 = 3;
    pub const MUL: u64 = 5;
    pub const DIV: u64 = 5;
    pub const MOD: u64 = 5;
    /// Moderate step cost for modular reduce (`AddMod` / `MulMod`).
    pub const ADDMOD: u64 = 8;
    pub const MULMOD: u64 = 8;
    pub const CMP: u64 = 3;
    pub const ISZERO: u64 = 3;
    pub const BITWISE: u64 = 3;
    /// Same tier as bitwise ops (EIP-145 shift instructions).
    pub const SHIFT: u64 = 3;
    pub const MLOAD: u64 = 3;
    pub const MSTORE: u64 = 3;
    pub const SLOAD: u64 = 100;
    pub const SSTORE: u64 = 20_000;
    pub const PUSH: u64 = 3;
    pub const JUMP: u64 = 8;
    pub const JUMPI: u64 = 10;
    pub const RETURN: u64 = 0;
    /// Base gas before nested execution; child uses remaining `gas_limit - gas_used`.
    pub const CALL: u64 = 700;
    /// EIP-1014-style CREATE2 base (before init-code byte linear term and init execution).
    pub const CREATE2: u64 = 32_000;
    /// Per byte of init code read from memory (EIP-1014 `200` tier, simplified).
    pub const CREATE2_PER_INIT_BYTE: u64 = 200;
    pub const DUP1: u64 = 3;
    pub const ADDRESS: u64 = 2;
    pub const CALLER: u64 = 2;
    /// Base gas per log plus linear components (topic count and payload bytes).
    pub const LOG_BASE: u64 = 100;
    pub const LOG_PER_TOPIC: u64 = 375;
    pub const LOG_PER_DATA_BYTE: u64 = 8;
}
