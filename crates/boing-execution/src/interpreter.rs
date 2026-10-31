//! Boing VM interpreter — deterministic stack machine.
//!
//! Executes bytecode with gas metering.

use std::cmp::Ordering;

use boing_primitives::{
    AccountId, ExecutionLog, MAX_EXECUTION_LOGS_PER_TX, MAX_EXECUTION_LOG_DATA_BYTES,
    MAX_EXECUTION_LOG_TOPICS, MAX_RECEIPT_RETURN_DATA_BYTES,
};
use boing_qa::RuleRegistry;
use boing_state::StateStore;
use num_bigint::{BigInt, BigUint, Sign};

use super::bytecode::{gas, Opcode};
use super::vm::VmError;

/// Maximum nested [`Opcode::Call`] depth (root frame = 0).
pub const MAX_CALL_DEPTH: u8 = 64;

/// Maximum operand-stack depth per frame once [`VmExecutionContext::vm_hardening`] is active
/// (audit #61 item 23; same bound as the EVM).
pub const MAX_STACK_DEPTH: usize = 1024;

/// Linear gas per 32-byte memory word once [`VmExecutionContext::vm_hardening`] is active.
pub const MEMORY_GAS_PER_WORD: u64 = 3;
/// Quadratic divisor: expanding to `w` words costs `3*w + w*w/512` in total (EVM schedule).
pub const MEMORY_GAS_QUAD_DIVISOR: u64 = 512;

/// Total memory gas for a frame of `words` 32-byte words (audit #61 item 22).
pub fn memory_gas_cost(words: u64) -> u64 {
    MEMORY_GAS_PER_WORD
        .saturating_mul(words)
        .saturating_add(words.saturating_mul(words) / MEMORY_GAS_QUAD_DIVISOR)
}

/// Block-level execution context supplied by the host ([`Vm`](crate::vm::Vm)) for every transaction.
///
/// Drives [`Opcode::BlockHeight`] and [`Opcode::Timestamp`]. Defaults to zeros when omitted (tests / legacy callers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VmExecutionContext {
    pub block_height: u64,
    pub block_timestamp: u64,
    /// Native-value rules active for this block (`BALANCE`, `CALLVALUE`, `SELFBALANCE`,
    /// `NATIVE_TRANSFER` execute; otherwise they are invalid bytecode, as before the activation
    /// height). Set by [`crate::Vm`] from the block height; hosts may leave it `false`.
    pub native_value: bool,
    /// VM hardening rules active for this block (audit #61 items 3, 22, 23): init-code runtime QA,
    /// memory-expansion gas and the [`MAX_STACK_DEPTH`] cap. Set by [`crate::Vm`] from the block
    /// height; `false` keeps the historical behaviour exactly.
    pub vm_hardening: bool,
}

fn word_u64_env(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

fn word_u128(n: u128) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[16..32].copy_from_slice(&n.to_be_bytes());
    w
}

fn u256_be_to_biguint(w: &[u8; 32]) -> BigUint {
    BigUint::from_bytes_be(w)
}

/// Effective shift count for `Shl` / `Shr` / `Sar`: unsigned shift word modulo 256 (see `TECHNICAL-SPECIFICATION.md` §7.2).
#[inline]
fn effective_shift_count(w: &[u8; 32]) -> u32 {
    w[31] as u32
}

fn signed_bigint_to_i256_word(i: &BigInt) -> [u8; 32] {
    let mut bytes = i.to_signed_bytes_be();
    if bytes.is_empty() {
        return [0u8; 32];
    }
    let neg = matches!(i.sign(), Sign::Minus);
    let fill: u8 = if neg { 0xff } else { 0x00 };
    if bytes.len() > 32 {
        bytes = bytes[bytes.len() - 32..].to_vec();
    }
    let mut out = [fill; 32];
    let start = 32usize.saturating_sub(bytes.len());
    out[start..].copy_from_slice(&bytes);
    out
}

fn biguint_rem_to_u256_be(rem: BigUint) -> [u8; 32] {
    let bytes = rem.to_bytes_be();
    let mut out = [0u8; 32];
    if bytes.is_empty() {
        return out;
    }
    let len = bytes.len().min(32);
    out[32 - len..].copy_from_slice(&bytes[bytes.len() - len..]);
    out
}

/// Low **256** bits of a nonnegative integer as a big-endian EVM-style word (product mod **2²⁵⁶**).
fn biguint_low_u256_be(v: BigUint) -> [u8; 32] {
    let bytes = v.to_bytes_be();
    let mut out = [0u8; 32];
    if bytes.is_empty() {
        return out;
    }
    if bytes.len() <= 32 {
        out[32 - bytes.len()..].copy_from_slice(&bytes);
    } else {
        out.copy_from_slice(&bytes[bytes.len() - 32..]);
    }
    out
}

/// Stack machine interpreter.
pub struct Interpreter {
    pub code: Vec<u8>,
    pub pc: usize,
    pub stack: Vec<[u8; 32]>,
    pub memory: Vec<u8>,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub return_data: Option<Vec<u8>>,
    /// Emitted during `run`; consumed by the host after execution.
    pub logs: Vec<ExecutionLog>,
    caller_id: AccountId,
    contract_id: AccountId,
    exec_ctx: VmExecutionContext,
    /// Native BOING attached to this frame (`CALLVALUE`). Set by the host for the top-level frame of a
    /// value-carrying call; `0` for every nested / init frame.
    pub call_value: u128,
}

