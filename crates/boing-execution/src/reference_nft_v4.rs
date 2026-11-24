//! Reference **NFT collection template v4** (Boing VM): faults on every failure, emits logs, and adds
//! approvals, admin transfer, metadata freezing and collection reads.
//!
//! Not consensus-enforced — see `docs/BOING-REFERENCE-NFT.md` (section "Template v4").
//!
//! Why a v4: templates v1–v3 end unauthorized `transfer_nft` / `set_metadata_hash` with `STOP`, so the
//! receipt says `success: true` although nothing changed, and they emit no logs. Indexers had to replay
//! the auth rules from calldata. v4 makes `success` mean "it happened":
//!
//! - **Every failed check and every unknown selector faults** (`JUMP` out of bounds → `InvalidJump`), so
//!   the executor reverts the whole transaction. There is no silent `STOP` path.
//! - **Admin is set by the `0xFD` init frame** ([`reference_nft_v4_deploy_bytecode`]). There is no lazy
//!   admin: a runtime deployed without init has no admin and nothing can be minted.
//! - **Logs** for every state change (see the `REF_NFT_V4_TOPIC_*` constants).
//! - **Approvals**: per-token `approve` and per-owner `set_approval_for_all`, so marketplaces can sell
//!   without taking custody.
//! - **`transfer_admin`**, **`freeze_metadata`** (one token) and **`freeze_all_metadata`** (collection).
//! - Metadata is **admin-only** after mint (v1–v3 let the current holder rewrite it).
//! - Minting is **only** through `mint_batch` (v1–v3 also lazy-minted through `transfer_nft`).
//!
//! Storage keys are XOR-tagged words (no hashing opcode). Owner and metadata slots use the **same** keys
//! as v1–v3 ([`REF_NFT_OWNER_STORAGE_XOR`], [`REF_NFT_METADATA_STORAGE_XOR`]) and the admin sits in the
//! same singleton slot ([`ref_nft_collection_admin_key`]), so existing storage probes keep working.
//! Token ids are opaque 32-byte words chosen by the admin; XOR namespaces mean an admin who picks
//! adversarial ids could alias slots, which only the admin (who already controls minting) can do.

use boing_primitives::{AccountId, CONTRACT_DEPLOY_INIT_CODE_MARKER};

use crate::bytecode::Opcode;
use crate::reference_nft::{
    ref_nft_collection_admin_key, REF_NFT_METADATA_STORAGE_XOR, REF_NFT_OWNER_STORAGE_XOR,
    SELECTOR_MINT_BATCH, SELECTOR_OWNER_OF, SELECTOR_SET_METADATA_HASH, SELECTOR_TRANSFER_NFT,
};
use crate::reference_token::selector_word;

/// Pinned template version (artifact id `boing.reference_nft_collection.v4`).
pub const REFERENCE_NFT_V4_TEMPLATE_VERSION: u32 = 4;

/// `approve(spender, token_id)` — owner or operator; `spender = 0` clears. 96-byte calldata.
pub const SELECTOR_NFT_APPROVE: u8 = 0x07;
/// `set_approval_for_all(operator, approved)` — `approved` is 0 or 1. 96-byte calldata.
pub const SELECTOR_NFT_SET_APPROVAL_FOR_ALL: u8 = 0x08;
/// `get_approved(token_id)` → AccountId word. 64-byte calldata.
pub const SELECTOR_NFT_GET_APPROVED: u8 = 0x09;
/// `is_approved_for_all(owner, operator)` → 0/1 word. 96-byte calldata.
pub const SELECTOR_NFT_IS_APPROVED_FOR_ALL: u8 = 0x0a;
/// `admin()` → AccountId word. 32-byte calldata.
pub const SELECTOR_NFT_ADMIN: u8 = 0x0b;
/// `transfer_admin(new_admin)` — admin only; `new_admin != 0`. 64-byte calldata.
pub const SELECTOR_NFT_TRANSFER_ADMIN: u8 = 0x0c;
/// `freeze_metadata(token_id)` — admin only; token must exist. 64-byte calldata.
pub const SELECTOR_NFT_FREEZE_METADATA: u8 = 0x0d;
/// `freeze_all_metadata()` — admin only; blocks every later `set_metadata_hash`. 32-byte calldata.
pub const SELECTOR_NFT_FREEZE_ALL_METADATA: u8 = 0x0e;
/// `metadata_hash(token_id)` → word. 64-byte calldata.
pub const SELECTOR_NFT_METADATA_HASH: u8 = 0x0f;
/// `total_minted()` → u64 word (count of tokens ever minted). 32-byte calldata.
pub const SELECTOR_NFT_TOTAL_MINTED: u8 = 0x10;
/// `is_metadata_frozen(token_id)` → 0/1 word (token frozen or whole collection frozen). 64-byte calldata.
pub const SELECTOR_NFT_IS_METADATA_FROZEN: u8 = 0x11;

/// Max tokens per v4 `mint_batch` (same as v3). Mint logs carry 32 ids each, so 500 → 16 logs (VM cap 24).
pub const MAX_REFERENCE_NFT_V4_MINT_BATCH: u16 = 500;
/// Token ids per `MINT_BATCH` log (`MAX_EXECUTION_LOG_DATA_BYTES / 32`).
pub const REF_NFT_V4_IDS_PER_MINT_LOG: u64 = 32;

const fn tag32(s: &[u8]) -> [u8; 32] {
    let mut o = [0u8; 32];
    let mut i = 0;
    while i < s.len() {
        o[i] = s[i];
        i += 1;
    }
    o
}

/// Per-token approval slot: `token_id ^ REF_NFT_V4_APPROVED_XOR`.
pub const REF_NFT_V4_APPROVED_XOR: [u8; 32] = tag32(b"BOING_REFNFT_APPROVED_V4");
/// Per-token metadata freeze slot: `token_id ^ REF_NFT_V4_FROZEN_XOR` (1 = frozen).
pub const REF_NFT_V4_FROZEN_XOR: [u8; 32] = tag32(b"BOING_REFNFT_FROZEN_V4");
/// Operator slot: `(owner[0..16] || operator[0..16]) ^ REF_NFT_V4_OPERATOR_XOR` (1 = approved for all).
pub const REF_NFT_V4_OPERATOR_XOR: [u8; 32] = tag32(b"BOING_REFNFT_OPERATOR_V4");

