//! Reference **multi-token** (ERC-1155-style, semi-fungible) template for wallets, games and indexers.
//!
//! Not consensus-enforced — see `docs/BOING-REFERENCE-MULTI-TOKEN.md`.
//!
//! One contract holds many **item types**. Every holder of item `7` holds the *same* item; the ledger is
//! `balance[item_id][account]` (u128), plus a per-item `total_supply` and a type-level 32-byte metadata
//! commitment set once per `item_id` (not once per copy).
//!
//! Deploy with [`reference_multi_token_deploy_bytecode`] (`0xFD` init): the init frame stores the deployer
//! (`CALLER`) as **admin** and returns [`reference_multi_token_runtime_bytecode`]. There is no lazy-admin
//! fallback, so nobody can front-run the first call. Purpose category: **`nft`** (existing QA category).
//!
//! No hashing opcode is used (the gated `BLAKE3` opcode stays off): storage keys are XOR-tagged words.

use boing_primitives::{AccountId, CONTRACT_DEPLOY_INIT_CODE_MARKER};

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// `balance_of(account, item_id)` → 32-byte word (u128 in the low 16 bytes). 96-byte calldata.
pub const SELECTOR_MT_BALANCE_OF: u8 = 0x20;
/// `transfer(to, item_id, amount)` — move `amount` of the caller's own balance. 128-byte calldata.
pub const SELECTOR_MT_TRANSFER: u8 = 0x21;
/// `mint(to, item_id, amount)` — admin only. 128-byte calldata.
pub const SELECTOR_MT_MINT: u8 = 0x22;
/// `set_item_metadata(item_id, hash)` — admin only; type-level 32-byte commitment. 96-byte calldata.
pub const SELECTOR_MT_SET_ITEM_METADATA: u8 = 0x23;
/// `mint_batch(to, n, item_ids[n], amounts[n])` — admin only. `96 + 64n` bytes.
pub const SELECTOR_MT_MINT_BATCH: u8 = 0x24;
/// `transfer_batch(to, n, item_ids[n], amounts[n])` — caller's own balances. `96 + 64n` bytes.
pub const SELECTOR_MT_TRANSFER_BATCH: u8 = 0x25;
/// `total_supply(item_id)` → 32-byte word. 64-byte calldata.
pub const SELECTOR_MT_TOTAL_SUPPLY: u8 = 0x26;
/// `item_metadata(item_id)` → 32-byte word. 64-byte calldata.
pub const SELECTOR_MT_ITEM_METADATA: u8 = 0x27;
/// `admin()` → 32-byte AccountId. 32-byte calldata.
pub const SELECTOR_MT_ADMIN: u8 = 0x28;
/// `transfer_admin(new_admin)` — admin only; `new_admin != 0`. 64-byte calldata.
pub const SELECTOR_MT_TRANSFER_ADMIN: u8 = 0x29;

/// Max entries per `mint_batch` / `transfer_batch`. Each entry emits one transfer log and the VM allows
/// `MAX_EXECUTION_LOGS_PER_TX = 24` logs per transaction.
pub const MAX_REFERENCE_MULTI_TOKEN_BATCH: u16 = 16;

/// Pinned template version (artifact id `boing.reference_multi_token.v1`).
pub const REFERENCE_MULTI_TOKEN_TEMPLATE_VERSION: u32 = 1;

const fn tag32(s: &[u8]) -> [u8; 32] {
    let mut o = [0u8; 32];
    let mut i = 0;
    while i < s.len() {
        o[i] = s[i];
        i += 1;
    }
    o
}

/// Balance slot: `(account[0..24] || item_id_be8) ^ REF_MT_BALANCE_XOR`.
pub const REF_MT_BALANCE_XOR: [u8; 32] = tag32(b"BOING_REFMT_BAL_V1");
/// Total-supply slot: `item_id_word ^ REF_MT_SUPPLY_XOR`.
pub const REF_MT_SUPPLY_XOR: [u8; 32] = tag32(b"BOING_REFMT_SUPPLY_V1");
/// Item metadata slot: `item_id_word ^ REF_MT_METADATA_XOR`.
pub const REF_MT_METADATA_XOR: [u8; 32] = tag32(b"BOING_REFMT_META_V1");
/// `LOG3` topic0 for every balance move: topics `[TOPIC, from, to]`, data `item_id_word || amount_word`.
/// Mints use `from = 0`.
pub const REF_MT_TOPIC_TRANSFER: [u8; 32] = tag32(b"BOING_REFMT_TRANSFER_V1");
/// `LOG2` topic0 for `set_item_metadata`: topics `[TOPIC, item_id_word]`, data `hash`.
pub const REF_MT_TOPIC_METADATA: [u8; 32] = tag32(b"BOING_REFMT_METADATA_V1");
/// `LOG3` topic0 for `transfer_admin`: topics `[TOPIC, old_admin, new_admin]`, no data.
pub const REF_MT_TOPIC_ADMIN: [u8; 32] = tag32(b"BOING_REFMT_ADMIN_V1");

/// Singleton admin slot (set by the init frame).
#[must_use]
pub fn ref_multi_token_admin_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xe0;
    k
}

fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

/// `item_id` as a 32-byte word (big-endian u64 in the low 8 bytes; high 24 bytes must be zero on-chain).
#[must_use]
pub fn multi_token_item_id_word(item_id: u64) -> [u8; 32] {
    word_u64(item_id)
}

/// `amount` as a 32-byte word (big-endian u128 in the low 16 bytes).
#[must_use]
pub fn multi_token_amount_word(amount: u128) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[16..32].copy_from_slice(&amount.to_be_bytes());
    w
}