/// Storage interface for SLOAD/SSTORE, nested [`Opcode::Call`], and [`Opcode::Create2`].
pub trait StorageAccess {
    fn sload(&self, contract: AccountId, key: [u8; 32]) -> [u8; 32];
    fn sstore(&mut self, contract: AccountId, key: [u8; 32], value: [u8; 32]);
    /// Contract bytecode at `contract`. `None` or empty → `CALL` succeeds with empty return (no execution).
    fn get_contract_code(&self, contract: AccountId) -> Option<Vec<u8>>;

    /// In-contract CREATE2 (opcode `0xf5`). Default: unsupported (custom [`StorageAccess`] impls).
    fn create2_deploy(
        &mut self,
        _qa_registry: &RuleRegistry,
        _deployer: AccountId,
        _salt: [u8; 32],
        _bytecode: Vec<u8>,
        _init_gas_limit: u64,
        _parent_logs: &mut Vec<ExecutionLog>,
        _parent_call_depth: u8,
        _exec_ctx: VmExecutionContext,
    ) -> Result<(AccountId, u64), VmError> {
        Err(VmError::Create2NotSupported)
    }

    /// Native BOING balance of `account` (base units). Default: `0` (custom backends).
    fn native_balance(&self, _account: AccountId) -> u128 {
        0
    }

    /// Move `amount` native BOING from `from` to `to` (journaled, reverted with the tx). Default:
    /// unsupported (custom backends).
    fn native_transfer(
        &mut self,
        _from: AccountId,
        _to: AccountId,
        _amount: u128,
    ) -> Result<(), VmError> {
        Err(VmError::NativeValueNotSupported)
    }
}

impl Interpreter {
    pub fn new(code: Vec<u8>, gas_limit: u64) -> Self {
        Self {
            code,
            pc: 0,
            stack: Vec::new(),
            memory: Vec::new(),
            gas_used: 0,
            gas_limit,
            return_data: None,
            logs: Vec::new(),
            caller_id: AccountId([0u8; 32]),
            contract_id: AccountId([0u8; 32]),
            exec_ctx: VmExecutionContext::default(),
            call_value: 0,
        }
    }

    fn append_log(&mut self, topics: Vec<[u8; 32]>, data: Vec<u8>) -> Result<(), VmError> {
        if topics.len() > MAX_EXECUTION_LOG_TOPICS {
            return Err(VmError::InvalidLog("too many topics"));
        }
        if data.len() > MAX_EXECUTION_LOG_DATA_BYTES {
            return Err(VmError::InvalidLog("log data too large"));
        }
        if self.logs.len() >= MAX_EXECUTION_LOGS_PER_TX {
            return Err(VmError::InvalidLog("too many logs"));
        }
        let log_gas = gas::LOG_BASE
            .saturating_add(gas::LOG_PER_TOPIC.saturating_mul(topics.len() as u64))
            .saturating_add(gas::LOG_PER_DATA_BYTE.saturating_mul(data.len() as u64));
        self.spend_gas(log_gas)?;
        self.logs.push(ExecutionLog { topics, data });
        Ok(())
    }

    fn log_with_topics(&mut self, n: usize) -> Result<(), VmError> {
        let offset = Self::u256_to_usize(&self.pop()?);
        let size = Self::u256_to_usize(&self.pop()?);
        let mut topics = Vec::with_capacity(n);
        for _ in 0..n {
            topics.push(self.pop()?);
        }
        topics.reverse();
        self.ensure_memory(offset, size)?;
        let data = self.memory[offset..offset.saturating_add(size)].to_vec();
        self.append_log(topics, data)
    }

    fn spend_gas(&mut self, amount: u64) -> Result<(), VmError> {
        self.gas_used = self.gas_used.saturating_add(amount);
        if self.gas_used > self.gas_limit {
            return Err(VmError::OutOfGas);
        }
        Ok(())
    }

    fn pop(&mut self) -> Result<[u8; 32], VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn push(&mut self, value: [u8; 32]) {
        self.stack.push(value);
    }

    fn ensure_memory(&mut self, offset: usize, size: usize) -> Result<(), VmError> {
        let end = offset
            .checked_add(size)
            .ok_or(VmError::MemoryLimitExceeded)?;
        if end > self.memory.len() {
            if end > crate::vm::MAX_VM_MEMORY_BYTES {
                return Err(VmError::MemoryLimitExceeded);
            }
            if self.exec_ctx.vm_hardening {
                let old_words = self.memory.len().div_ceil(32) as u64;
                let new_words = end.div_ceil(32) as u64;
                let cost = memory_gas_cost(new_words).saturating_sub(memory_gas_cost(old_words));
                self.spend_gas(cost)?;
            }
            self.memory.resize(end, 0);
        }
        Ok(())
    }

    fn u256_to_usize(v: &[u8; 32]) -> usize {
        let mut n: u64 = 0;
        for (i, &b) in v.iter().rev().take(8).enumerate() {
            n |= (b as u64) << (i * 8);
        }
        n as usize
    }

    fn add_u256(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
        let mut carry = 0u16;
        let mut out = [0u8; 32];
        for i in (0..32).rev() {
            let s = (a[i] as u16) + (b[i] as u16) + carry;
            out[i] = s as u8;
            carry = s >> 8;
        }
        out
    }

    fn sub_u256(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
        let mut borrow = 0i32;
        let mut out = [0u8; 32];
        for i in (0..32).rev() {
            let diff = (a[i] as i32) - (b[i] as i32) - borrow;
            borrow = if diff < 0 { 1 } else { 0 };
            out[i] = diff.wrapping_rem(256) as u8;
        }
        out
    }

    /// **256×256 → 256** multiplication (low limb; same as **MulMod**’s product width before reduce).
    fn mul_u256(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
        let prod = u256_be_to_biguint(a) * u256_be_to_biguint(b);
        biguint_low_u256_be(prod)
    }