/// `LOG3 [TOPIC, from, to]`, data `token_id`. Emitted by `transfer_nft`.
pub const REF_NFT_V4_TOPIC_TRANSFER: [u8; 32] = tag32(b"BOING_REFNFT_TRANSFER_V4");
/// `LOG2 [TOPIC, to]`, data = up to 32 minted `token_id` words (one log per chunk of `mint_batch`).
pub const REF_NFT_V4_TOPIC_MINT_BATCH: [u8; 32] = tag32(b"BOING_REFNFT_MINT_BATCH_V4");
/// `LOG3 [TOPIC, owner, spender]`, data `token_id`.
pub const REF_NFT_V4_TOPIC_APPROVAL: [u8; 32] = tag32(b"BOING_REFNFT_APPROVAL_V4");
/// `LOG3 [TOPIC, owner, operator]`, data `approved` word (0/1).
pub const REF_NFT_V4_TOPIC_APPROVAL_FOR_ALL: [u8; 32] = tag32(b"BOING_REFNFT_APPROVAL_ALL_V4");
/// `LOG2 [TOPIC, token_id]`, data `metadata_hash`.
pub const REF_NFT_V4_TOPIC_METADATA: [u8; 32] = tag32(b"BOING_REFNFT_METADATA_V4");
/// `LOG2 [TOPIC, token_id]` (`token_id = 0` for `freeze_all_metadata`), no data.
pub const REF_NFT_V4_TOPIC_FREEZE: [u8; 32] = tag32(b"BOING_REFNFT_FREEZE_V4");
/// `LOG3 [TOPIC, old_admin, new_admin]`, no data.
pub const REF_NFT_V4_TOPIC_ADMIN: [u8; 32] = tag32(b"BOING_REFNFT_ADMIN_V4");

fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

fn xor32(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..32 {
        o[i] = a[i] ^ b[i];
    }
    o
}

/// Singleton: collection-wide metadata freeze flag (1 = frozen).
#[must_use]
pub fn ref_nft_v4_frozen_all_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xe1;
    k
}

/// Singleton: count of tokens ever minted (u64 word).
#[must_use]
pub fn ref_nft_v4_total_minted_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xe2;
    k
}

/// Approval slot for `token_id`.
#[must_use]
pub fn ref_nft_v4_approved_storage_key(token_id: &[u8; 32]) -> [u8; 32] {
    xor32(token_id, &REF_NFT_V4_APPROVED_XOR)
}

/// Metadata freeze slot for `token_id`.
#[must_use]
pub fn ref_nft_v4_frozen_storage_key(token_id: &[u8; 32]) -> [u8; 32] {
    xor32(token_id, &REF_NFT_V4_FROZEN_XOR)
}

/// Operator slot for `(owner, operator)`.
#[must_use]
pub fn ref_nft_v4_operator_storage_key(owner: &AccountId, operator: &AccountId) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[..16].copy_from_slice(&owner.0[..16]);
    k[16..].copy_from_slice(&operator.0[..16]);
    xor32(&k, &REF_NFT_V4_OPERATOR_XOR)
}

fn call1(sel: u8) -> Vec<u8> {
    selector_word(sel).to_vec()
}

fn call2(sel: u8, a: &[u8; 32]) -> Vec<u8> {
    let mut v = call1(sel);
    v.extend_from_slice(a);
    v
}

fn call3(sel: u8, a: &[u8; 32], b: &[u8; 32]) -> Vec<u8> {
    let mut v = call2(sel, a);
    v.extend_from_slice(b);
    v
}

/// `approve(spender, token_id)` calldata (96 bytes).
pub fn encode_nft_approve_calldata(spender: &AccountId, token_id: &[u8; 32]) -> Vec<u8> {
    call3(SELECTOR_NFT_APPROVE, &spender.0, token_id)
}

/// `set_approval_for_all(operator, approved)` calldata (96 bytes).
pub fn encode_nft_set_approval_for_all_calldata(operator: &AccountId, approved: bool) -> Vec<u8> {
    call3(
        SELECTOR_NFT_SET_APPROVAL_FOR_ALL,
        &operator.0,
        &word_u64(u64::from(approved)),
    )
}

/// `get_approved(token_id)` calldata (64 bytes).
pub fn encode_nft_get_approved_calldata(token_id: &[u8; 32]) -> Vec<u8> {
    call2(SELECTOR_NFT_GET_APPROVED, token_id)
}

/// `is_approved_for_all(owner, operator)` calldata (96 bytes).
pub fn encode_nft_is_approved_for_all_calldata(owner: &AccountId, operator: &AccountId) -> Vec<u8> {
    call3(SELECTOR_NFT_IS_APPROVED_FOR_ALL, &owner.0, &operator.0)
}

/// `admin()` calldata (32 bytes).
pub fn encode_nft_admin_calldata() -> Vec<u8> {
    call1(SELECTOR_NFT_ADMIN)
}

/// `transfer_admin(new_admin)` calldata (64 bytes).
pub fn encode_nft_transfer_admin_calldata(new_admin: &AccountId) -> Vec<u8> {
    call2(SELECTOR_NFT_TRANSFER_ADMIN, &new_admin.0)
}

/// `freeze_metadata(token_id)` calldata (64 bytes).
pub fn encode_nft_freeze_metadata_calldata(token_id: &[u8; 32]) -> Vec<u8> {
    call2(SELECTOR_NFT_FREEZE_METADATA, token_id)
}

/// `freeze_all_metadata()` calldata (32 bytes).
pub fn encode_nft_freeze_all_metadata_calldata() -> Vec<u8> {
    call1(SELECTOR_NFT_FREEZE_ALL_METADATA)
}

/// `metadata_hash(token_id)` calldata (64 bytes).
pub fn encode_nft_metadata_hash_calldata(token_id: &[u8; 32]) -> Vec<u8> {
    call2(SELECTOR_NFT_METADATA_HASH, token_id)
}

/// `total_minted()` calldata (32 bytes).
pub fn encode_nft_total_minted_calldata() -> Vec<u8> {
    call1(SELECTOR_NFT_TOTAL_MINTED)
}