fn xor32(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..32 {
        o[i] = a[i] ^ b[i];
    }
    o
}

/// Storage key for `balance[item_id][account]`.
#[must_use]
pub fn multi_token_balance_storage_key(account: &AccountId, item_id: u64) -> [u8; 32] {
    let mut raw = [0u8; 32];
    raw[..24].copy_from_slice(&account.0[..24]);
    raw[24..].copy_from_slice(&item_id.to_be_bytes());
    xor32(&raw, &REF_MT_BALANCE_XOR)
}

/// Storage key for `total_supply[item_id]`.
#[must_use]
pub fn multi_token_supply_storage_key(item_id: u64) -> [u8; 32] {
    xor32(&word_u64(item_id), &REF_MT_SUPPLY_XOR)
}

/// Storage key for `item_metadata[item_id]`.
#[must_use]
pub fn multi_token_metadata_storage_key(item_id: u64) -> [u8; 32] {
    xor32(&word_u64(item_id), &REF_MT_METADATA_XOR)
}

/// `balance_of(account, item_id)` calldata (96 bytes).
#[must_use]
pub fn encode_mt_balance_of_calldata(account: &AccountId, item_id: u64) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_MT_BALANCE_OF).to_vec();
    v.extend_from_slice(&account.0);
    v.extend_from_slice(&word_u64(item_id));
    v
}

fn encode_to_item_amount(sel: u8, to: &AccountId, item_id: u64, amount: u128) -> Vec<u8> {
    let mut v = selector_word(sel).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&word_u64(item_id));
    v.extend_from_slice(&multi_token_amount_word(amount));
    v
}

/// `transfer(to, item_id, amount)` calldata (128 bytes).
#[must_use]
pub fn encode_mt_transfer_calldata(to: &AccountId, item_id: u64, amount: u128) -> Vec<u8> {
    encode_to_item_amount(SELECTOR_MT_TRANSFER, to, item_id, amount)
}

/// `mint(to, item_id, amount)` calldata (128 bytes).
#[must_use]
pub fn encode_mt_mint_calldata(to: &AccountId, item_id: u64, amount: u128) -> Vec<u8> {
    encode_to_item_amount(SELECTOR_MT_MINT, to, item_id, amount)
}

/// `set_item_metadata(item_id, hash)` calldata (96 bytes).
#[must_use]
pub fn encode_mt_set_item_metadata_calldata(item_id: u64, metadata_hash: &[u8; 32]) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_MT_SET_ITEM_METADATA).to_vec();
    v.extend_from_slice(&word_u64(item_id));
    v.extend_from_slice(metadata_hash);
    v
}

fn encode_batch(sel: u8, to: &AccountId, entries: &[(u64, u128)]) -> Vec<u8> {
    let n = entries.len();
    assert!(n >= 1, "batch n must be >= 1");
    assert!(
        n <= MAX_REFERENCE_MULTI_TOKEN_BATCH as usize,
        "batch n exceeds MAX_REFERENCE_MULTI_TOKEN_BATCH"
    );
    let mut v = selector_word(sel).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&word_u64(n as u64));
    for (id, _) in entries {
        v.extend_from_slice(&word_u64(*id));
    }
    for (_, amt) in entries {
        v.extend_from_slice(&multi_token_amount_word(*amt));
    }
    v
}

/// `mint_batch(to, n, item_ids[n], amounts[n])` calldata (`96 + 64n`). Panics unless `1 <= n <= 16`.
#[must_use]
pub fn encode_mt_mint_batch_calldata(to: &AccountId, entries: &[(u64, u128)]) -> Vec<u8> {
    encode_batch(SELECTOR_MT_MINT_BATCH, to, entries)
}

/// `transfer_batch(to, n, item_ids[n], amounts[n])` calldata (`96 + 64n`). Panics unless `1 <= n <= 16`.
#[must_use]
pub fn encode_mt_transfer_batch_calldata(to: &AccountId, entries: &[(u64, u128)]) -> Vec<u8> {
    encode_batch(SELECTOR_MT_TRANSFER_BATCH, to, entries)
}

/// `total_supply(item_id)` calldata (64 bytes).
#[must_use]
pub fn encode_mt_total_supply_calldata(item_id: u64) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_MT_TOTAL_SUPPLY).to_vec();
    v.extend_from_slice(&word_u64(item_id));
    v
}

/// `item_metadata(item_id)` calldata (64 bytes).
#[must_use]
pub fn encode_mt_item_metadata_calldata(item_id: u64) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_MT_ITEM_METADATA).to_vec();
    v.extend_from_slice(&word_u64(item_id));
    v
}

/// `admin()` calldata (32 bytes).
#[must_use]
pub fn encode_mt_admin_calldata() -> Vec<u8> {
    selector_word(SELECTOR_MT_ADMIN).to_vec()
}

/// `transfer_admin(new_admin)` calldata (64 bytes).
#[must_use]
pub fn encode_mt_transfer_admin_calldata(new_admin: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_MT_TRANSFER_ADMIN).to_vec();
    v.extend_from_slice(&new_admin.0);
    v
}

// --- VM bytecode -----------------------------------------------------------------------------
// Calldata sits at mem[0..96+64*16 = 1120). Scratch words start at 2048.

const S_SEL: u64 = 2048;
const S_TO: u64 = 2080;
const S_ITEM: u64 = 2112;
const S_AMT: u64 = 2144;
const S_KEY: u64 = 2176;
const S_BAL: u64 = 2208;
const S_N: u64 = 2240;
const S_I: u64 = 2272;
const S_SKEY: u64 = 2304;
const S_RET: u64 = 2336;
/// 64-byte log data buffer.
const S_LOG: u64 = 2368;