    fn word_one() -> [u8; 32] {
        let mut w = [0u8; 32];
        w[31] = 1;
        w
    }

    fn u256_shl1_be(r: &mut [u8; 32]) {
        let mut carry = 0u8;
        for i in (0..32).rev() {
            let new_carry = r[i] >> 7;
            r[i] = (r[i] << 1) | carry;
            carry = new_carry;
        }
    }

    fn get_bit_msb_first(a: &[u8; 32], i: u32) -> u8 {
        let bi = i as usize;
        let byte_i = bi / 8;
        let bit_in_byte = 7 - (bi % 8);
        (a[byte_i] >> bit_in_byte) & 1
    }

    fn set_bit_msb_first(q: &mut [u8; 32], i: u32) {
        let bi = i as usize;
        let byte_i = bi / 8;
        let bit_in_byte = 7 - (bi % 8);
        q[byte_i] |= 1 << bit_in_byte;
    }

    /// Full unsigned 256-bit division and remainder (big-endian words). `b == 0` → error.
    fn div_mod_u256_be(a: [u8; 32], b: [u8; 32]) -> Result<([u8; 32], [u8; 32]), VmError> {
        if b == [0u8; 32] {
            return Err(VmError::DivisionByZero);
        }
        let mut q = [0u8; 32];
        let mut r = [0u8; 32];
        for i in 0..256u32 {
            Self::u256_shl1_be(&mut r);
            if Self::get_bit_msb_first(&a, i) != 0 {
                r[31] |= 1;
            }
            if r.cmp(&b) != Ordering::Less {
                r = Self::sub_u256(&r, &b);
                Self::set_bit_msb_first(&mut q, i);
            }
        }
        Ok((q, r))
    }

    /// Execute until STOP or RETURN. Returns gas used.
    ///
    /// **Top-level contract call:** `caller_id` is `tx.sender`. **Nested [`Opcode::Call`]:** `caller_id` is
    /// the contract that issued `CALL` so [`Opcode::Caller`] in the callee matches that account (e.g. pool
    /// calling a reference-token contract).
    pub fn run<S: StorageAccess>(
        &mut self,
        caller_id: AccountId,
        contract_id: AccountId,
        calldata: &[u8],
        storage: &mut S,
    ) -> Result<u64, VmError> {
        let default_reg = RuleRegistry::new();
        self.run_nested(
            caller_id,
            contract_id,
            calldata,
            storage,
            &default_reg,
            0,
            VmExecutionContext::default(),
        )
    }

    /// Same as [`Self::run`], but uses `qa_registry` for in-contract [`Opcode::Create2`] (must match mempool / block executor policy).
    pub fn run_with_qa<S: StorageAccess>(
        &mut self,
        caller_id: AccountId,
        contract_id: AccountId,
        calldata: &[u8],
        storage: &mut S,
        qa_registry: &RuleRegistry,
        exec_ctx: VmExecutionContext,
    ) -> Result<u64, VmError> {
        self.run_nested(
            caller_id,
            contract_id,
            calldata,
            storage,
            qa_registry,
            0,
            exec_ctx,
        )
    }