/// `is_metadata_frozen(token_id)` calldata (64 bytes).
pub fn encode_nft_is_metadata_frozen_calldata(token_id: &[u8; 32]) -> Vec<u8> {
    call2(SELECTOR_NFT_IS_METADATA_FROZEN, token_id)
}

// --- VM bytecode -----------------------------------------------------------------------------
// Calldata sits at mem[0..96+64*500 = 32096). Scratch words start at 32768.

const S_SEL: u64 = 32_768;
const S_TO: u64 = S_SEL + 32;
const S_TID: u64 = S_SEL + 64;
const S_OWNER: u64 = S_SEL + 96;
const S_WORD: u64 = S_SEL + 128;
const S_N: u64 = S_SEL + 160;
const S_I: u64 = S_SEL + 192;
const S_J: u64 = S_SEL + 224;
const S_CNT: u64 = S_SEL + 256;
const S_RET: u64 = S_SEL + 288;
/// 32-byte log data buffer.
const S_LOG: u64 = S_SEL + 320;

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
    fn require_admin(&mut self) {
        self.op(Opcode::Caller);
        self.push(&ref_nft_collection_admin_key());
        self.op(Opcode::SLoad);
        self.op(Opcode::Eq);
        self.op(Opcode::IsZero);
        self.fault_if();
    }
    /// `SLOAD(mem[S_TID] ^ xor)` on the stack.
    fn sload_tid(&mut self, xor: &[u8; 32]) {
        self.mload(S_TID);
        self.push(xor);
        self.op(Opcode::Xor);
        self.op(Opcode::SLoad);
    }
    /// Store the stack top at `mem[S_TID] ^ xor`.
    fn sstore_tid(&mut self, xor: &[u8; 32]) {
        self.mload(S_TID);
        self.push(xor);
        self.op(Opcode::Xor);
        self.op(Opcode::SStore);
    }
    /// `mem[S_OWNER] = owner(mem[S_TID])`; fault when the token does not exist.
    fn load_owner_require_exists(&mut self) {
        self.sload_tid(&REF_NFT_OWNER_STORAGE_XOR);
        self.mstore(S_OWNER);
        self.require_nonzero(S_OWNER);
    }
    /// Operator key for the two account words on the stack (`owner` pushed first, then `operator`).
    fn operator_key_tail(&mut self) {
        // operator >> 128 (low half)
        self.push_u64(128);
        self.op(Opcode::Shr);
        self.mstore(S_WORD);
        // (owner >> 128) << 128 (high half)
        self.push_u64(128);
        self.op(Opcode::Shr);
        self.push_u64(128);
        self.op(Opcode::Shl);
        self.mload(S_WORD);
        self.op(Opcode::Or);
        self.push(&REF_NFT_V4_OPERATOR_XOR);
        self.op(Opcode::Xor);
    }
    /// Pushes 1 when `CALLER` is `mem[S_OWNER]` or an operator of it, else 0.
    fn caller_is_owner_or_operator(&mut self) {
        self.op(Opcode::Caller);
        self.mload(S_OWNER);
        self.op(Opcode::Eq);
        self.mload(S_OWNER);
        self.op(Opcode::Caller);
        self.operator_key_tail();
        self.op(Opcode::SLoad);
        self.op(Opcode::Or);
    }
    fn return_word(&mut self, off: u64) {
        self.push_u64(32);
        self.push_u64(off);
        self.op(Opcode::Return);
    }
    /// `RETURN` the stack top as a word.
    fn return_top(&mut self) {
        self.mstore(S_RET);
        self.return_word(S_RET);
    }
    /// Fault when the 32-byte word at calldata offset `off` is non-zero (catches over-long calldata
    /// for fixed-size calls; zero padding cannot be told apart without `CALLDATASIZE`).
    fn require_zero_word_at(&mut self, off: u64) {
        self.mload(off);
        self.op(Opcode::IsZero);
        self.op(Opcode::IsZero);
        self.fault_if();
    }
}

/// `mem[S_TID] = MLOAD(96 + 32 * mem[idx_off])`.
fn load_batch_id(a: &mut Asm, idx_off: u64) {
    a.mload(idx_off);
    a.push_u64(32);
    a.op(Opcode::Mul);
    a.push_u64(96);
    a.op(Opcode::Add);
    a.op(Opcode::MLoad);
    a.mstore(S_TID);
}