struct Asm {
    c: Vec<u8>,
    faults: Vec<usize>,
}

impl Asm {
    fn op(&mut self, o: Opcode) {
        self.c.push(o as u8);
    }
    /// Shortest `PUSHn` for the word (leading zero bytes dropped; `PUSH1 0` for zero).
    fn push(&mut self, w: &[u8; 32]) {
        let first = w.iter().position(|b| *b != 0).unwrap_or(31);
        let n = 32 - first;
        self.c.push(Opcode::Push1 as u8 + (n as u8 - 1));
        self.c.extend_from_slice(&w[first..]);
    }
    fn push_u64(&mut self, n: u64) {
        self.push(&word_u64(n));
    }
    fn mload(&mut self, off: u64) {
        self.push_u64(off);
        self.op(Opcode::MLoad);
    }
    /// Store the stack top at `off`.
    fn mstore(&mut self, off: u64) {
        self.push_u64(off);
        self.op(Opcode::MStore);
    }
    fn copy(&mut self, from: u64, to: u64) {
        self.mload(from);
        self.mstore(to);
    }
    fn here(&self) -> usize {
        self.c.len()
    }
    /// `PUSH4 <dest>; JUMPI` with the destination patched later by [`Self::patch`].
    fn jumpi_fwd(&mut self) -> usize {
        let at = self.here();
        self.c.push(Opcode::Push1 as u8 + 3);
        self.c.extend_from_slice(&[0u8; 4]);
        self.op(Opcode::JumpI);
        at
    }
    fn jump_to(&mut self, dest: usize) {
        self.push_u64(dest as u64);
        self.op(Opcode::Jump);
    }
    fn patch(&mut self, at: usize, dest: usize) {
        let d = u32::try_from(dest).expect("bytecode offset fits in PUSH4");
        self.c[at + 1..at + 5].copy_from_slice(&d.to_be_bytes());
    }
    /// Consume the condition on the stack; fault (whole tx reverts) when non-zero.
    fn fault_if(&mut self) {
        let at = self.jumpi_fwd();
        self.faults.push(at);
    }
    fn require_nonzero(&mut self, off: u64) {
        self.mload(off);
        self.op(Opcode::IsZero);
        self.fault_if();
    }
    /// Fault unless `mem[off] < 2^bits`.
    fn require_fits(&mut self, off: u64, bits: u64) {
        self.mload(off);
        self.push_u64(bits);
        self.op(Opcode::Shr);
        self.op(Opcode::IsZero);
        self.op(Opcode::IsZero);
        self.fault_if();
    }
    fn require_admin(&mut self) {
        self.op(Opcode::Caller);
        self.push(&ref_multi_token_admin_key());
        self.op(Opcode::SLoad);
        self.op(Opcode::Eq);
        self.op(Opcode::IsZero);
        self.fault_if();
    }
    /// Account word on the stack → balance key for `mem[S_ITEM]` on the stack.
    fn balance_key_tail(&mut self) {
        self.push_u64(64);
        self.op(Opcode::Shr);
        self.push_u64(64);
        self.op(Opcode::Shl);
        self.mload(S_ITEM);
        self.op(Opcode::Or);
        self.push(&REF_MT_BALANCE_XOR);
        self.op(Opcode::Xor);
    }
    fn validate_item_amount(&mut self) {
        self.require_fits(S_ITEM, 64);
        self.require_fits(S_AMT, 128);
        self.require_nonzero(S_AMT);
    }
    /// `balance[S_ITEM][CALLER] -= mem[S_AMT]`, faulting on insufficient balance.
    fn debit_caller(&mut self) {
        self.op(Opcode::Caller);
        self.balance_key_tail();
        self.op(Opcode::Dup1);
        self.mstore(S_KEY);
        self.op(Opcode::SLoad);
        self.mstore(S_BAL);
        self.mload(S_BAL);
        self.mload(S_AMT);
        self.op(Opcode::Lt);
        self.fault_if();
        self.mload(S_BAL);
        self.mload(S_AMT);
        self.op(Opcode::Sub);
        self.mload(S_KEY);
        self.op(Opcode::SStore);
    }
    /// `balance[S_ITEM][mem[S_TO]] += mem[S_AMT]`, faulting if the result exceeds u128.
    fn credit_to(&mut self) {
        self.mload(S_TO);
        self.balance_key_tail();
        self.op(Opcode::Dup1);
        self.mstore(S_KEY);
        self.op(Opcode::SLoad);
        self.mload(S_AMT);
        self.op(Opcode::Add);
        self.mstore(S_BAL);
        self.require_fits(S_BAL, 128);
        self.mload(S_BAL);
        self.mload(S_KEY);
        self.op(Opcode::SStore);
    }
    /// `total_supply[S_ITEM] += mem[S_AMT]`, faulting if the result exceeds u128.
    fn supply_add(&mut self) {
        self.mload(S_ITEM);
        self.push(&REF_MT_SUPPLY_XOR);
        self.op(Opcode::Xor);
        self.mstore(S_SKEY);
        self.mload(S_SKEY);
        self.op(Opcode::SLoad);
        self.mload(S_AMT);
        self.op(Opcode::Add);
        self.mstore(S_BAL);
        self.require_fits(S_BAL, 128);
        self.mload(S_BAL);
        self.mload(S_SKEY);
        self.op(Opcode::SStore);
    }
    /// `LOG3 [TRANSFER, from, to]` with data `item || amount`. `from_is_caller == false` → zero (mint).
    fn log_transfer(&mut self, from_is_caller: bool) {
        self.copy(S_ITEM, S_LOG);
        self.copy(S_AMT, S_LOG + 32);
        self.push(&REF_MT_TOPIC_TRANSFER);
        if from_is_caller {
            self.op(Opcode::Caller);
        } else {
            self.push(&[0u8; 32]);
        }
        self.mload(S_TO);
        self.push_u64(64);
        self.push_u64(S_LOG);
        self.op(Opcode::Log3);
    }
    fn return_word(&mut self, off: u64) {
        self.push_u64(32);
        self.push_u64(off);
        self.op(Opcode::Return);
    }
}