    pub(crate) fn run_nested<S: StorageAccess>(
        &mut self,
        caller_id: AccountId,
        contract_id: AccountId,
        calldata: &[u8],
        storage: &mut S,
        qa_registry: &RuleRegistry,
        call_depth: u8,
        exec_ctx: VmExecutionContext,
    ) -> Result<u64, VmError> {
        self.caller_id = caller_id;
        self.contract_id = contract_id;
        self.exec_ctx = exec_ctx;
        self.logs.clear();
        self.ensure_memory(0, calldata.len())?;
        self.memory[..calldata.len()].copy_from_slice(calldata);

        while self.pc < self.code.len() {
            // Each instruction pushes at most one word, so checking before every instruction keeps
            // the stack within MAX_STACK_DEPTH + 1.
            if self.exec_ctx.vm_hardening && self.stack.len() > MAX_STACK_DEPTH {
                return Err(VmError::StackOverflow);
            }
            let op = self.code[self.pc];
            self.pc += 1;

            if let Some(push_len) = Opcode::push_size(op) {
                self.spend_gas(gas::PUSH)?;
                let len = push_len as usize;
                if self.pc + len > self.code.len() {
                    return Err(VmError::InvalidBytecode);
                }
                let mut val = [0u8; 32];
                let start = 32 - len;
                val[start..].copy_from_slice(&self.code[self.pc..self.pc + len]);
                self.stack.push(val);
                self.pc += len;
                continue;
            }

            let opcode = Opcode::from_byte(op).ok_or(VmError::InvalidBytecode)?;

            match opcode {
                Opcode::Stop => {
                    self.spend_gas(gas::STOP)?;
                    break;
                }
                Opcode::Add => {
                    self.spend_gas(gas::ADD)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Self::add_u256(&a, &b));
                }
                Opcode::Sub => {
                    self.spend_gas(gas::SUB)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Self::sub_u256(&a, &b));
                }
                Opcode::Mul => {
                    self.spend_gas(gas::MUL)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Self::mul_u256(&a, &b));
                }
                Opcode::Div => {
                    self.spend_gas(gas::DIV)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let (q, _) = Self::div_mod_u256_be(a, b)?;
                    self.push(q);
                }
                Opcode::Mod => {
                    self.spend_gas(gas::MOD)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let (_, rem) = Self::div_mod_u256_be(a, b)?;
                    self.push(rem);
                }
                Opcode::AddMod => {
                    self.spend_gas(gas::ADDMOD)?;
                    let n = self.pop()?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if n == [0u8; 32] {
                        self.push([0u8; 32]);
                    } else {
                        let sum = u256_be_to_biguint(&a) + u256_be_to_biguint(&b);
                        let rem = sum % u256_be_to_biguint(&n);
                        self.push(biguint_rem_to_u256_be(rem));
                    }
                }
                Opcode::MulMod => {
                    self.spend_gas(gas::MULMOD)?;
                    let n = self.pop()?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if n == [0u8; 32] {
                        self.push([0u8; 32]);
                    } else {
                        let prod = u256_be_to_biguint(&a) * u256_be_to_biguint(&b);
                        let rem = prod % u256_be_to_biguint(&n);
                        self.push(biguint_rem_to_u256_be(rem));
                    }
                }
                Opcode::Lt => {
                    self.spend_gas(gas::CMP)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let out = if a.cmp(&b) == Ordering::Less {
                        Self::word_one()
                    } else {
                        [0u8; 32]
                    };
                    self.push(out);
                }
                Opcode::Gt => {
                    self.spend_gas(gas::CMP)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let out = if a.cmp(&b) == Ordering::Greater {
                        Self::word_one()
                    } else {
                        [0u8; 32]
                    };
                    self.push(out);
                }
                Opcode::Eq => {
                    self.spend_gas(gas::CMP)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let out = if a == b { Self::word_one() } else { [0u8; 32] };
                    self.push(out);
                }
                Opcode::IsZero => {
                    self.spend_gas(gas::ISZERO)?;
                    let a = self.pop()?;
                    let out = if a == [0u8; 32] {
                        Self::word_one()
                    } else {
                        [0u8; 32]
                    };
                    self.push(out);
                }
                Opcode::And => {
                    self.spend_gas(gas::BITWISE)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let mut out = [0u8; 32];
                    for i in 0..32 {
                        out[i] = a[i] & b[i];
                    }
                    self.push(out);
                }
                Opcode::Or => {
                    self.spend_gas(gas::BITWISE)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let mut out = [0u8; 32];
                    for i in 0..32 {
                        out[i] = a[i] | b[i];
                    }
                    self.push(out);
                }
                Opcode::Xor => {
                    self.spend_gas(gas::BITWISE)?;
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let mut out = [0u8; 32];
                    for i in 0..32 {
                        out[i] = a[i] ^ b[i];
                    }
                    self.push(out);
                }
                Opcode::Not => {
                    self.spend_gas(gas::BITWISE)?;
                    let a = self.pop()?;
                    let mut out = [0u8; 32];
                    for i in 0..32 {
                        out[i] = !a[i];
                    }
                    self.push(out);
                }
                Opcode::Shl => {
                    self.spend_gas(gas::SHIFT)?;
                    let shift_w = self.pop()?;
                    let value = self.pop()?;
                    let s = effective_shift_count(&shift_w);
                    let bu = u256_be_to_biguint(&value);
                    let m = BigUint::from(1u8) << 256u32;
                    let out = biguint_rem_to_u256_be((bu << s) % m);
                    self.push(out);
                }
                Opcode::Shr => {
                    self.spend_gas(gas::SHIFT)?;
                    let shift_w = self.pop()?;
                    let value = self.pop()?;
                    let s = effective_shift_count(&shift_w);
                    let bu = u256_be_to_biguint(&value);
                    self.push(biguint_rem_to_u256_be(bu >> s));
                }
                Opcode::Sar => {
                    self.spend_gas(gas::SHIFT)?;
                    let shift_w = self.pop()?;
                    let value = self.pop()?;
                    let s = effective_shift_count(&shift_w);
                    let i = BigInt::from_signed_bytes_be(&value[..]);
                    let shifted = i >> s;
                    self.push(signed_bigint_to_i256_word(&shifted));
                }
                Opcode::MLoad => {
                    self.spend_gas(gas::MLOAD)?;
                    let offset = Self::u256_to_usize(&self.pop()?);
                    self.ensure_memory(offset, 32)?;
                    let mut val = [0u8; 32];
                    val.copy_from_slice(&self.memory[offset..offset + 32]);
                    self.push(val);
                }
                Opcode::MStore => {
                    self.spend_gas(gas::MSTORE)?;
                    let offset = Self::u256_to_usize(&self.pop()?);
                    let value = self.pop()?;
                    self.ensure_memory(offset, 32)?;
                    self.memory[offset..offset + 32].copy_from_slice(&value);
                }
                Opcode::Dup1 => {
                    self.spend_gas(gas::DUP1)?;
                    let v = *self.stack.last().ok_or(VmError::StackUnderflow)?;
                    self.push(v);
                }
                Opcode::Address => {
                    self.spend_gas(gas::ADDRESS)?;
                    self.push(self.contract_id.0);
                }
                Opcode::BlockHeight => {
                    self.spend_gas(gas::BLOCKCTX)?;
                    self.push(word_u64_env(self.exec_ctx.block_height));
                }
                Opcode::Timestamp => {
                    self.spend_gas(gas::BLOCKCTX)?;
                    self.push(word_u64_env(self.exec_ctx.block_timestamp));
                }
                Opcode::Caller => {
                    self.spend_gas(gas::CALLER)?;
                    self.push(self.caller_id.0);
                }
                Opcode::Log0 => self.log_with_topics(0)?,
                Opcode::Log1 => self.log_with_topics(1)?,
                Opcode::Log2 => self.log_with_topics(2)?,
                Opcode::Log3 => self.log_with_topics(3)?,
                Opcode::Log4 => self.log_with_topics(4)?,
                Opcode::SLoad => {
                    self.spend_gas(gas::SLOAD)?;
                    let key = self.pop()?;
                    let value = storage.sload(contract_id, key);
                    self.push(value);
                }
                Opcode::SStore => {
                    self.spend_gas(gas::SSTORE)?;
                    let key = self.pop()?;
                    let value = self.pop()?;
                    storage.sstore(contract_id, key, value);
                }
                Opcode::Jump => {
                    self.spend_gas(gas::JUMP)?;
                    let dest = Self::u256_to_usize(&self.pop()?);
                    if dest >= self.code.len() {
                        return Err(VmError::InvalidJump);
                    }
                    self.pc = dest;
                }
                Opcode::JumpI => {
                    self.spend_gas(gas::JUMPI)?;
                    let dest = Self::u256_to_usize(&self.pop()?);
                    let cond = self.pop()?;
                    let is_nonzero = cond != [0u8; 32];
                    // Match Jump: a taken jump to an out-of-range destination faults
                    // (audit #61 item 29). Untaken jumps fall through.
                    if is_nonzero {
                        if dest >= self.code.len() {
                            return Err(VmError::InvalidJump);
                        }
                        self.pc = dest;
                    }
                }
                Opcode::Call => {
                    if call_depth >= MAX_CALL_DEPTH {
                        return Err(VmError::CallDepthExceeded);
                    }
                    self.spend_gas(gas::CALL)?;
                    let ret_size = Self::u256_to_usize(&self.pop()?);
                    let ret_offset = Self::u256_to_usize(&self.pop()?);
                    let args_size = Self::u256_to_usize(&self.pop()?);
                    let args_offset = Self::u256_to_usize(&self.pop()?);
                    let target_word = self.pop()?;
                    if args_size > MAX_RECEIPT_RETURN_DATA_BYTES
                        || ret_size > MAX_RECEIPT_RETURN_DATA_BYTES
                    {
                        return Err(VmError::CallBufferTooLarge);
                    }
                    self.ensure_memory(args_offset, args_size)?;
                    self.ensure_memory(ret_offset, ret_size)?;
                    let calldata_slice = &self.memory[args_offset..args_offset + args_size];
                    let target = AccountId(target_word);
                    let child_code = match storage.get_contract_code(target) {
                        Some(c) if !c.is_empty() => c,
                        _ => {
                            self.memory[ret_offset..ret_offset + ret_size].fill(0);
                            self.push(Self::word_one());
                            continue;
                        }
                    };
                    let remaining = self.gas_limit.saturating_sub(self.gas_used);
                    if remaining == 0 {
                        return Err(VmError::OutOfGas);
                    }
                    let child_caller = self.contract_id;
                    let mut child = Interpreter::new(child_code, remaining);
                    child.run_nested(
                        child_caller,
                        target,
                        calldata_slice,
                        storage,
                        qa_registry,
                        call_depth.saturating_add(1),
                        exec_ctx,
                    )?;
                    self.gas_used = self.gas_used.saturating_add(child.gas_used);
                    Self::merge_child_logs(&mut self.logs, &child)?;
                    let ret = child.return_data.unwrap_or_default();
                    let n = ret.len().min(ret_size);
                    self.memory[ret_offset..ret_offset + n].copy_from_slice(&ret[..n]);
                    if n < ret_size {
                        self.memory[ret_offset + n..ret_offset + ret_size].fill(0);
                    }
                    self.push(Self::word_one());
                }
                Opcode::Create2 => {
                    // Constructor runs as an extra frame (depth + 1), like contract creation in EVM.
                    if call_depth.saturating_add(1) >= MAX_CALL_DEPTH {
                        return Err(VmError::CallDepthExceeded);
                    }
                    // Stack top first, matching EVM CREATE2: salt, size, offset.
                    let salt_w = self.pop()?;
                    let size = Self::u256_to_usize(&self.pop()?);
                    let offset = Self::u256_to_usize(&self.pop()?);
                    if size == 0 {
                        return Err(VmError::Create2InitCodeEmpty);
                    }
                    if size > boing_qa::DEFAULT_MAX_BYTECODE_SIZE {
                        return Err(VmError::CallBufferTooLarge);
                    }
                    let linear = gas::CREATE2_PER_INIT_BYTE.saturating_mul(size as u64);
                    self.spend_gas(gas::CREATE2.saturating_add(linear))?;
                    self.ensure_memory(offset, size)?;
                    let bytecode = self.memory[offset..offset + size].to_vec();
                    let deployer = self.contract_id;
                    let remaining = self.gas_limit.saturating_sub(self.gas_used);
                    let mut salt = [0u8; 32];
                    salt.copy_from_slice(&salt_w);
                    let (addr, g_init) = storage.create2_deploy(
                        qa_registry,
                        deployer,
                        salt,
                        bytecode,
                        remaining,
                        &mut self.logs,
                        call_depth,
                        exec_ctx,
                    )?;
                    self.gas_used = self.gas_used.saturating_add(g_init);
                    self.push(addr.0);
                }
                Opcode::Return => {
                    self.spend_gas(gas::RETURN)?;
                    let offset = Self::u256_to_usize(&self.pop()?);
                    let size = Self::u256_to_usize(&self.pop()?);
                    self.ensure_memory(offset, size)?;
                    self.return_data = Some(self.memory[offset..offset + size].to_vec());
                    break;
                }
                Opcode::Balance
                | Opcode::CallValue
                | Opcode::SelfBalance
                | Opcode::NativeTransfer
                    if !exec_ctx.native_value =>
                {
                    // Before the native-value activation these bytes are not opcodes.
                    return Err(VmError::InvalidBytecode);
                }
                Opcode::CallValue => {
                    self.spend_gas(gas::CALLVALUE)?;
                    self.push(word_u128(self.call_value));
                }
                Opcode::SelfBalance => {
                    self.spend_gas(gas::SELFBALANCE)?;
                    self.push(word_u128(storage.native_balance(contract_id)));
                }
                Opcode::Balance => {
                    self.spend_gas(gas::BALANCE)?;
                    let who = AccountId(self.pop()?);
                    self.push(word_u128(storage.native_balance(who)));
                }
                Opcode::NativeTransfer => {
                    self.spend_gas(gas::NATIVE_TRANSFER)?;
                    let to = AccountId(self.pop()?);
                    let amount_w = self.pop()?;
                    if amount_w[..16].iter().any(|b| *b != 0) {
                        return Err(VmError::NativeAmountExceedsU128);
                    }
                    let mut lo = [0u8; 16];
                    lo.copy_from_slice(&amount_w[16..]);
                    let amount = u128::from_be_bytes(lo);
                    storage.native_transfer(contract_id, to, amount)?;
                }
                Opcode::Push1 | Opcode::Push32 => {
                    unreachable!("handled above")
                }
            }
        }

        Ok(self.gas_used)
    }

    /// Append child logs without re-charging log gas (already accounted in `child.gas_used`).
    pub(crate) fn merge_child_logs(
        parent: &mut Vec<ExecutionLog>,
        child: &Interpreter,
    ) -> Result<(), VmError> {
        if parent.len() + child.logs.len() > MAX_EXECUTION_LOGS_PER_TX {
            return Err(VmError::InvalidLog("too many logs"));
        }
        for log in &child.logs {
            if log.topics.len() > MAX_EXECUTION_LOG_TOPICS {
                return Err(VmError::InvalidLog("too many topics"));
            }
            if log.data.len() > MAX_EXECUTION_LOG_DATA_BYTES {
                return Err(VmError::InvalidLog("log data too large"));
            }
        }
        parent.extend(child.logs.iter().cloned());
        Ok(())
    }
}