fn emit_mint_batch(a: &mut Asm) {
    a.require_admin();
    a.copy(32, S_TO);
    a.copy(64, S_N);
    a.require_nonzero(S_TO);
    // 1 <= n <= MAX (Gt on the full word also rejects high-byte garbage).
    a.require_nonzero(S_N);
    a.mload(S_N);
    a.push_u64(u64::from(MAX_REFERENCE_NFT_V4_MINT_BATCH));
    a.op(Opcode::Gt);
    a.fault_if();
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

    // Check pass: every id unowned and unique (O(n²), as in v3).
    a.push_u64(0);
    a.mstore(S_I);
    let off_check = a.here();
    a.mload(S_I);
    a.mload(S_N);
    a.op(Opcode::Lt);
    a.op(Opcode::IsZero);
    let fix_checked = a.jumpi_fwd();
    load_batch_id(a, S_I);
    a.sload_tid(&REF_NFT_OWNER_STORAGE_XOR);
    a.op(Opcode::IsZero);
    a.op(Opcode::IsZero);
    a.fault_if();
    a.mload(S_TID);
    a.mstore(S_WORD);
    a.push_u64(0);
    a.mstore(S_J);
    let off_uniq = a.here();
    a.mload(S_J);
    a.mload(S_I);
    a.op(Opcode::Lt);
    a.op(Opcode::IsZero);
    let fix_uniq_done = a.jumpi_fwd();
    load_batch_id(a, S_J);
    a.mload(S_TID);
    a.mload(S_WORD);
    a.op(Opcode::Eq);
    a.fault_if();
    a.mload(S_J);
    a.push_u64(1);
    a.op(Opcode::Add);
    a.mstore(S_J);
    a.jump_to(off_uniq);
    let off_uniq_done = a.here();
    a.patch(fix_uniq_done, off_uniq_done);
    a.mload(S_I);
    a.push_u64(1);
    a.op(Opcode::Add);
    a.mstore(S_I);
    a.jump_to(off_check);
    let off_checked = a.here();
    a.patch(fix_checked, off_checked);

    // Write pass: owner, then metadata when the hash is non-zero.
    a.push_u64(0);
    a.mstore(S_I);
    let off_write = a.here();
    a.mload(S_I);
    a.mload(S_N);
    a.op(Opcode::Lt);
    a.op(Opcode::IsZero);
    let fix_written = a.jumpi_fwd();
    load_batch_id(a, S_I);
    a.mload(S_TO);
    a.sstore_tid(&REF_NFT_OWNER_STORAGE_XOR);
    // hash = MLOAD(96 + 32(n + i))
    a.mload(S_N);
    a.mload(S_I);
    a.op(Opcode::Add);
    a.push_u64(32);
    a.op(Opcode::Mul);
    a.push_u64(96);
    a.op(Opcode::Add);
    a.op(Opcode::MLoad);
    a.mstore(S_WORD);
    a.mload(S_WORD);
    a.op(Opcode::IsZero);
    let fix_skip_meta = a.jumpi_fwd();
    a.mload(S_WORD);
    a.sstore_tid(&REF_NFT_METADATA_STORAGE_XOR);
    let off_skip_meta = a.here();
    a.patch(fix_skip_meta, off_skip_meta);
    a.mload(S_I);
    a.push_u64(1);
    a.op(Opcode::Add);
    a.mstore(S_I);
    a.jump_to(off_write);
    let off_written = a.here();
    a.patch(fix_written, off_written);

    // total_minted += n
    a.push(&ref_nft_v4_total_minted_key());
    a.op(Opcode::SLoad);
    a.mload(S_N);
    a.op(Opcode::Add);
    a.push(&ref_nft_v4_total_minted_key());
    a.op(Opcode::SStore);

    // Logs: LOG2 [MINT_BATCH, to] per chunk of up to 32 ids, data read straight from calldata.
    a.push_u64(0);
    a.mstore(S_I);
    let off_log = a.here();
    a.mload(S_I);
    a.mload(S_N);
    a.op(Opcode::Lt);
    a.op(Opcode::IsZero);
    let fix_logged = a.jumpi_fwd();
    // cnt = min(32, n - i)
    a.mload(S_N);
    a.mload(S_I);
    a.op(Opcode::Sub);
    a.mstore(S_CNT);
    a.mload(S_CNT);
    a.push_u64(REF_NFT_V4_IDS_PER_MINT_LOG);
    a.op(Opcode::Gt);
    a.op(Opcode::IsZero);
    let fix_cnt_ok = a.jumpi_fwd();
    a.push_u64(REF_NFT_V4_IDS_PER_MINT_LOG);
    a.mstore(S_CNT);
    let off_cnt_ok = a.here();
    a.patch(fix_cnt_ok, off_cnt_ok);
    a.push(&REF_NFT_V4_TOPIC_MINT_BATCH);
    a.mload(S_TO);
    // size = cnt * 32
    a.mload(S_CNT);
    a.push_u64(32);
    a.op(Opcode::Mul);
    // offset = 96 + 32 i
    a.mload(S_I);
    a.push_u64(32);
    a.op(Opcode::Mul);
    a.push_u64(96);
    a.op(Opcode::Add);
    a.op(Opcode::Log2);
    a.mload(S_I);
    a.push_u64(REF_NFT_V4_IDS_PER_MINT_LOG);
    a.op(Opcode::Add);
    a.mstore(S_I);
    a.jump_to(off_log);
    let off_logged = a.here();
    a.patch(fix_logged, off_logged);
    a.op(Opcode::Stop);
}