fn emit_batch(a: &mut Asm, is_mint: bool) {
    a.copy(32, S_TO);
    a.copy(64, S_N);
    // 1 <= n <= MAX (Gt on the full word also rejects any high-byte garbage).
    a.require_nonzero(S_N);
    a.mload(S_N);
    a.push_u64(MAX_REFERENCE_MULTI_TOKEN_BATCH as u64);
    a.op(Opcode::Gt);
    a.fault_if();
    a.require_nonzero(S_TO);
    // Trailing non-zero word at 96 + 64n → malformed (length mismatch).
    a.mload(S_N);
    a.push_u64(64);
    a.op(Opcode::Mul);
    a.push_u64(96);
    a.op(Opcode::Add);
    a.op(Opcode::MLoad);
    a.op(Opcode::IsZero);
    a.op(Opcode::IsZero);
    a.fault_if();
    if is_mint {
        a.require_admin();
    }
    a.push_u64(0);
    a.mstore(S_I);
    let off_loop = a.here();
    a.mload(S_I);
    a.mload(S_N);
    a.op(Opcode::Lt);
    a.op(Opcode::IsZero);
    let fix_done = a.jumpi_fwd();
    // item = MLOAD(96 + 32i)
    a.mload(S_I);
    a.push_u64(32);
    a.op(Opcode::Mul);
    a.push_u64(96);
    a.op(Opcode::Add);
    a.op(Opcode::MLoad);
    a.mstore(S_ITEM);
    // amount = MLOAD(96 + 32(n + i))
    a.mload(S_N);
    a.mload(S_I);
    a.op(Opcode::Add);
    a.push_u64(32);
    a.op(Opcode::Mul);
    a.push_u64(96);
    a.op(Opcode::Add);
    a.op(Opcode::MLoad);
    a.mstore(S_AMT);
    a.validate_item_amount();
    if is_mint {
        a.credit_to();
        a.supply_add();
        a.log_transfer(false);
    } else {
        a.debit_caller();
        a.credit_to();
        a.log_transfer(true);
    }
    a.mload(S_I);
    a.push_u64(1);
    a.op(Opcode::Add);
    a.mstore(S_I);
    a.jump_to(off_loop);
    let off_done = a.here();
    a.patch(fix_done, off_done);
    a.op(Opcode::Stop);
}

/// Runtime bytecode of the reference multi-token (what `boing_getContractCode` returns after deploy).
///
/// Every failed check `JUMP`s out of bounds (`InvalidJump`), so the executor reverts the whole
/// transaction (batches are all-or-nothing). Unknown selectors fault too.
#[must_use]
pub fn reference_multi_token_runtime_bytecode() -> Vec<u8> {
    let mut a = Asm {
        c: Vec::new(),
        faults: Vec::new(),
    };
    // Selector = low byte of word 0.
    a.mload(0);
    let mut mask = [0u8; 32];
    mask[31] = 0xff;
    a.push(&mask);
    a.op(Opcode::And);
    a.mstore(S_SEL);

    let selectors = [
        SELECTOR_MT_BALANCE_OF,
        SELECTOR_MT_TRANSFER,
        SELECTOR_MT_MINT,
        SELECTOR_MT_SET_ITEM_METADATA,
        SELECTOR_MT_MINT_BATCH,
        SELECTOR_MT_TRANSFER_BATCH,
        SELECTOR_MT_TOTAL_SUPPLY,
        SELECTOR_MT_ITEM_METADATA,
        SELECTOR_MT_ADMIN,
        SELECTOR_MT_TRANSFER_ADMIN,
    ];
    let mut fixes = Vec::with_capacity(selectors.len());
    for sel in selectors {
        a.mload(S_SEL);
        a.push(&selector_word(sel));
        a.op(Opcode::Eq);
        fixes.push((sel, a.jumpi_fwd()));
    }
    // Unknown selector.
    a.push_u64(0);
    a.op(Opcode::IsZero);
    a.fault_if();

    for (sel, fix) in fixes {
        let off = a.here();
        a.patch(fix, off);
        match sel {
            SELECTOR_MT_BALANCE_OF => {
                a.copy(64, S_ITEM);
                a.require_fits(S_ITEM, 64);
                a.mload(32);
                a.balance_key_tail();
                a.op(Opcode::SLoad);
                a.mstore(S_RET);
                a.return_word(S_RET);
            }
            SELECTOR_MT_TRANSFER | SELECTOR_MT_MINT => {
                let is_mint = sel == SELECTOR_MT_MINT;
                if is_mint {
                    a.require_admin();
                }
                a.copy(32, S_TO);
                a.copy(64, S_ITEM);
                a.copy(96, S_AMT);
                a.require_nonzero(S_TO);
                a.validate_item_amount();
                if is_mint {
                    a.credit_to();
                    a.supply_add();
                } else {
                    a.debit_caller();
                    a.credit_to();
                }
                a.log_transfer(!is_mint);
                a.op(Opcode::Stop);
            }
            SELECTOR_MT_SET_ITEM_METADATA => {
                a.require_admin();
                a.copy(32, S_ITEM);
                a.require_fits(S_ITEM, 64);
                a.copy(64, S_LOG);
                a.mload(S_LOG);
                a.mload(S_ITEM);
                a.push(&REF_MT_METADATA_XOR);
                a.op(Opcode::Xor);
                a.op(Opcode::SStore);
                a.push(&REF_MT_TOPIC_METADATA);
                a.mload(S_ITEM);
                a.push_u64(32);
                a.push_u64(S_LOG);
                a.op(Opcode::Log2);
                a.op(Opcode::Stop);
            }
            SELECTOR_MT_MINT_BATCH => emit_batch(&mut a, true),
            SELECTOR_MT_TRANSFER_BATCH => emit_batch(&mut a, false),
            SELECTOR_MT_TOTAL_SUPPLY | SELECTOR_MT_ITEM_METADATA => {
                a.copy(32, S_ITEM);
                a.require_fits(S_ITEM, 64);
                a.mload(S_ITEM);
                a.push(if sel == SELECTOR_MT_TOTAL_SUPPLY {
                    &REF_MT_SUPPLY_XOR
                } else {
                    &REF_MT_METADATA_XOR
                });
                a.op(Opcode::Xor);
                a.op(Opcode::SLoad);
                a.mstore(S_RET);
                a.return_word(S_RET);
            }
            SELECTOR_MT_ADMIN => {
                a.push(&ref_multi_token_admin_key());
                a.op(Opcode::SLoad);
                a.mstore(S_RET);
                a.return_word(S_RET);
            }
            SELECTOR_MT_TRANSFER_ADMIN => {
                a.require_admin();
                a.copy(32, S_TO);
                a.require_nonzero(S_TO);
                a.mload(S_TO);
                a.push(&ref_multi_token_admin_key());
                a.op(Opcode::SStore);
                a.push(&REF_MT_TOPIC_ADMIN);
                a.op(Opcode::Caller);
                a.mload(S_TO);
                a.push_u64(0);
                a.push_u64(S_LOG);
                a.op(Opcode::Log3);
                a.op(Opcode::Stop);
            }
            _ => unreachable!(),
        }
    }

    let off_fault = a.here();
    let faults = std::mem::take(&mut a.faults);
    for at in faults {
        a.patch(at, off_fault);
    }
    a.push_u64(u64::MAX);
    a.op(Opcode::Jump);
    a.c
}