impl StorageAccess for StateStore {
    fn sload(&self, contract: AccountId, key: [u8; 32]) -> [u8; 32] {
        self.get_contract_storage(&contract, &key)
    }

    fn sstore(&mut self, contract: AccountId, key: [u8; 32], value: [u8; 32]) {
        // Journaled so checkpoints can undo it.
        self.merge_contract_storage(contract, key, value);
    }

    fn get_contract_code(&self, contract: AccountId) -> Option<Vec<u8>> {
        self.get_contract_code(&contract).cloned()
    }

    fn native_balance(&self, account: AccountId) -> u128 {
        self.get(&account).map(|s| s.balance).unwrap_or(0)
    }

    fn native_transfer(
        &mut self,
        from: AccountId,
        to: AccountId,
        amount: u128,
    ) -> Result<(), VmError> {
        crate::vm::move_native(self, from, to, amount)
    }

    fn create2_deploy(
        &mut self,
        qa_registry: &RuleRegistry,
        deployer: AccountId,
        salt: [u8; 32],
        bytecode: Vec<u8>,
        init_gas_limit: u64,
        parent_logs: &mut Vec<ExecutionLog>,
        parent_call_depth: u8,
        exec_ctx: VmExecutionContext,
    ) -> Result<(AccountId, u64), VmError> {
        crate::vm::apply_in_tx_create2(
            self,
            qa_registry,
            deployer,
            salt,
            bytecode,
            init_gas_limit,
            parent_logs,
            parent_call_depth,
            exec_ctx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::vm::VmError;

    /// Run `code` against an empty contract and return the VM result.
    fn run_code(code: Vec<u8>) -> Result<u64, VmError> {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        Interpreter::new(code, 10_000_000).run(contract, contract, &[], &mut state)
    }

    /// PUSH32 `value`, PUSH32 `offset`, then the given opcode bytes.
    fn push32_pair(value: u8, offset: u128, tail: &[u8]) -> Vec<u8> {
        let mut code = vec![0x7f];
        let mut v = [0u8; 32];
        v[31] = value;
        code.extend_from_slice(&v);
        code.push(0x7f);
        let mut o = [0u8; 32];
        o[16..].copy_from_slice(&offset.to_be_bytes());
        code.extend_from_slice(&o);
        code.extend_from_slice(tail);
        code
    }

    #[test]
    fn mstore_beyond_memory_cap_fails_instead_of_allocating() {
        // 2^40 would try to allocate a terabyte before the cap.
        let r = run_code(push32_pair(1, 1u128 << 40, &[0x52, 0x00]));
        assert!(matches!(r, Err(VmError::MemoryLimitExceeded)), "{r:?}");
        // offset near u64::MAX: previously a capacity-overflow panic / wrapped slice index.
        let r = run_code(push32_pair(1, u64::MAX as u128, &[0x52, 0x00]));
        assert!(matches!(r, Err(VmError::MemoryLimitExceeded)), "{r:?}");
    }

    #[test]
    fn mload_and_return_beyond_cap_fail() {
        // MLOAD pops the offset (top of stack).
        let mut code = vec![0x7f];
        let mut o = [0u8; 32];
        o[24..].copy_from_slice(&(u64::MAX - 8).to_be_bytes());
        code.extend_from_slice(&o);
        code.extend_from_slice(&[0x51, 0x00]);
        assert!(matches!(run_code(code), Err(VmError::MemoryLimitExceeded)));
        // RETURN pops offset then size: huge size at offset 0.
        let mut code = vec![0x7f];
        let mut size = [0u8; 32];
        size[24..].copy_from_slice(&(1u64 << 40).to_be_bytes());
        code.extend_from_slice(&size);
        code.extend_from_slice(&[0x60, 0x00, 0xf3]);
        assert!(matches!(run_code(code), Err(VmError::MemoryLimitExceeded)));
    }

    /// Run `code` with the VM hardening flag on or off; returns gas used.
    fn run_hardened(code: Vec<u8>, gas_limit: u64, hardened: bool) -> Result<u64, VmError> {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        let ctx = VmExecutionContext {
            vm_hardening: hardened,
            ..Default::default()
        };
        Interpreter::new(code, gas_limit).run_with_qa(
            contract,
            contract,
            &[],
            &mut state,
            &RuleRegistry::new(),
            ctx,
        )
    }

    fn pushes(n: usize) -> Vec<u8> {
        let mut code = Vec::with_capacity(n * 2 + 1);
        for _ in 0..n {
            code.extend_from_slice(&[0x60, 0x00]);
        }
        code.push(0x00);
        code
    }

    /// Audit #61 item 23: before the gate the stack is unbounded (historical); from it, > 1024 fails.
    #[test]
    fn stack_depth_cap_is_gated() {
        assert!(run_hardened(pushes(MAX_STACK_DEPTH + 50), 10_000_000, false).is_ok());
        assert!(matches!(
            run_hardened(pushes(MAX_STACK_DEPTH + 50), 10_000_000, true),
            Err(VmError::StackOverflow)
        ));
        assert!(run_hardened(pushes(MAX_STACK_DEPTH), 10_000_000, true).is_ok());
    }

    /// Audit #61 item 22: memory expansion costs gas only from the gate, on the EVM schedule.
    #[test]
    fn memory_expansion_gas_is_gated() {
        let offset = 1u128 << 20; // 1 MiB
        let code = push32_pair(1, offset, &[0x52, 0x00]);
        let plain = run_hardened(code.clone(), 10_000_000, false).unwrap();
        let hard = run_hardened(code, 10_000_000, true).unwrap();
        let words = ((offset as u64) + 32).div_ceil(32);
        assert_eq!(hard - plain, memory_gas_cost(words));
        // 4 MiB: within the 8 MiB hard cap, but the quadratic cost exceeds a 10M gas budget.
        let code = push32_pair(1, 4u128 << 20, &[0x52, 0x00]);
        assert!(run_hardened(code.clone(), 10_000_000, false).is_ok());
        assert!(matches!(
            run_hardened(code, 10_000_000, true),
            Err(VmError::OutOfGas)
        ));
        // Re-touching already paid memory is free.
        assert_eq!(memory_gas_cost(0), 0);
        assert_eq!(memory_gas_cost(1), 3);
        assert_eq!(memory_gas_cost(1024), 3 * 1024 + 2048);
    }

    #[test]
    fn mstore_within_cap_still_works() {
        let r = run_code(push32_pair(7, 4096, &[0x52, 0x00]));
        assert!(r.is_ok(), "{r:?}");
    }

    #[test]
    fn bigint_sar_neg8_two_complement_word() {
        let mut neg8 = [0xffu8; 32];
        neg8[31] = 0xf8;
        let i = BigInt::from_signed_bytes_be(&neg8);
        assert_eq!(i, BigInt::from(-8));
        let shifted = i >> 2u32;
        assert_eq!(shifted, BigInt::from(-2), "got {:?}", shifted);
        let w = signed_bigint_to_i256_word(&shifted);
        let mut exp = [0xffu8; 32];
        exp[31] = 0xfe;
        assert_eq!(w, exp);
    }

    #[test]
    fn test_interpreter_lt_and_iszero() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        // PUSH1 2, PUSH1 5, LT -> 2 < 5 -> 1; then PUSH1 0, ISZERO -> 1
        let code = vec![0x60, 2, 0x60, 5, 0x10, 0x60, 0, 0x15, 0x00];
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it.stack.len(), 2);
        assert_eq!(it.stack[0][31], 1); // LT
        assert_eq!(it.stack[1][31], 1); // ISZERO(0)
    }