/// Runtime bytecode of the v4 collection (what `boing_getContractCode` returns after deploy).
///
/// Every failed check `JUMP`s out of bounds (`InvalidJump`), so the executor reverts the whole
/// transaction. Unknown selectors fault too.
#[must_use]
pub fn reference_nft_v4_runtime_bytecode() -> Vec<u8> {
    let mut a = Asm {
        c: Vec::new(),
        faults: Vec::new(),
    };
    // Selector = low byte of word 0; the rest of word 0 must be zero.
    a.mload(0);
    a.mstore(S_SEL);
    a.mload(S_SEL);
    a.push_u64(8);
    a.op(Opcode::Shr);
    a.op(Opcode::IsZero);
    a.op(Opcode::IsZero);
    a.fault_if();

    let selectors = [
        SELECTOR_OWNER_OF,
        SELECTOR_TRANSFER_NFT,
        SELECTOR_SET_METADATA_HASH,
        SELECTOR_MINT_BATCH,
        SELECTOR_NFT_APPROVE,
        SELECTOR_NFT_SET_APPROVAL_FOR_ALL,
        SELECTOR_NFT_GET_APPROVED,
        SELECTOR_NFT_IS_APPROVED_FOR_ALL,
        SELECTOR_NFT_ADMIN,
        SELECTOR_NFT_TRANSFER_ADMIN,
        SELECTOR_NFT_FREEZE_METADATA,
        SELECTOR_NFT_FREEZE_ALL_METADATA,
        SELECTOR_NFT_METADATA_HASH,
        SELECTOR_NFT_TOTAL_MINTED,
        SELECTOR_NFT_IS_METADATA_FROZEN,
    ];
    let mut fixes = Vec::with_capacity(selectors.len());
    for sel in selectors {
        a.mload(S_SEL);
        a.push(&selector_word(sel));
        a.op(Opcode::Eq);
        fixes.push((sel, a.jumpi_fwd()));
    }
    // Unknown selector.
    a.push_u64(1);
    a.fault_if();

    for (sel, fix) in fixes {
        let off = a.here();
        a.patch(fix, off);
        match sel {
            SELECTOR_OWNER_OF => {
                a.copy(32, S_TID);
                a.sload_tid(&REF_NFT_OWNER_STORAGE_XOR);
                a.return_top();
            }
            SELECTOR_TRANSFER_NFT => {
                a.copy(32, S_TO);
                a.copy(64, S_TID);
                a.require_zero_word_at(96);
                a.require_nonzero(S_TO);
                a.load_owner_require_exists();
                // authorized = caller is owner | operator | approved
                a.caller_is_owner_or_operator();
                a.op(Opcode::Caller);
                a.sload_tid(&REF_NFT_V4_APPROVED_XOR);
                a.op(Opcode::Eq);
                a.op(Opcode::Or);
                a.op(Opcode::IsZero);
                a.fault_if();
                a.mload(S_TO);
                a.sstore_tid(&REF_NFT_OWNER_STORAGE_XOR);
                // Clear a per-token approval when one is set.
                a.sload_tid(&REF_NFT_V4_APPROVED_XOR);
                a.op(Opcode::IsZero);
                let fix_no_approval = a.jumpi_fwd();
                a.push_u64(0);
                a.sstore_tid(&REF_NFT_V4_APPROVED_XOR);
                let off_no_approval = a.here();
                a.patch(fix_no_approval, off_no_approval);
                a.copy(S_TID, S_LOG);
                a.push(&REF_NFT_V4_TOPIC_TRANSFER);
                a.mload(S_OWNER);
                a.mload(S_TO);
                a.push_u64(32);
                a.push_u64(S_LOG);
                a.op(Opcode::Log3);
                a.op(Opcode::Stop);
            }
            SELECTOR_SET_METADATA_HASH => {
                a.require_admin();
                a.copy(32, S_TID);
                a.copy(64, S_WORD);
                a.require_zero_word_at(96);
                a.load_owner_require_exists();
                a.push(&ref_nft_v4_frozen_all_key());
                a.op(Opcode::SLoad);
                a.sload_tid(&REF_NFT_V4_FROZEN_XOR);
                a.op(Opcode::Or);
                a.fault_if();
                a.mload(S_WORD);
                a.sstore_tid(&REF_NFT_METADATA_STORAGE_XOR);
                a.push(&REF_NFT_V4_TOPIC_METADATA);
                a.mload(S_TID);
                a.push_u64(32);
                a.push_u64(S_WORD);
                a.op(Opcode::Log2);
                a.op(Opcode::Stop);
            }
            SELECTOR_MINT_BATCH => emit_mint_batch(&mut a),
            SELECTOR_NFT_APPROVE => {
                a.copy(32, S_TO);
                a.copy(64, S_TID);
                a.require_zero_word_at(96);
                a.load_owner_require_exists();
                a.caller_is_owner_or_operator();
                a.op(Opcode::IsZero);
                a.fault_if();
                a.mload(S_TO);
                a.sstore_tid(&REF_NFT_V4_APPROVED_XOR);
                a.copy(S_TID, S_LOG);
                a.push(&REF_NFT_V4_TOPIC_APPROVAL);
                a.mload(S_OWNER);
                a.mload(S_TO);
                a.push_u64(32);
                a.push_u64(S_LOG);
                a.op(Opcode::Log3);
                a.op(Opcode::Stop);
            }
            SELECTOR_NFT_SET_APPROVAL_FOR_ALL => {
                a.copy(32, S_TO);
                a.copy(64, S_WORD);
                a.require_zero_word_at(96);
                a.require_nonzero(S_TO);
                // approved must be 0 or 1
                a.mload(S_WORD);
                a.push_u64(1);
                a.op(Opcode::Gt);
                a.fault_if();
                a.mload(S_WORD);
                a.op(Opcode::Caller);
                a.mload(S_TO);
                a.operator_key_tail();
                a.op(Opcode::SStore);
                a.copy(S_WORD, S_LOG);
                a.push(&REF_NFT_V4_TOPIC_APPROVAL_FOR_ALL);
                a.op(Opcode::Caller);
                a.mload(S_TO);
                a.push_u64(32);
                a.push_u64(S_LOG);
                a.op(Opcode::Log3);
                a.op(Opcode::Stop);
            }
            SELECTOR_NFT_GET_APPROVED => {
                a.copy(32, S_TID);
                a.sload_tid(&REF_NFT_V4_APPROVED_XOR);
                a.return_top();
            }
            SELECTOR_NFT_IS_APPROVED_FOR_ALL => {
                a.mload(32);
                a.mload(64);
                a.operator_key_tail();
                a.op(Opcode::SLoad);
                a.return_top();
            }
            SELECTOR_NFT_ADMIN => {
                a.push(&ref_nft_collection_admin_key());
                a.op(Opcode::SLoad);
                a.return_top();
            }
            SELECTOR_NFT_TRANSFER_ADMIN => {
                a.require_admin();
                a.copy(32, S_TO);
                a.require_zero_word_at(64);
                a.require_nonzero(S_TO);
                a.mload(S_TO);
                a.push(&ref_nft_collection_admin_key());
                a.op(Opcode::SStore);
                a.push(&REF_NFT_V4_TOPIC_ADMIN);
                a.op(Opcode::Caller);
                a.mload(S_TO);
                a.push_u64(0);
                a.push_u64(S_LOG);
                a.op(Opcode::Log3);
                a.op(Opcode::Stop);
            }
            SELECTOR_NFT_FREEZE_METADATA => {
                a.require_admin();
                a.copy(32, S_TID);
                a.require_zero_word_at(64);
                a.load_owner_require_exists();
                a.push_u64(1);
                a.sstore_tid(&REF_NFT_V4_FROZEN_XOR);
                a.push(&REF_NFT_V4_TOPIC_FREEZE);
                a.mload(S_TID);
                a.push_u64(0);
                a.push_u64(S_LOG);
                a.op(Opcode::Log2);
                a.op(Opcode::Stop);
            }
            SELECTOR_NFT_FREEZE_ALL_METADATA => {
                a.require_admin();
                a.require_zero_word_at(32);
                a.push_u64(1);
                a.push(&ref_nft_v4_frozen_all_key());
                a.op(Opcode::SStore);
                a.push(&REF_NFT_V4_TOPIC_FREEZE);
                a.push_u64(0);
                a.push_u64(0);
                a.push_u64(S_LOG);
                a.op(Opcode::Log2);
                a.op(Opcode::Stop);
            }
            SELECTOR_NFT_METADATA_HASH => {
                a.copy(32, S_TID);
                a.sload_tid(&REF_NFT_METADATA_STORAGE_XOR);
                a.return_top();
            }
            SELECTOR_NFT_TOTAL_MINTED => {
                a.push(&ref_nft_v4_total_minted_key());
                a.op(Opcode::SLoad);
                a.return_top();
            }
            SELECTOR_NFT_IS_METADATA_FROZEN => {
                a.copy(32, S_TID);
                a.push(&ref_nft_v4_frozen_all_key());
                a.op(Opcode::SLoad);
                a.sload_tid(&REF_NFT_V4_FROZEN_XOR);
                a.op(Opcode::Or);
                a.return_top();
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
pub fn reference_nft_v4_init_bytecode(runtime: &[u8]) -> Vec<u8> {
    let mut a = Asm {
        c: Vec::new(),
        faults: Vec::new(),
    };
    a.op(Opcode::Caller);
    a.push(&ref_nft_collection_admin_key());
    a.op(Opcode::SStore);
    let mut rt = runtime.to_vec();
    while !rt.len().is_multiple_of(32) {
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
///
/// Apps that also claim the collection in the linked NFT↔token registry wrap
/// [`reference_nft_v4_runtime_bytecode`] in their own init (admin slot `0x…e0` = creator, then
/// `claim_self`), exactly as they did for v3.
#[must_use]
pub fn reference_nft_v4_deploy_bytecode() -> Vec<u8> {
    let rt = reference_nft_v4_runtime_bytecode();
    let mut out = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
    out.extend(reference_nft_v4_init_bytecode(&rt));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference_nft::{
        encode_mint_batch_calldata, encode_owner_of_calldata, encode_set_metadata_hash_calldata,
        encode_transfer_nft_calldata,
    };
    use boing_primitives::Account;
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;
    use crate::vm::{VmError, GAS_PER_CONTRACT_CALL};

    const ADMIN: AccountId = AccountId([0xad; 32]);
    const ALICE: AccountId = AccountId([0xa1; 32]);
    const BOB: AccountId = AccountId([0xb2; 32]);
    const CAROL: AccountId = AccountId([0xc3; 32]);
    const NFT: AccountId = AccountId([0x4e; 32]);

    fn tid(n: u64) -> [u8; 32] {
        word_u64(n)
    }

    fn h(n: u8) -> [u8; 32] {
        [n; 32]
    }

    fn deploy() -> StateStore {
        let mut st = StateStore::new();
        st.insert(Account {
            id: NFT,
            state: Default::default(),
        });
        let deploy = reference_nft_v4_deploy_bytecode();
        assert_eq!(deploy[0], CONTRACT_DEPLOY_INIT_CODE_MARKER);
        let mut it = Interpreter::new(deploy[1..].to_vec(), GAS_PER_CONTRACT_CALL);
        it.run(ADMIN, NFT, &[], &mut st).unwrap();
        let rt = it.return_data.expect("init returns runtime");
        assert_eq!(rt, reference_nft_v4_runtime_bytecode());
        st.set_contract_code(NFT, rt);
        st
    }

    /// Runs like a transaction: a fault reverts all state.
    fn call(st: &mut StateStore, who: AccountId, cd: &[u8]) -> Result<Interpreter, VmError> {
        let cp = st.checkpoint();
        let code = st.get_contract_code(&NFT).unwrap().clone();
        let mut it = Interpreter::new(code, GAS_PER_CONTRACT_CALL);
        match it.run(who, NFT, cd, st) {
            Ok(_) => Ok(it),
            Err(e) => {
                st.revert(cp);
                Err(e)
            }
        }
    }

    fn fails(st: &mut StateStore, who: AccountId, cd: &[u8]) {
        assert!(call(st, who, cd).is_err(), "expected the call to fault");
    }

    fn word(st: &mut StateStore, cd: &[u8]) -> [u8; 32] {
        let it = call(st, CAROL, cd).unwrap();
        it.return_data.as_deref().unwrap().try_into().unwrap()
    }

    fn owner(st: &mut StateStore, n: u64) -> AccountId {
        AccountId(word(st, &encode_owner_of_calldata(&tid(n))))
    }

    fn minted(ids: &[u64], to: AccountId) -> StateStore {
        let mut st = deploy();
        let tids: Vec<_> = ids.iter().map(|n| tid(*n)).collect();
        let hashes: Vec<_> = ids.iter().map(|n| h(*n as u8)).collect();
        call(
            &mut st,
            ADMIN,
            &encode_mint_batch_calldata(&to, &tids, &hashes),
        )
        .unwrap();
        st
    }

    #[test]
    fn init_sets_deployer_admin_and_no_lazy_admin() {
        let mut st = deploy();
        assert_eq!(word(&mut st, &encode_nft_admin_calldata()), ADMIN.0);
        // A runtime installed without init has no admin, and the first caller does not become one.
        let mut raw = StateStore::new();
        raw.insert(Account {
            id: NFT,
            state: Default::default(),
        });
        raw.set_contract_code(NFT, reference_nft_v4_runtime_bytecode());
        fails(
            &mut raw,
            ALICE,
            &encode_mint_batch_calldata(&ALICE, &[tid(1)], &[h(1)]),
        );
        assert_eq!(word(&mut raw, &encode_nft_admin_calldata()), [0u8; 32]);
    }

    #[test]
    fn mint_batch_sets_owners_metadata_counter_and_logs() {
        let mut st = deploy();
        let ids: Vec<_> = (1..=40).map(tid).collect();
        let hashes: Vec<_> = (1..=40u8)
            .map(|n| if n == 2 { [0; 32] } else { h(n) })
            .collect();
        let it = call(
            &mut st,
            ADMIN,
            &encode_mint_batch_calldata(&ALICE, &ids, &hashes),
        )
        .unwrap();
        assert_eq!(it.logs.len(), 2, "40 ids → one log of 32 and one of 8");
        assert_eq!(
            it.logs[0].topics,
            vec![REF_NFT_V4_TOPIC_MINT_BATCH, ALICE.0]
        );
        assert_eq!(it.logs[0].data.len(), 32 * 32);
        assert_eq!(it.logs[1].data.len(), 8 * 32);
        assert_eq!(&it.logs[1].data[..32], &tid(33));
        assert_eq!(owner(&mut st, 40), ALICE);
        assert_eq!(
            word(&mut st, &encode_nft_metadata_hash_calldata(&tid(3))),
            h(3)
        );
        assert_eq!(
            word(&mut st, &encode_nft_metadata_hash_calldata(&tid(2))),
            [0; 32]
        );
        assert_eq!(
            word(&mut st, &encode_nft_total_minted_calldata()),
            word_u64(40)
        );
    }

    #[test]
    fn mint_batch_faults_on_bad_input_and_non_admin() {
        let mut st = minted(&[1], ALICE);
        fails(
            &mut st,
            ALICE,
            &encode_mint_batch_calldata(&ALICE, &[tid(2)], &[h(2)]),
        );
        fails(
            &mut st,
            ADMIN,
            &encode_mint_batch_calldata(&ALICE, &[tid(1)], &[h(1)]),
        );
        fails(
            &mut st,
            ADMIN,
            &encode_mint_batch_calldata(&ALICE, &[tid(5), tid(5)], &[h(5), h(5)]),
        );
        fails(
            &mut st,
            ADMIN,
            &encode_mint_batch_calldata(&AccountId([0; 32]), &[tid(6)], &[h(6)]),
        );
        // Atomic: nothing from the failed batches persisted.
        assert_eq!(owner(&mut st, 5), AccountId([0; 32]));
        assert_eq!(
            word(&mut st, &encode_nft_total_minted_calldata()),
            word_u64(1)
        );
    }

    #[test]
    fn mint_batch_at_max_fits_call_gas_and_log_cap() {
        let mut st = deploy();
        let n = MAX_REFERENCE_NFT_V4_MINT_BATCH as u64;
        let ids: Vec<_> = (1..=n).map(tid).collect();
        let hashes: Vec<_> = (1..=n).map(|i| h((i % 250 + 1) as u8)).collect();
        let it = call(
            &mut st,
            ADMIN,
            &encode_mint_batch_calldata(&BOB, &ids, &hashes),
        )
        .unwrap();
        assert_eq!(it.logs.len(), 16);
        assert_eq!(owner(&mut st, n), BOB);
    }

    #[test]
    fn transfer_by_owner_logs_and_by_stranger_faults() {
        let mut st = minted(&[1], ALICE);
        // The v1–v3 bug: an unauthorized transfer must fault, not STOP with success.
        fails(&mut st, BOB, &encode_transfer_nft_calldata(&BOB, &tid(1)));
        fails(&mut st, ADMIN, &encode_transfer_nft_calldata(&BOB, &tid(1)));
        // No lazy mint through transfer_nft.
        fails(&mut st, ADMIN, &encode_transfer_nft_calldata(&BOB, &tid(9)));
        fails(
            &mut st,
            ALICE,
            &encode_transfer_nft_calldata(&AccountId([0; 32]), &tid(1)),
        );
        let it = call(&mut st, ALICE, &encode_transfer_nft_calldata(&BOB, &tid(1))).unwrap();
        assert_eq!(it.logs.len(), 1);
        assert_eq!(
            it.logs[0].topics,
            vec![REF_NFT_V4_TOPIC_TRANSFER, ALICE.0, BOB.0]
        );
        assert_eq!(it.logs[0].data, tid(1).to_vec());
        assert_eq!(owner(&mut st, 1), BOB);
    }

    #[test]
    fn approved_spender_can_transfer_once() {
        let mut st = minted(&[1], ALICE);
        fails(&mut st, BOB, &encode_nft_approve_calldata(&BOB, &tid(1)));
        let it = call(
            &mut st,
            ALICE,
            &encode_nft_approve_calldata(&CAROL, &tid(1)),
        )
        .unwrap();
        assert_eq!(
            it.logs[0].topics,
            vec![REF_NFT_V4_TOPIC_APPROVAL, ALICE.0, CAROL.0]
        );
        assert_eq!(
            word(&mut st, &encode_nft_get_approved_calldata(&tid(1))),
            CAROL.0
        );
        call(&mut st, CAROL, &encode_transfer_nft_calldata(&BOB, &tid(1))).unwrap();
        assert_eq!(owner(&mut st, 1), BOB);
        assert_eq!(
            word(&mut st, &encode_nft_get_approved_calldata(&tid(1))),
            [0; 32]
        );
        fails(
            &mut st,
            CAROL,
            &encode_transfer_nft_calldata(&CAROL, &tid(1)),
        );
    }

    #[test]
    fn operator_can_transfer_and_approve_until_revoked() {
        let mut st = minted(&[1, 2], ALICE);
        let it = call(
            &mut st,
            ALICE,
            &encode_nft_set_approval_for_all_calldata(&CAROL, true),
        )
        .unwrap();
        assert_eq!(
            it.logs[0].topics,
            vec![REF_NFT_V4_TOPIC_APPROVAL_FOR_ALL, ALICE.0, CAROL.0]
        );
        assert_eq!(
            word(
                &mut st,
                &encode_nft_is_approved_for_all_calldata(&ALICE, &CAROL)
            ),
            word_u64(1)
        );
        assert_eq!(
            word(
                &mut st,
                &encode_nft_is_approved_for_all_calldata(&BOB, &CAROL)
            ),
            word_u64(0)
        );
        call(&mut st, CAROL, &encode_transfer_nft_calldata(&BOB, &tid(1))).unwrap();
        call(&mut st, CAROL, &encode_nft_approve_calldata(&BOB, &tid(2))).unwrap();
        call(
            &mut st,
            ALICE,
            &encode_nft_set_approval_for_all_calldata(&CAROL, false),
        )
        .unwrap();
        fails(
            &mut st,
            CAROL,
            &encode_transfer_nft_calldata(&CAROL, &tid(2)),
        );
        // Operator rights are per owner: CAROL is not BOB's operator.
        fails(
            &mut st,
            CAROL,
            &encode_transfer_nft_calldata(&CAROL, &tid(1)),
        );
        let mut bad = encode_nft_set_approval_for_all_calldata(&CAROL, true);
        bad[95] = 2;
        fails(&mut st, ALICE, &bad);
        assert_eq!(
            ref_nft_v4_operator_storage_key(&ALICE, &CAROL),
            ref_nft_v4_operator_storage_key(&ALICE, &CAROL)
        );
        assert_ne!(
            ref_nft_v4_operator_storage_key(&ALICE, &CAROL),
            ref_nft_v4_operator_storage_key(&CAROL, &ALICE)
        );
    }

    #[test]
    fn operator_slot_matches_documented_key() {
        let mut st = minted(&[1], ALICE);
        call(
            &mut st,
            ALICE,
            &encode_nft_set_approval_for_all_calldata(&CAROL, true),
        )
        .unwrap();
        let slot = st.get_contract_storage(&NFT, &ref_nft_v4_operator_storage_key(&ALICE, &CAROL));
        assert_eq!(slot, word_u64(1));
        call(&mut st, ALICE, &encode_nft_approve_calldata(&BOB, &tid(1))).unwrap();
        let slot = st.get_contract_storage(&NFT, &ref_nft_v4_approved_storage_key(&tid(1)));
        assert_eq!(slot, BOB.0);
    }

    #[test]
    fn metadata_is_admin_only_and_freezable() {
        let mut st = minted(&[1, 2], ALICE);
        // Holder can no longer rewrite metadata (v1–v3 allowed it).
        fails(
            &mut st,
            ALICE,
            &encode_set_metadata_hash_calldata(&tid(1), &h(9)),
        );
        fails(
            &mut st,
            ADMIN,
            &encode_set_metadata_hash_calldata(&tid(7), &h(9)),
        );
        let it = call(
            &mut st,
            ADMIN,
            &encode_set_metadata_hash_calldata(&tid(1), &h(9)),
        )
        .unwrap();
        assert_eq!(it.logs[0].topics, vec![REF_NFT_V4_TOPIC_METADATA, tid(1)]);
        assert_eq!(it.logs[0].data, h(9).to_vec());
        call(
            &mut st,
            ADMIN,
            &encode_nft_freeze_metadata_calldata(&tid(1)),
        )
        .unwrap();
        fails(
            &mut st,
            ADMIN,
            &encode_set_metadata_hash_calldata(&tid(1), &h(8)),
        );
        assert_eq!(
            word(&mut st, &encode_nft_is_metadata_frozen_calldata(&tid(1))),
            word_u64(1)
        );
        assert_eq!(
            word(&mut st, &encode_nft_is_metadata_frozen_calldata(&tid(2))),
            word_u64(0)
        );
        call(
            &mut st,
            ADMIN,
            &encode_set_metadata_hash_calldata(&tid(2), &h(8)),
        )
        .unwrap();
        fails(&mut st, ALICE, &encode_nft_freeze_all_metadata_calldata());
        call(&mut st, ADMIN, &encode_nft_freeze_all_metadata_calldata()).unwrap();
        fails(
            &mut st,
            ADMIN,
            &encode_set_metadata_hash_calldata(&tid(2), &h(7)),
        );
        assert_eq!(
            word(&mut st, &encode_nft_is_metadata_frozen_calldata(&tid(2))),
            word_u64(1)
        );
        assert_eq!(
            word(&mut st, &encode_nft_metadata_hash_calldata(&tid(2))),
            h(8)
        );
    }

    #[test]
    fn transfer_admin_moves_control() {
        let mut st = minted(&[1], ALICE);
        fails(&mut st, ALICE, &encode_nft_transfer_admin_calldata(&ALICE));
        fails(
            &mut st,
            ADMIN,
            &encode_nft_transfer_admin_calldata(&AccountId([0; 32])),
        );
        let it = call(&mut st, ADMIN, &encode_nft_transfer_admin_calldata(&BOB)).unwrap();
        assert_eq!(
            it.logs[0].topics,
            vec![REF_NFT_V4_TOPIC_ADMIN, ADMIN.0, BOB.0]
        );
        assert_eq!(word(&mut st, &encode_nft_admin_calldata()), BOB.0);
        fails(
            &mut st,
            ADMIN,
            &encode_mint_batch_calldata(&ADMIN, &[tid(2)], &[h(2)]),
        );
        call(
            &mut st,
            BOB,
            &encode_mint_batch_calldata(&BOB, &[tid(2)], &[h(2)]),
        )
        .unwrap();
    }

    #[test]
    fn unknown_selector_and_dirty_selector_word_fault() {
        let mut st = deploy();
        fails(&mut st, ALICE, &selector_word(0x42));
        fails(&mut st, ALICE, &[]);
        let mut dirty = encode_nft_admin_calldata();
        dirty[0] = 1;
        fails(&mut st, ALICE, &dirty);
        // Over-long fixed-size calls fault instead of being silently truncated.
        let mut long = encode_transfer_nft_calldata(&BOB, &tid(1));
        long.extend_from_slice(&h(1));
        fails(&mut st, ALICE, &long);
    }

    #[test]
    fn reads_of_unminted_tokens_return_zero() {
        let mut st = deploy();
        assert_eq!(owner(&mut st, 1), AccountId([0; 32]));
        assert_eq!(
            word(&mut st, &encode_nft_get_approved_calldata(&tid(1))),
            [0; 32]
        );
        assert_eq!(word(&mut st, &encode_nft_total_minted_calldata()), [0; 32]);
    }

    /// Golden vectors shared with `boing-sdk/tests/referenceNftV4.test.ts`.
    #[test]
    fn golden_keys_and_topics() {
        let hx = |w: [u8; 32]| w.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(
            hx(ref_nft_v4_operator_storage_key(&ALICE, &CAROL)),
            "e3eee8efe6fef3e4e7efe7f5feeef1e49182978c919c95f7c3c3c3c3c3c3c3c3"
        );
        assert_eq!(
            hx(REF_NFT_V4_TOPIC_TRANSFER),
            "424f494e475f5245464e46545f5452414e534645525f56340000000000000000"
        );
    }

    #[test]
    fn deploy_and_runtime_pass_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult};
        for code in [
            reference_nft_v4_deploy_bytecode(),
            reference_nft_v4_runtime_bytecode(),
        ] {
            let r = check_contract_deploy_full(
                &code,
                Some("nft"),
                None,
                &boing_qa::RuleRegistry::new(),
            );
            assert!(
                matches!(r, QaResult::Allow | QaResult::Unsure),
                "v4 bytecode must pass default QA: {r:?}"
            );
        }
    }
}