/// Init body (without the `0xFD` marker): `admin = CALLER`, then `RETURN` the runtime.
#[must_use]
pub fn reference_multi_token_init_bytecode(runtime: &[u8]) -> Vec<u8> {
    let mut a = Asm {
        c: Vec::new(),
        faults: Vec::new(),
    };
    a.op(Opcode::Caller);
    a.push(&ref_multi_token_admin_key());
    a.op(Opcode::SStore);
    let mut rt = runtime.to_vec();
    while rt.len() % 32 != 0 {
        rt.push(0);
    }
    for (i, ch) in rt.chunks(32).enumerate() {
        let mut w = [0u8; 32];
        w.copy_from_slice(ch);
        a.push(&w);
        a.mstore((i * 32) as u64);
    }
    a.push_u64(runtime.len() as u64);
    a.push_u64(0);
    a.op(Opcode::Return);
    a.op(Opcode::Stop);
    a.c
}

/// Pinned deploy payload: **`0xFD` || init** (init stores the deployer as admin and returns the runtime).
#[must_use]
pub fn reference_multi_token_deploy_bytecode() -> Vec<u8> {
    let rt = reference_multi_token_runtime_bytecode();
    let mut out = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
    out.extend(reference_multi_token_init_bytecode(&rt));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::Account;
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;
    use crate::vm::{VmError, GAS_PER_CONTRACT_CALL};

    const ADMIN: AccountId = AccountId([0xad; 32]);
    const ALICE: AccountId = AccountId([0xa1; 32]);
    const BOB: AccountId = AccountId([0xb2; 32]);
    const MT: AccountId = AccountId([0x3c; 32]);

    fn deploy() -> StateStore {
        let mut st = StateStore::new();
        st.insert(Account {
            id: MT,
            state: Default::default(),
        });
        let deploy = reference_multi_token_deploy_bytecode();
        assert_eq!(deploy[0], CONTRACT_DEPLOY_INIT_CODE_MARKER);
        let mut it = Interpreter::new(deploy[1..].to_vec(), GAS_PER_CONTRACT_CALL);
        it.run(ADMIN, MT, &[], &mut st).unwrap();
        let rt = it.return_data.expect("init returns runtime");
        assert_eq!(rt, reference_multi_token_runtime_bytecode());
        st.set_contract_code(MT, rt);
        st
    }

    /// Runs like a transaction: a fault reverts all state. Returns the interpreter on success.
    fn call(st: &mut StateStore, who: AccountId, cd: &[u8]) -> Result<Interpreter, VmError> {
        let cp = st.checkpoint();
        let code = st.get_contract_code(&MT).unwrap().clone();
        let mut it = Interpreter::new(code, GAS_PER_CONTRACT_CALL);
        match it.run(who, MT, cd, st) {
            Ok(_) => Ok(it),
            Err(e) => {
                st.revert(cp);
                Err(e)
            }
        }
    }

    fn fail(st: &mut StateStore, who: AccountId, cd: &[u8]) -> VmError {
        match call(st, who, cd) {
            Ok(_) => panic!("expected the call to fault"),
            Err(e) => e,
        }
    }

    fn u128_of(w: &[u8]) -> u128 {
        assert_eq!(w.len(), 32);
        assert!(w[..16].iter().all(|b| *b == 0));
        u128::from_be_bytes(w[16..32].try_into().unwrap())
    }

    fn bal(st: &mut StateStore, who: &AccountId, item: u64) -> u128 {
        let it = call(st, BOB, &encode_mt_balance_of_calldata(who, item)).unwrap();
        let via_call = u128_of(it.return_data.as_deref().unwrap());
        let via_slot =
            u128_of(&st.get_contract_storage(&MT, &multi_token_balance_storage_key(who, item)));
        assert_eq!(
            via_call, via_slot,
            "balance_of must equal the documented slot"
        );
        via_call
    }

    fn supply(st: &mut StateStore, item: u64) -> u128 {
        let it = call(st, BOB, &encode_mt_total_supply_calldata(item)).unwrap();
        u128_of(it.return_data.as_deref().unwrap())
    }

    #[test]
    fn calldata_lengths() {
        assert_eq!(encode_mt_balance_of_calldata(&ALICE, 1).len(), 96);
        assert_eq!(encode_mt_transfer_calldata(&ALICE, 1, 2).len(), 128);
        assert_eq!(encode_mt_mint_calldata(&ALICE, 1, 2).len(), 128);
        assert_eq!(encode_mt_set_item_metadata_calldata(1, &[7; 32]).len(), 96);
        assert_eq!(
            encode_mt_mint_batch_calldata(&ALICE, &[(1, 1), (2, 2)]).len(),
            96 + 128
        );
        assert_eq!(encode_mt_total_supply_calldata(1).len(), 64);
        assert_eq!(encode_mt_admin_calldata().len(), 32);
    }

    #[test]
    fn storage_keys_are_distinct_namespaces() {
        let b = multi_token_balance_storage_key(&ALICE, 5);
        assert_ne!(b, multi_token_balance_storage_key(&BOB, 5));
        assert_ne!(b, multi_token_balance_storage_key(&ALICE, 6));
        assert_ne!(
            multi_token_supply_storage_key(5),
            multi_token_metadata_storage_key(5)
        );
        assert_ne!(b, ref_multi_token_admin_key());
    }

    /// Golden vectors shared with `boing-sdk/tests/referenceMultiToken.test.ts`, Express and Observer.
    #[test]
    fn golden_storage_keys() {
        let h = |w: [u8; 32]| w.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(
            h(multi_token_balance_storage_key(&ALICE, 7)),
            "e3eee8efe6fef3e4e7ecf5fee3e0edfef790a1a1a1a1a1a10000000000000007"
        );
        assert_eq!(
            h(multi_token_supply_storage_key(7)),
            "424f494e475f5245464d545f535550504c595f56310000000000000000000007"
        );
        assert_eq!(
            h(multi_token_metadata_storage_key(7)),
            "424f494e475f5245464d545f4d4554415f563100000000000000000000000007"
        );
        assert_eq!(
            h(REF_MT_TOPIC_TRANSFER),
            "424f494e475f5245464d545f5452414e534645525f5631000000000000000000"
        );
    }

    #[test]
    fn init_sets_deployer_admin() {
        let mut st = deploy();
        let it = call(&mut st, ALICE, &encode_mt_admin_calldata()).unwrap();
        assert_eq!(it.return_data.as_deref(), Some(&ADMIN.0[..]));
    }

    #[test]
    fn mint_then_transfer_moves_shared_item_balances() {
        let mut st = deploy();
        let it = call(&mut st, ADMIN, &encode_mt_mint_calldata(&ALICE, 7, 100)).unwrap();
        assert_eq!(it.logs.len(), 1);
        assert_eq!(
            it.logs[0].topics,
            vec![REF_MT_TOPIC_TRANSFER, [0u8; 32], ALICE.0]
        );
        let mut data = multi_token_item_id_word(7).to_vec();
        data.extend_from_slice(&multi_token_amount_word(100));
        assert_eq!(it.logs[0].data, data);
        call(&mut st, ADMIN, &encode_mt_mint_calldata(&BOB, 7, 5)).unwrap();
        assert_eq!(bal(&mut st, &ALICE, 7), 100);
        assert_eq!(bal(&mut st, &BOB, 7), 5);
        assert_eq!(supply(&mut st, 7), 105);

        let it = call(&mut st, ALICE, &encode_mt_transfer_calldata(&BOB, 7, 40)).unwrap();
        assert_eq!(
            it.logs[0].topics,
            vec![REF_MT_TOPIC_TRANSFER, ALICE.0, BOB.0]
        );
        assert_eq!(bal(&mut st, &ALICE, 7), 60);
        assert_eq!(bal(&mut st, &BOB, 7), 45);
        assert_eq!(supply(&mut st, 7), 105, "transfers never change supply");
        assert_eq!(bal(&mut st, &ALICE, 8), 0, "other item ids are independent");

        // Self-transfer is a no-op on the balance.
        call(&mut st, ALICE, &encode_mt_transfer_calldata(&ALICE, 7, 60)).unwrap();
        assert_eq!(bal(&mut st, &ALICE, 7), 60);
    }

    #[test]
    fn insufficient_balance_faults_and_leaves_state() {
        let mut st = deploy();
        call(&mut st, ADMIN, &encode_mt_mint_calldata(&ALICE, 1, 10)).unwrap();
        let err = fail(&mut st, ALICE, &encode_mt_transfer_calldata(&BOB, 1, 11));
        assert!(matches!(err, VmError::InvalidJump));
        assert_eq!(bal(&mut st, &ALICE, 1), 10);
        assert_eq!(bal(&mut st, &BOB, 1), 0);
        // Holder of nothing.
        assert!(call(&mut st, BOB, &encode_mt_transfer_calldata(&ALICE, 1, 1)).is_err());
    }

    #[test]
    fn unauthorized_mint_metadata_and_admin_transfer_fault() {
        let mut st = deploy();
        assert!(matches!(
            fail(&mut st, ALICE, &encode_mt_mint_calldata(&ALICE, 1, 10)),
            VmError::InvalidJump
        ));
        assert!(call(
            &mut st,
            ALICE,
            &encode_mt_mint_batch_calldata(&ALICE, &[(1, 1)])
        )
        .is_err());
        assert!(call(
            &mut st,
            ALICE,
            &encode_mt_set_item_metadata_calldata(1, &[9; 32])
        )
        .is_err());
        assert!(call(&mut st, ALICE, &encode_mt_transfer_admin_calldata(&ALICE)).is_err());
        assert_eq!(bal(&mut st, &ALICE, 1), 0);
        assert_eq!(supply(&mut st, 1), 0);
    }

    #[test]
    fn invalid_arguments_fault() {
        let mut st = deploy();
        // amount 0, to 0
        assert!(call(&mut st, ADMIN, &encode_mt_mint_calldata(&ALICE, 1, 0)).is_err());
        assert!(call(
            &mut st,
            ADMIN,
            &encode_mt_mint_calldata(&AccountId([0; 32]), 1, 1)
        )
        .is_err());
        // item id with high bytes set
        let mut cd = encode_mt_mint_calldata(&ALICE, 1, 1);
        cd[64] = 1;
        assert!(call(&mut st, ADMIN, &cd).is_err());
        // amount above u128
        let mut cd = encode_mt_mint_calldata(&ALICE, 1, 1);
        cd[96] = 1;
        assert!(call(&mut st, ADMIN, &cd).is_err());
        // u128 overflow on balance
        call(
            &mut st,
            ADMIN,
            &encode_mt_mint_calldata(&ALICE, 2, u128::MAX),
        )
        .unwrap();
        assert!(
            call(&mut st, ADMIN, &encode_mt_mint_calldata(&BOB, 2, 1)).is_err(),
            "supply overflow"
        );
        assert_eq!(bal(&mut st, &BOB, 2), 0);
        // unknown selector
        let mut cd = vec![0u8; 32];
        cd[31] = 0x2f;
        assert!(call(&mut st, ADMIN, &cd).is_err());
        assert!(call(&mut st, ADMIN, &[]).is_err());
    }

    #[test]
    fn item_metadata_is_type_level() {
        let mut st = deploy();
        let h = [0x42u8; 32];
        let it = call(&mut st, ADMIN, &encode_mt_set_item_metadata_calldata(3, &h)).unwrap();
        assert_eq!(
            it.logs[0].topics,
            vec![REF_MT_TOPIC_METADATA, multi_token_item_id_word(3)]
        );
        assert_eq!(it.logs[0].data, h.to_vec());
        let it = call(&mut st, ALICE, &encode_mt_item_metadata_calldata(3)).unwrap();
        assert_eq!(it.return_data.as_deref(), Some(&h[..]));
        assert_eq!(
            st.get_contract_storage(&MT, &multi_token_metadata_storage_key(3)),
            h
        );
    }

    #[test]
    fn mint_batch_and_transfer_batch() {
        let mut st = deploy();
        let n = MAX_REFERENCE_MULTI_TOKEN_BATCH as u64;
        let entries: Vec<(u64, u128)> = (1..=n).map(|i| (i, (i as u128) * 10)).collect();
        let it = call(
            &mut st,
            ADMIN,
            &encode_mt_mint_batch_calldata(&ALICE, &entries),
        )
        .unwrap();
        assert_eq!(it.logs.len(), n as usize);
        for (id, amt) in &entries {
            assert_eq!(bal(&mut st, &ALICE, *id), *amt);
            assert_eq!(supply(&mut st, *id), *amt);
        }
        // Duplicate ids in one batch accumulate.
        call(
            &mut st,
            ALICE,
            &encode_mt_transfer_batch_calldata(&BOB, &[(1, 4), (1, 6), (2, 20)]),
        )
        .unwrap();
        assert_eq!(bal(&mut st, &ALICE, 1), 0);
        assert_eq!(bal(&mut st, &BOB, 1), 10);
        assert_eq!(bal(&mut st, &BOB, 2), 20);
        // All-or-nothing: second entry overdraws → first entry rolled back.
        let err = fail(
            &mut st,
            ALICE,
            &encode_mt_transfer_batch_calldata(&BOB, &[(3, 30), (4, 41)]),
        );
        assert!(matches!(err, VmError::InvalidJump));
        assert_eq!(bal(&mut st, &ALICE, 3), 30);
        assert_eq!(bal(&mut st, &BOB, 3), 0);
        // n = MAX + 1 faults; n = 0 faults; trailing garbage faults.
        let mut cd = encode_mt_mint_batch_calldata(&ALICE, &[(1, 1)]);
        cd[95] = (n + 1) as u8;
        cd.resize(96 + 64 * (n as usize + 1), 0);
        assert!(call(&mut st, ADMIN, &cd).is_err());
        let mut cd = encode_mt_mint_batch_calldata(&ALICE, &[(1, 1)]);
        cd[95] = 0;
        assert!(call(&mut st, ADMIN, &cd).is_err());
        let mut cd = encode_mt_mint_batch_calldata(&ALICE, &[(1, 1)]);
        cd.extend_from_slice(&[0xff; 32]);
        assert!(call(&mut st, ADMIN, &cd).is_err());
    }

    #[test]
    fn transfer_admin_hands_over_mint_rights() {
        let mut st = deploy();
        let it = call(&mut st, ADMIN, &encode_mt_transfer_admin_calldata(&ALICE)).unwrap();
        assert_eq!(
            it.logs[0].topics,
            vec![REF_MT_TOPIC_ADMIN, ADMIN.0, ALICE.0]
        );
        assert!(call(&mut st, ADMIN, &encode_mt_mint_calldata(&BOB, 1, 1)).is_err());
        call(&mut st, ALICE, &encode_mt_mint_calldata(&BOB, 1, 1)).unwrap();
        assert!(call(
            &mut st,
            ALICE,
            &encode_mt_transfer_admin_calldata(&AccountId([0; 32]))
        )
        .is_err());
    }

    #[test]
    fn gas_per_op_is_bounded() {
        let mut st = deploy();
        let g = |st: &mut StateStore, who, cd: &[u8]| call(st, who, cd).unwrap().gas_used;
        let mint_new = g(&mut st, ADMIN, &encode_mt_mint_calldata(&ALICE, 1, 10));
        let transfer = g(&mut st, ALICE, &encode_mt_transfer_calldata(&BOB, 1, 5));
        let entries: Vec<(u64, u128)> = (1..=16).map(|i| (i, 1)).collect();
        let batch16 = g(
            &mut st,
            ADMIN,
            &encode_mt_mint_batch_calldata(&ALICE, &entries),
        );
        let xfer16 = g(
            &mut st,
            ALICE,
            &encode_mt_transfer_batch_calldata(&BOB, &entries),
        );
        let read = g(&mut st, BOB, &encode_mt_balance_of_calldata(&ALICE, 1));
        println!("gas mint={mint_new} transfer={transfer} mint_batch16={batch16} transfer_batch16={xfer16} balance_of={read}");
        assert!(mint_new < 50_000, "mint {mint_new}");
        assert!(transfer < 50_000, "transfer {transfer}");
        assert!(batch16 < 800_000, "mint_batch16 {batch16}");
        assert!(xfer16 < 800_000, "transfer_batch16 {xfer16}");
        assert!(read < 2_000, "balance_of {read}");
        let size = reference_multi_token_deploy_bytecode().len();
        assert!(
            size <= boing_qa::DEFAULT_MAX_BYTECODE_SIZE,
            "deploy bytecode {size} bytes"
        );
    }

    #[test]
    fn executor_deploys_with_purpose_nft_and_runs_calls() {
        use boing_primitives::{
            nonce_derived_contract_address, AccessList, AccountState, Transaction,
            TransactionPayload,
        };
        let vm = crate::vm::Vm::new();
        let mut st = StateStore::new();
        st.insert(Account {
            id: ADMIN,
            state: AccountState {
                balance: 1_000_000_000,
                ..Default::default()
            },
        });
        let deploy = Transaction {
            nonce: 0,
            sender: ADMIN,
            payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: reference_multi_token_deploy_bytecode(),
                purpose_category: "nft".to_string(),
                description_hash: None,
                asset_name: Some("Boing Items".to_string()),
                asset_symbol: Some("ITEMS".to_string()),
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        vm.execute(&deploy, &mut st).unwrap();
        let contract = nonce_derived_contract_address(&ADMIN, 0);
        assert_eq!(
            st.get_contract_code(&contract).unwrap().as_slice(),
            reference_multi_token_runtime_bytecode().as_slice()
        );
        assert_eq!(
            st.get_contract_storage(&contract, &ref_multi_token_admin_key()),
            ADMIN.0
        );
        let call_tx = |nonce: u64, sender: AccountId, calldata: Vec<u8>| Transaction {
            nonce,
            sender,
            payload: TransactionPayload::ContractCall { contract, calldata },
            access_list: AccessList::default(),
        };
        let out = vm
            .execute(
                &call_tx(
                    1,
                    ADMIN,
                    encode_mt_mint_batch_calldata(&ALICE, &[(1, 3), (2, 4)]),
                ),
                &mut st,
            )
            .unwrap();
        assert_eq!(out.logs.len(), 2);
        assert_eq!(
            st.get_contract_storage(&contract, &multi_token_balance_storage_key(&ALICE, 2)),
            multi_token_amount_word(4)
        );
        // A faulting batch (overdraw on entry 2) reverts entry 1 at the executor level.
        st.insert(Account {
            id: ALICE,
            state: AccountState {
                balance: 1_000_000_000,
                ..Default::default()
            },
        });
        // (The block executor wraps every tx in a state checkpoint; mirror that here.)
        let cp = st.checkpoint();
        let res = vm.execute(
            &call_tx(
                0,
                ALICE,
                encode_mt_transfer_batch_calldata(&BOB, &[(1, 3), (2, 5)]),
            ),
            &mut st,
        );
        assert!(res.is_err());
        st.revert(cp);
        assert_eq!(
            st.get_contract_storage(&contract, &multi_token_balance_storage_key(&ALICE, 1)),
            multi_token_amount_word(3)
        );
        assert_eq!(
            st.get_contract_storage(&contract, &multi_token_balance_storage_key(&BOB, 1)),
            [0u8; 32]
        );
    }

    #[test]
    fn deploy_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
        let registry = RuleRegistry::new();
        for code in [
            reference_multi_token_deploy_bytecode(),
            reference_multi_token_runtime_bytecode(),
        ] {
            let r = check_contract_deploy_full(&code, Some("nft"), None, &registry);
            assert!(matches!(r, QaResult::Allow | QaResult::Unsure), "got {r:?}");
        }
    }
}
