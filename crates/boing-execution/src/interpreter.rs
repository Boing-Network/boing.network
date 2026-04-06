//! Boing VM interpreter — deterministic stack machine.
//!
//! Executes bytecode with gas metering.

use std::cmp::Ordering;

use boing_primitives::{
    AccountId, ExecutionLog, MAX_EXECUTION_LOG_DATA_BYTES, MAX_EXECUTION_LOGS_PER_TX,
    MAX_EXECUTION_LOG_TOPICS, MAX_RECEIPT_RETURN_DATA_BYTES,
};
use boing_qa::RuleRegistry;
use boing_state::StateStore;
use num_bigint::{BigInt, BigUint, Sign};

use super::bytecode::{gas, Opcode};
use super::vm::VmError;

/// Maximum nested [`Opcode::Call`] depth (root frame = 0).
pub const MAX_CALL_DEPTH: u8 = 64;

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
    ) -> Result<(AccountId, u64), VmError> {
        Err(VmError::Create2NotSupported)
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
        self.ensure_memory(offset, size);
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

    fn ensure_memory(&mut self, offset: usize, size: usize) {
        let end = offset.saturating_add(size);
        if end > self.memory.len() {
            self.memory.resize(end, 0);
        }
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
        self.run_nested(caller_id, contract_id, calldata, storage, &default_reg, 0)
    }

    /// Same as [`Self::run`], but uses `qa_registry` for in-contract [`Opcode::Create2`] (must match mempool / block executor policy).
    pub fn run_with_qa<S: StorageAccess>(
        &mut self,
        caller_id: AccountId,
        contract_id: AccountId,
        calldata: &[u8],
        storage: &mut S,
        qa_registry: &RuleRegistry,
    ) -> Result<u64, VmError> {
        self.run_nested(caller_id, contract_id, calldata, storage, qa_registry, 0)
    }

    pub(crate) fn run_nested<S: StorageAccess>(
        &mut self,
        caller_id: AccountId,
        contract_id: AccountId,
        calldata: &[u8],
        storage: &mut S,
        qa_registry: &RuleRegistry,
        call_depth: u8,
    ) -> Result<u64, VmError> {
        self.caller_id = caller_id;
        self.contract_id = contract_id;
        self.logs.clear();
        self.ensure_memory(0, calldata.len());
        self.memory[..calldata.len()].copy_from_slice(calldata);

        while self.pc < self.code.len() {
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
                    self.ensure_memory(offset, 32);
                    let mut val = [0u8; 32];
                    val.copy_from_slice(&self.memory[offset..offset + 32]);
                    self.push(val);
                }
                Opcode::MStore => {
                    self.spend_gas(gas::MSTORE)?;
                    let offset = Self::u256_to_usize(&self.pop()?);
                    let value = self.pop()?;
                    self.ensure_memory(offset, 32);
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
                    if is_nonzero && dest < self.code.len() {
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
                    self.ensure_memory(args_offset, args_size);
                    self.ensure_memory(ret_offset, ret_size);
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
                    self.ensure_memory(offset, size);
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
                    )?;
                    self.gas_used = self.gas_used.saturating_add(g_init);
                    self.push(addr.0);
                }
                Opcode::Return => {
                    self.spend_gas(gas::RETURN)?;
                    let offset = Self::u256_to_usize(&self.pop()?);
                    let size = Self::u256_to_usize(&self.pop()?);
                    self.ensure_memory(offset, size);
                    self.return_data = Some(self.memory[offset..offset + size].to_vec());
                    break;
                }
                Opcode::Push1 | Opcode::Push32 => {
                    unreachable!("handled above")
                }
            }
        }

        Ok(self.gas_used)
    }

    /// Append child logs without re-charging log gas (already accounted in `child.gas_used`).
    pub(crate) fn merge_child_logs(parent: &mut Vec<ExecutionLog>, child: &Interpreter) -> Result<(), VmError> {
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
        self.contract_storage
            .get(&(contract, key))
            .copied()
            .unwrap_or([0u8; 32])
    }

    fn sstore(&mut self, contract: AccountId, key: [u8; 32], value: [u8; 32]) {
        self.contract_storage.insert((contract, key), value);
    }

    fn get_contract_code(&self, contract: AccountId) -> Option<Vec<u8>> {
        self.get_contract_code(&contract).cloned()
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
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::vm::VmError;

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
            state: boing_primitives::AccountState { balance: 0, nonce: 0, stake: 0 },
        });
        let bytecode = vec![
            0x60, 0x02, // PUSH1 2
            0x60, 0x03, // PUSH1 3
            0x01,       // ADD -> 5
            0x00,       // STOP
        ];
        state.set_contract_code(contract, bytecode.clone());
        let mut interpreter = Interpreter::new(bytecode, 1000);
        let gas = interpreter.run(contract, contract, &[], &mut state).unwrap();
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
                    assert_eq!(
                        it.stack[0][31],
                        want,
                        "op={op:#x} a={a} b={b}"
                    );
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