    #[test]
    fn test_interpreter_div_mod_small() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        // 7 / 3 = 2; 7 % 3 = 1 (dividend then divisor on stack so top = divisor)
        let code = vec![0x60, 7, 0x60, 3, 0x04, 0x60, 7, 0x60, 3, 0x06, 0x00];
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it.stack.len(), 2);
        assert_eq!(it.stack[0][31], 2);
        assert_eq!(it.stack[1][31], 1);
    }

    #[test]
    fn test_interpreter_mul_small() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        // 6 * 7 = 42 (stack top = 7 = b, next = 6 = a)
        let code = vec![0x60, 6, 0x60, 7, 0x03, 0x00];
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it.stack.len(), 1);
        assert_eq!(it.stack[0][31], 42);
    }

    #[test]
    fn test_interpreter_mul_exceeds_u64_product() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        // (2^63) * 16 = 2^67 — old u64×u64 Mul would saturate wrong; full Mul keeps low 256 bits.
        let mut a = [0u8; 32];
        a[24..32].copy_from_slice(&(1u64 << 63).to_be_bytes());
        let mut b = [0u8; 32];
        b[31] = 16;
        let mut code = vec![0x7f];
        code.extend_from_slice(&a);
        code.push(0x7f);
        code.extend_from_slice(&b);
        code.extend([0x03, 0x00]); // MUL
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        let mut want = [0u8; 32];
        want[16..32].copy_from_slice(&(1u128 << 67).to_be_bytes());
        assert_eq!(it.stack[0], want);
    }

    #[test]
    fn test_interpreter_mul_wraps_at_256_bits() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        let mut two_pow_255 = [0u8; 32];
        two_pow_255[0] = 0x80; // 2^255 in BE
        let mut two = [0u8; 32];
        two[31] = 2;
        let mut code = vec![0x7f];
        code.extend_from_slice(&two_pow_255);
        code.push(0x7f);
        code.extend_from_slice(&two);
        code.extend([0x03, 0x00]);
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it.stack[0], [0u8; 32], "2^255 * 2 = 2^256 ≡ 0 (mod 2^256)");
    }

    #[test]
    fn test_interpreter_addmod_mulmod() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        // (10+20)%7=2; (3*4)%5=2; mod 0 -> 0
        let code = vec![
            0x60, 10, 0x60, 20, 0x60, 7, 0x08, // ADDMOD
            0x60, 3, 0x60, 4, 0x60, 5, 0x09, // MULMOD
            0x60, 1, 0x60, 2, 0x60, 0, 0x08, // ADDMOD mod 0 -> 0
            0x00,
        ];
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it.stack.len(), 3);
        assert_eq!(it.stack[0][31], 2);
        assert_eq!(it.stack[1][31], 2);
        assert_eq!(it.stack[2], [0u8; 32]);
    }

    #[test]
    fn test_interpreter_addmod_overflow_before_mod() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        // (2^256-1 + 1) % 7 == 2^256 % 7 == 2
        let mut code = vec![0x7f];
        code.extend([0xffu8; 32]);
        code.extend([0x60, 1, 0x60, 7, 0x08, 0x00]);
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it.stack.len(), 1);
        assert_eq!(it.stack[0][31], 2);
    }

    #[test]
    fn test_interpreter_shl_shr_sar_smoke() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        // value=1, shift=3 (PUSH1 3 as shift word) -> SHL -> 8
        let code = vec![0x60, 1, 0x60, 3, 0x1b, 0x00];
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it.stack[0][31], 8);

        // SHR: 16 >> 2 = 4
        let code2 = vec![0x60, 16, 0x60, 2, 0x1c, 0x00];
        let mut it2 = Interpreter::new(code2, 1_000_000);
        it2.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it2.stack[0][31], 4);

        // SAR: -8 (i256) >> 2 = -2 → two's complement word
        let mut neg8 = [0xffu8; 32];
        neg8[31] = 0xf8;
        let mut code3 = Vec::new();
        code3.push(0x7f);
        code3.extend_from_slice(&neg8);
        code3.extend([0x60, 2, 0x1d, 0x00]);
        let mut it3 = Interpreter::new(code3, 1_000_000);
        it3.run(contract, contract, &[], &mut state).unwrap();
        let mut neg2 = [0xffu8; 32];
        neg2[31] = 0xfe;
        assert_eq!(it3.stack[0], neg2);
    }

    #[test]
    fn test_interpreter_div_by_zero() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        let code = vec![0x60, 1, 0x60, 0, 0x04, 0x00];
        let mut it = Interpreter::new(code, 1_000_000);
        assert!(matches!(
            it.run(contract, contract, &[], &mut state),
            Err(VmError::DivisionByZero)
        ));
    }

    #[test]
    fn test_interpreter_add_stop() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState {
                balance: 0,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let bytecode = vec![
            0x60, 0x02, // PUSH1 2
            0x60, 0x03, // PUSH1 3
            0x01, // ADD -> 5
            0x00, // STOP
        ];
        state.set_contract_code(contract, bytecode.clone());
        let mut interpreter = Interpreter::new(bytecode, 1000);
        let gas = interpreter
            .run(contract, contract, &[], &mut state)
            .unwrap();
        assert!(gas > 0);
        assert_eq!(interpreter.stack.len(), 1);
        assert_eq!(interpreter.stack[0][31], 5); // low byte = 5
    }

    /// V6 follow-up: small-value matrix for compare opcodes (stack: push `a`, push `b`, op — top is `b`).
    #[test]
    fn test_interpreter_compare_opcode_matrix() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        for a in 0u8..=3u8 {
            for b in 0u8..=3u8 {
                for (op, want) in [
                    (0x10u8, u8::from(a < b)),
                    (0x11u8, u8::from(a > b)),
                    (0x14u8, u8::from(a == b)),
                ] {
                    let code = vec![0x60, a, 0x60, b, op, 0x00];
                    let mut it = Interpreter::new(code, 1_000_000);
                    it.run(contract, contract, &[], &mut state).unwrap();
                    assert_eq!(it.stack[0][31], want, "op={op:#x} a={a} b={b}");
                }
            }
        }
    }

    #[test]
    fn test_interpreter_iszero_and_bitwise_samples() {
        let mut state = StateStore::new();
        let contract = AccountId([1u8; 32]);
        state.insert(boing_primitives::Account {
            id: contract,
            state: boing_primitives::AccountState::default(),
        });
        // ISZERO(0) -> 1; ISZERO(1) -> 0 (stack grows up: index 0 = older value)
        let code = vec![0x60, 0, 0x15, 0x60, 1, 0x15, 0x00];
        let mut it = Interpreter::new(code, 1_000_000);
        it.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it.stack[0][31], 1);
        assert_eq!(it.stack[1][31], 0);

        let code2 = vec![0x60, 0x0f, 0x60, 0xf0, 0x16, 0x00]; // AND
        let mut it2 = Interpreter::new(code2, 1_000_000);
        it2.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it2.stack[0][31], 0);

        let code3 = vec![0x60, 0x0f, 0x60, 0xf0, 0x17, 0x00]; // OR
        let mut it3 = Interpreter::new(code3, 1_000_000);
        it3.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it3.stack[0][31], 0xff);

        let code4 = vec![0x60, 0x0f, 0x60, 0xff, 0x18, 0x00]; // XOR
        let mut it4 = Interpreter::new(code4, 1_000_000);
        it4.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it4.stack[0][31], 0xf0);

        let code5 = vec![0x60, 0, 0x19, 0x00]; // NOT
        let mut it5 = Interpreter::new(code5, 1_000_000);
        it5.run(contract, contract, &[], &mut state).unwrap();
        assert_eq!(it5.stack[0], [0xffu8; 32]);
    }
}
