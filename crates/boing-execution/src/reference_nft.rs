//! Reference **NFT** calldata layout (Boing-defined) for wallets and indexers.
//!
//! Not consensus-enforced — see `docs/BOING-REFERENCE-NFT.md`.
//!
//! [`reference_nft_collection_template_bytecode`] is a minimal **collection** contract: lazy admin
//! (first caller), `owner_of` / `transfer_nft` / `set_metadata_hash` / `mint_batch` per the reference doc.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// `owner_of(token_id)` — read path; contract returns current holder `AccountId` (e.g. via `RETURN`).
pub const SELECTOR_OWNER_OF: u8 = 0x03;
/// `transfer_nft(to, token_id)` — move `token_id` to `to` if `CALLER` is authorized.
pub const SELECTOR_TRANSFER_NFT: u8 = 0x04;
/// Optional: bind a 32-byte metadata commitment (URI hash, etc.) to `token_id`.
pub const SELECTOR_SET_METADATA_HASH: u8 = 0x05;
/// `mint_batch(to, token_ids[], metadata_hashes[])` — admin-only atomic multi-mint (template v2).
pub const SELECTOR_MINT_BATCH: u8 = 0x06;

/// Bytecode hard cap on `n` for [`SELECTOR_MINT_BATCH`]. Call gas must still cover `n` `SSTORE`s.
pub const MAX_REFERENCE_NFT_MINT_BATCH: u16 = 50;

/// Opaque token id as a full 32-byte big-endian word (contract defines encoding).
pub fn token_id_word(id: &[u8; 32]) -> [u8; 32] {
    *id
}

/// Reference `owner_of(token_id)` calldata (96 bytes): selector + `token_id` + zero padding word.
pub fn encode_owner_of_calldata(token_id: &[u8; 32]) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_OWNER_OF).to_vec();
    v.extend_from_slice(&token_id_word(token_id));
    v.extend_from_slice(&[0u8; 32]);
    v
}

/// Reference `transfer_nft(to, token_id)` calldata (96 bytes).
pub fn encode_transfer_nft_calldata(to: &AccountId, token_id: &[u8; 32]) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_TRANSFER_NFT).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&token_id_word(token_id));
    v
}

/// Reference `set_metadata_hash(token_id, hash)` calldata (96 bytes).
pub fn encode_set_metadata_hash_calldata(token_id: &[u8; 32], metadata_hash: &[u8; 32]) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_SET_METADATA_HASH).to_vec();
    v.extend_from_slice(&token_id_word(token_id));
    v.extend_from_slice(metadata_hash);
    v
}

/// `mint_batch` calldata: `96 + 64n` bytes (selector, `to`, `n`, then `n` ids, then `n` hashes).
///
/// Layout: word0 selector `0x06`; word1 `to`; word2 `n` as big-endian **u64 in the low 8 bytes**;
/// then `token_ids[0..n)` then `metadata_hashes[0..n)`. Panics if lengths differ, `n == 0`, or
/// `n > MAX_REFERENCE_NFT_MINT_BATCH`.
pub fn encode_mint_batch_calldata(
    to: &AccountId,
    token_ids: &[[u8; 32]],
    metadata_hashes: &[[u8; 32]],
) -> Vec<u8> {
    assert_eq!(
        token_ids.len(),
        metadata_hashes.len(),
        "token_ids and metadata_hashes length mismatch"
    );
    let n = token_ids.len();
    assert!(n >= 1, "mint_batch n must be >= 1");
    assert!(
        n <= MAX_REFERENCE_NFT_MINT_BATCH as usize,
        "mint_batch n exceeds MAX_REFERENCE_NFT_MINT_BATCH"
    );
    let mut v = selector_word(SELECTOR_MINT_BATCH).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&word_u64(n as u64));
    for id in token_ids {
        v.extend_from_slice(id);
    }
    for h in metadata_hashes {
        v.extend_from_slice(h);
    }
    v
}

// --- Minimal collection VM bytecode (scratch memory, big-endian words) ---
// Calldata for `mint_batch` n=50 is `96+64*50 = 3296` bytes at mem[0..). Scratch must sit above that.

const MEM_SCRATCH_SEL: u64 = 4160;
const MEM_SCRATCH_TO: u64 = 4128;
const MEM_SCRATCH_TID: u64 = 4192;
const MEM_SCRATCH_HASH: u64 = 4224;
const MEM_SCRATCH_OWNER: u64 = 4096;
const MEM_RET_OWNER_OF: u64 = 4256;
const MEM_BATCH_I: u64 = 4352;
const MEM_BATCH_N: u64 = 4384;
const MEM_BATCH_TO: u64 = 4416;
const MEM_BATCH_TID: u64 = 4448;
const MEM_BATCH_J: u64 = 4480;

/// Singleton storage key: **lazy admin** — first caller becomes admin when this slot is zero.
#[must_use]
pub fn ref_nft_collection_admin_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xe0;
    k
}

/// XOR mask for **owner** ledger slot: `storage_key = token_id_word ^ REF_NFT_OWNER_STORAGE_XOR`.
pub const REF_NFT_OWNER_STORAGE_XOR: [u8; 32] =
    *b"BOING_REFNFT_OWNER01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// XOR mask for **metadata hash** slot: `storage_key = token_id_word ^ REF_NFT_METADATA_STORAGE_XOR`.
pub const REF_NFT_METADATA_STORAGE_XOR: [u8; 32] =
    *b"BOING_REFNFT_META01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

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

fn emit_push32_jumpi(code: &mut Vec<u8>) -> usize {
    let at = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);
    at
}

fn emit_push32_jump(code: &mut Vec<u8>) -> usize {
    let at = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::Jump as u8);
    at
}

fn mload_at(code: &mut Vec<u8>, off: u64) {
    push32(code, &word_u64(off));
    code.push(Opcode::MLoad as u8);
}

fn mstore_at(code: &mut Vec<u8>, off: u64) {
    push32(code, &word_u64(off));
    code.push(Opcode::MStore as u8);
}

/// Load `token_ids[index_mem]` — `MLOAD(96 + 32 * mem[index_mem])`.
fn emit_load_token_id_at_index(code: &mut Vec<u8>, index_mem: u64) {
    mload_at(code, index_mem);
    push32(code, &word_u64(32));
    code.push(Opcode::Mul as u8);
    push32(code, &word_u64(96));
    code.push(Opcode::Add as u8);
    code.push(Opcode::MLoad as u8);
}

/// Minimal **reference NFT collection** bytecode for `contract_deploy_meta` with purpose **`nft`** / **`NFT`**.
///
/// Semantics:
/// - **Admin:** first successful call initializes [`ref_nft_collection_admin_key`] to `CALLER`.
/// - **`owner_of`:** returns `SLOAD(token_id ^ [`REF_NFT_OWNER_STORAGE_XOR`])` (zero word if unminted).
/// - **`transfer_nft`:** if unowned, only admin may set owner to `to` (lazy mint). If owned, only owner may transfer.
/// - **`set_metadata_hash`:** admin **or** current owner may write `token_id ^ [`REF_NFT_METADATA_STORAGE_XOR`]`.
/// - **`mint_batch`:** admin only; check-all-then-write. Failures **`JUMP`** out of bounds (`InvalidJump`) so
///   the executor rolls back storage (do **not** `STOP` after a prefix). Zero metadata hashes skip `SSTORE`.
///   `to == 0` faults (v2-only). Lazy admin `SSTORE` still runs at the start of the call if the slot is empty.
#[must_use]
pub fn reference_nft_collection_template_bytecode() -> Vec<u8> {
    let mut c = Vec::new();

    // Lazy admin: if admin slot zero, set to CALLER.
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_nft_collection_admin_key());
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
    push32(&mut c, &ref_nft_collection_admin_key());
    c.push(Opcode::SStore as u8);

    let off_dispatch = c.len();
    patch_push32_dest(&mut c, fix_skip_init, off_dispatch);

    // mem[MEM_SCRATCH_SEL] = calldata selector (low byte only as word)
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &mask_low_byte());
    c.push(Opcode::And as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MStore as u8);

    // -- set_metadata (0x05): if selector != 5, skip past this whole block (JumpI/ Jump must not share
    //    the same dest — `off_not == off_go` when only `patch` runs between `c.len()` calls).
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_SET_METADATA_HASH));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_meta = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // tid, hash → scratch
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TID));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_HASH));
    c.push(Opcode::MStore as u8);

    // if CALLER == admin → store; else require CALLER == owner
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_nft_collection_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_meta_owner_check = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    let fix_meta_admin_to_store = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);
    let off_meta_owner_check = c.len();
    patch_push32_dest(&mut c, fix_meta_owner_check, off_meta_owner_check);

    push32(&mut c, &word_u64(MEM_SCRATCH_TID));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_NFT_OWNER_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_OWNER));
    c.push(Opcode::MStore as u8);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_OWNER));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_meta_fail = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    let fix_meta_owner_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    let off_meta_abort = c.len();
    patch_push32_dest(&mut c, fix_meta_fail, off_meta_abort);
    c.push(Opcode::Stop as u8);

    let off_meta_store = c.len();
    patch_push32_dest(&mut c, fix_meta_admin_to_store, off_meta_store);
    patch_push32_dest(&mut c, fix_meta_owner_ok, off_meta_store);
    // SStore: pop key (top), then value — stack: value, key (key on top)
    push32(&mut c, &word_u64(MEM_SCRATCH_HASH));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TID));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_NFT_METADATA_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let off_after_meta = c.len();
    patch_push32_dest(&mut c, fix_skip_meta, off_after_meta);

    // -- transfer_nft (0x04): skip entire block if selector != 4
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_TRANSFER_NFT));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_xfer = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TID));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_TID));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_NFT_OWNER_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_OWNER));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_OWNER));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_xfer_mint = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // existing owner: require caller == owner
    c.push(Opcode::Caller as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_OWNER));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_xfer_fail_owned = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TID));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_NFT_OWNER_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let off_xfer_mint = c.len();
    patch_push32_dest(&mut c, fix_xfer_mint, off_xfer_mint);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_nft_collection_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_xfer_fail_mint = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TID));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_NFT_OWNER_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let off_xfer_abort = c.len();
    patch_push32_dest(&mut c, fix_xfer_fail_owned, off_xfer_abort);
    patch_push32_dest(&mut c, fix_xfer_fail_mint, off_xfer_abort);
    c.push(Opcode::Stop as u8);

    let off_after_xfer = c.len();
    patch_push32_dest(&mut c, fix_skip_xfer, off_after_xfer);

    // -- owner_of (0x03): if selector != 3, skip to unknown (Stop)
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_OWNER_OF));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_owner_of = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_NFT_OWNER_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_RET_OWNER_OF));
    c.push(Opcode::MStore as u8);
    // Return pops offset (top), then size.
    c.push(Opcode::Push1 as u8);
    c.push(32);
    push32(&mut c, &word_u64(MEM_RET_OWNER_OF));
    c.push(Opcode::Return as u8);

    let off_mint_batch = c.len();
    patch_push32_dest(&mut c, fix_skip_owner_of, off_mint_batch);
    emit_reference_nft_mint_batch(&mut c);

    c
}

/// `mint_batch` (selector `0x06`): two-phase mint. On any check failure, `JUMP` to `u64::MAX` (`InvalidJump`).
fn emit_reference_nft_mint_batch(c: &mut Vec<u8>) {
    let mut fault_jumpis: Vec<usize> = Vec::new();

    mload_at(c, MEM_SCRATCH_SEL);
    push32(c, &selector_word(SELECTOR_MINT_BATCH));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_batch = emit_push32_jumpi(c);

    // to, n → scratch
    mload_at(c, 32);
    mstore_at(c, MEM_BATCH_TO);
    mload_at(c, 64);
    mstore_at(c, MEM_BATCH_N);

    // n must fit in low 8 bytes (high 24 bytes of the word zero).
    mload_at(c, MEM_BATCH_N);
    push32(c, &word_u64(64));
    c.push(Opcode::Shr as u8);
    c.push(Opcode::IsZero as u8);
    c.push(Opcode::IsZero as u8);
    fault_jumpis.push(emit_push32_jumpi(c));

    // n == 0
    mload_at(c, MEM_BATCH_N);
    c.push(Opcode::IsZero as u8);
    fault_jumpis.push(emit_push32_jumpi(c));

    // n > MAX
    mload_at(c, MEM_BATCH_N);
    push32(c, &word_u64(MAX_REFERENCE_NFT_MINT_BATCH as u64));
    c.push(Opcode::Gt as u8);
    fault_jumpis.push(emit_push32_jumpi(c));

    // to == 0 (v2)
    mload_at(c, MEM_BATCH_TO);
    c.push(Opcode::IsZero as u8);
    fault_jumpis.push(emit_push32_jumpi(c));

    // extra trailing nonzero word at offset 96+64n (zero padding is indistinguishable without CALLDATASIZE)
    mload_at(c, MEM_BATCH_N);
    push32(c, &word_u64(64));
    c.push(Opcode::Mul as u8);
    push32(c, &word_u64(96));
    c.push(Opcode::Add as u8);
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    c.push(Opcode::IsZero as u8);
    fault_jumpis.push(emit_push32_jumpi(c));

    // caller == admin
    c.push(Opcode::Caller as u8);
    push32(c, &ref_nft_collection_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    fault_jumpis.push(emit_push32_jumpi(c));

    // i = 0
    push32(c, &word_u64(0));
    mstore_at(c, MEM_BATCH_I);

    let off_check_loop = c.len();
    // if i >= n → write pass
    mload_at(c, MEM_BATCH_I);
    mload_at(c, MEM_BATCH_N);
    c.push(Opcode::Lt as u8);
    c.push(Opcode::IsZero as u8);
    let fix_to_write = emit_push32_jumpi(c);

    emit_load_token_id_at_index(c, MEM_BATCH_I);
    c.push(Opcode::Dup1 as u8);
    mstore_at(c, MEM_BATCH_TID);

    // owned? fault
    push32(c, &REF_NFT_OWNER_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    c.push(Opcode::IsZero as u8);
    fault_jumpis.push(emit_push32_jumpi(c));

    // uniqueness: j = 0 .. i-1
    push32(c, &word_u64(0));
    mstore_at(c, MEM_BATCH_J);
    let off_uniq = c.len();
    mload_at(c, MEM_BATCH_J);
    mload_at(c, MEM_BATCH_I);
    c.push(Opcode::Lt as u8);
    c.push(Opcode::IsZero as u8);
    let fix_uniq_done = emit_push32_jumpi(c);
    emit_load_token_id_at_index(c, MEM_BATCH_J);
    mload_at(c, MEM_BATCH_TID);
    c.push(Opcode::Eq as u8);
    fault_jumpis.push(emit_push32_jumpi(c));
    mload_at(c, MEM_BATCH_J);
    push32(c, &word_u64(1));
    c.push(Opcode::Add as u8);
    mstore_at(c, MEM_BATCH_J);
    let fix_uniq_back = emit_push32_jump(c);
    patch_push32_dest(c, fix_uniq_back, off_uniq);

    let off_uniq_done = c.len();
    patch_push32_dest(c, fix_uniq_done, off_uniq_done);
    mload_at(c, MEM_BATCH_I);
    push32(c, &word_u64(1));
    c.push(Opcode::Add as u8);
    mstore_at(c, MEM_BATCH_I);
    let fix_check_back = emit_push32_jump(c);
    patch_push32_dest(c, fix_check_back, off_check_loop);

    let off_write = c.len();
    patch_push32_dest(c, fix_to_write, off_write);
    push32(c, &word_u64(0));
    mstore_at(c, MEM_BATCH_I);

    let off_write_loop = c.len();
    mload_at(c, MEM_BATCH_I);
    mload_at(c, MEM_BATCH_N);
    c.push(Opcode::Lt as u8);
    c.push(Opcode::IsZero as u8);
    let fix_write_done = emit_push32_jumpi(c);

    emit_load_token_id_at_index(c, MEM_BATCH_I);
    c.push(Opcode::Dup1 as u8);
    mstore_at(c, MEM_BATCH_TID);
    // SSTORE owner: value then key
    mload_at(c, MEM_BATCH_TO);
    mload_at(c, MEM_BATCH_TID);
    push32(c, &REF_NFT_OWNER_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);

    // hash at 96 + 32*n + 32*i
    mload_at(c, MEM_BATCH_N);
    mload_at(c, MEM_BATCH_I);
    c.push(Opcode::Add as u8);
    push32(c, &word_u64(32));
    c.push(Opcode::Mul as u8);
    push32(c, &word_u64(96));
    c.push(Opcode::Add as u8);
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Dup1 as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_meta = emit_push32_jumpi(c);
    mload_at(c, MEM_BATCH_TID);
    push32(c, &REF_NFT_METADATA_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);
    let fix_after_meta = emit_push32_jump(c);

    // Zero hash: discard via MSTORE (no POP opcode).
    let off_skip_meta = c.len();
    patch_push32_dest(c, fix_skip_meta, off_skip_meta);
    push32(c, &word_u64(MEM_BATCH_TID));
    c.push(Opcode::MStore as u8);

    let off_after_meta = c.len();
    patch_push32_dest(c, fix_after_meta, off_after_meta);
    mload_at(c, MEM_BATCH_I);
    push32(c, &word_u64(1));
    c.push(Opcode::Add as u8);
    mstore_at(c, MEM_BATCH_I);
    let fix_write_back = emit_push32_jump(c);
    patch_push32_dest(c, fix_write_back, off_write_loop);

    let off_write_done = c.len();
    patch_push32_dest(c, fix_write_done, off_write_done);
    c.push(Opcode::Stop as u8);

    let off_fault = c.len();
    for at in fault_jumpis {
        patch_push32_dest(c, at, off_fault);
    }
    push32(c, &word_u64(u64::MAX));
    c.push(Opcode::Jump as u8);

    let off_unknown = c.len();
    patch_push32_dest(c, fix_skip_batch, off_unknown);
    c.push(Opcode::Stop as u8);
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{Account, AccountId};
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;

    #[test]
    fn reference_nft_calldata_lengths() {
        let tid = [1u8; 32];
        let to = AccountId([2u8; 32]);
        let h = [3u8; 32];
        assert_eq!(encode_owner_of_calldata(&tid).len(), 96);
        assert_eq!(encode_transfer_nft_calldata(&to, &tid).len(), 96);
        assert_eq!(encode_set_metadata_hash_calldata(&tid, &h).len(), 96);
        let ids = [tid, [4u8; 32]];
        let hashes = [h, [5u8; 32]];
        assert_eq!(
            encode_mint_batch_calldata(&to, &ids, &hashes).len(),
            96 + 64 * 2
        );
    }

    fn xor_storage_key(token_id: &[u8; 32], tag: &[u8; 32]) -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = token_id[i] ^ tag[i];
        }
        k
    }

    #[test]
    fn reference_nft_collection_template_mint_transfer_and_metadata() {
        let deployer = AccountId([0xadu8; 32]);
        let alice = AccountId([0xbeu8; 32]);
        let collection = AccountId([0xcfu8; 32]);
        let tid = [7u8; 32];
        let meta = [8u8; 32];

        let mut state = StateStore::new();
        state.insert(Account {
            id: collection,
            state: Default::default(),
        });
        let code = reference_nft_collection_template_bytecode();
        state.set_contract_code(collection, code.clone());

        let mut run = |sender: AccountId, calldata: &[u8]| {
            let mut it = Interpreter::new(code.clone(), 5_000_000);
            it.run(sender, collection, calldata, &mut state).unwrap();
            it
        };

        // First touch: `owner_of` initializes admin = deployer; unminted → zero address.
        let it = run(deployer, &encode_owner_of_calldata(&tid));
        assert_eq!(it.return_data.as_deref(), Some(&[0u8; 32][..]));

        run(deployer, &encode_transfer_nft_calldata(&alice, &tid));

        let it = run(deployer, &encode_owner_of_calldata(&tid));
        assert_eq!(it.return_data.as_deref(), Some(&alice.0[..]));

        run(alice, &encode_transfer_nft_calldata(&deployer, &tid));

        run(deployer, &encode_set_metadata_hash_calldata(&tid, &meta));

        let ok = xor_storage_key(&tid, &REF_NFT_OWNER_STORAGE_XOR);
        assert_eq!(state.get_contract_storage(&collection, &ok), deployer.0);
        let mk = xor_storage_key(&tid, &REF_NFT_METADATA_STORAGE_XOR);
        assert_eq!(state.get_contract_storage(&collection, &mk), meta);
    }

    #[test]
    fn reference_nft_collection_non_admin_cannot_mint() {
        let deployer = AccountId([0x11u8; 32]);
        let stranger = AccountId([0x22u8; 32]);
        let collection = AccountId([0x33u8; 32]);
        let tid = [9u8; 32];

        let mut state = StateStore::new();
        state.insert(Account {
            id: collection,
            state: Default::default(),
        });
        let code = reference_nft_collection_template_bytecode();
        state.set_contract_code(collection, code.clone());

        let mut it = Interpreter::new(code.clone(), 5_000_000);
        it.run(
            deployer,
            collection,
            &encode_owner_of_calldata(&tid),
            &mut state,
        )
        .unwrap();

        let mut it = Interpreter::new(code.clone(), 5_000_000);
        it.run(
            stranger,
            collection,
            &encode_transfer_nft_calldata(&deployer, &tid),
            &mut state,
        )
        .unwrap();

        let ok = xor_storage_key(&tid, &REF_NFT_OWNER_STORAGE_XOR);
        assert_eq!(state.get_contract_storage(&collection, &ok), [0u8; 32]);
    }

    fn tid_word(n: u8) -> [u8; 32] {
        let mut t = [0u8; 32];
        t[31] = n;
        t
    }

    fn hash_word(n: u8) -> [u8; 32] {
        let mut h = [0u8; 32];
        h[0] = n;
        h[31] = 0xaa;
        h
    }

    fn collection_fixture(deployer: AccountId, collection: AccountId) -> (StateStore, Vec<u8>) {
        let mut state = StateStore::new();
        state.insert(Account {
            id: collection,
            state: Default::default(),
        });
        let code = reference_nft_collection_template_bytecode();
        state.set_contract_code(collection, code.clone());
        let mut it = Interpreter::new(code.clone(), crate::vm::GAS_PER_CONTRACT_CALL);
        it.run(
            deployer,
            collection,
            &encode_owner_of_calldata(&tid_word(1)),
            &mut state,
        )
        .unwrap();
        (state, code)
    }

    #[test]
    fn mint_batch_n2_owners_and_metadata_at_production_gas() {
        let deployer = AccountId([0x41u8; 32]);
        let to = AccountId([0x42u8; 32]);
        let collection = AccountId([0x43u8; 32]);
        let (mut state, code) = collection_fixture(deployer, collection);
        let ids = [tid_word(1), tid_word(2)];
        let hashes = [hash_word(1), hash_word(2)];
        let data = encode_mint_batch_calldata(&to, &ids, &hashes);
        let mut it = Interpreter::new(code, crate::vm::GAS_PER_CONTRACT_CALL);
        it.run(deployer, collection, &data, &mut state).unwrap();
        for (id, h) in ids.iter().zip(hashes.iter()) {
            let ok = xor_storage_key(id, &REF_NFT_OWNER_STORAGE_XOR);
            assert_eq!(state.get_contract_storage(&collection, &ok), to.0);
            let mk = xor_storage_key(id, &REF_NFT_METADATA_STORAGE_XOR);
            assert_eq!(state.get_contract_storage(&collection, &mk), *h);
        }
    }

    #[test]
    fn mint_batch_n_max_at_production_gas_including_zero_hash_skip() {
        let deployer = AccountId([0x51u8; 32]);
        let to = AccountId([0x52u8; 32]);
        let collection = AccountId([0x53u8; 32]);
        let (mut state, code) = collection_fixture(deployer, collection);
        let n = MAX_REFERENCE_NFT_MINT_BATCH as usize;
        let ids: Vec<[u8; 32]> = (1..=n as u8).map(tid_word).collect();
        let mut hashes: Vec<[u8; 32]> = (1..=n as u8).map(hash_word).collect();
        hashes[0] = [0u8; 32];
        let data = encode_mint_batch_calldata(&to, &ids, &hashes);
        let mut it = Interpreter::new(code, crate::vm::GAS_PER_CONTRACT_CALL);
        it.run(deployer, collection, &data, &mut state).unwrap();
        for (i, id) in ids.iter().enumerate() {
            let ok = xor_storage_key(id, &REF_NFT_OWNER_STORAGE_XOR);
            assert_eq!(state.get_contract_storage(&collection, &ok), to.0);
            let mk = xor_storage_key(id, &REF_NFT_METADATA_STORAGE_XOR);
            let expected = if i == 0 { [0u8; 32] } else { hashes[i] };
            assert_eq!(state.get_contract_storage(&collection, &mk), expected);
        }
    }

    #[test]
    fn mint_batch_already_owned_is_atomic_invalid_jump() {
        let deployer = AccountId([0x61u8; 32]);
        let to = AccountId([0x62u8; 32]);
        let collection = AccountId([0x63u8; 32]);
        let (mut state, code) = collection_fixture(deployer, collection);
        let n = MAX_REFERENCE_NFT_MINT_BATCH as usize;
        let ids: Vec<[u8; 32]> = (1..=n as u8).map(tid_word).collect();
        let hashes: Vec<[u8; 32]> = (1..=n as u8).map(hash_word).collect();
        let owned = ids[2];
        let mut it = Interpreter::new(code.clone(), crate::vm::GAS_PER_CONTRACT_CALL);
        it.run(
            deployer,
            collection,
            &encode_transfer_nft_calldata(&deployer, &owned),
            &mut state,
        )
        .unwrap();

        let mut it = Interpreter::new(code, crate::vm::GAS_PER_CONTRACT_CALL);
        let err = it
            .run(
                deployer,
                collection,
                &encode_mint_batch_calldata(&to, &ids, &hashes),
                &mut state,
            )
            .unwrap_err();
        assert!(matches!(err, crate::vm::VmError::InvalidJump));
        for (i, id) in ids.iter().enumerate() {
            let ok = xor_storage_key(id, &REF_NFT_OWNER_STORAGE_XOR);
            let got = state.get_contract_storage(&collection, &ok);
            if i == 2 {
                assert_eq!(got, deployer.0);
            } else {
                assert_eq!(got, [0u8; 32], "partial mint at index {i}");
            }
        }
    }

    #[test]
    fn mint_batch_non_admin_invalid_jump() {
        let deployer = AccountId([0x71u8; 32]);
        let stranger = AccountId([0x72u8; 32]);
        let collection = AccountId([0x73u8; 32]);
        let (mut state, code) = collection_fixture(deployer, collection);
        let ids = [tid_word(1), tid_word(2)];
        let hashes = [hash_word(1), hash_word(2)];
        let mut it = Interpreter::new(code, crate::vm::GAS_PER_CONTRACT_CALL);
        let err = it
            .run(
                stranger,
                collection,
                &encode_mint_batch_calldata(&deployer, &ids, &hashes),
                &mut state,
            )
            .unwrap_err();
        assert!(matches!(err, crate::vm::VmError::InvalidJump));
        assert_eq!(
            state.get_contract_storage(
                &collection,
                &xor_storage_key(&ids[0], &REF_NFT_OWNER_STORAGE_XOR)
            ),
            [0u8; 32]
        );
    }

    #[test]
    fn mint_batch_length_mismatch_trailing_word_faults() {
        let deployer = AccountId([0x81u8; 32]);
        let to = AccountId([0x82u8; 32]);
        let collection = AccountId([0x83u8; 32]);
        let (mut state, code) = collection_fixture(deployer, collection);
        let ids = [tid_word(1), tid_word(2)];
        let hashes = [hash_word(1), hash_word(2)];
        let mut data = encode_mint_batch_calldata(&to, &ids, &hashes);
        data.extend_from_slice(&[0xffu8; 32]);
        let mut it = Interpreter::new(code, crate::vm::GAS_PER_CONTRACT_CALL);
        let err = it.run(deployer, collection, &data, &mut state).unwrap_err();
        assert!(matches!(err, crate::vm::VmError::InvalidJump));
        assert_eq!(
            state.get_contract_storage(
                &collection,
                &xor_storage_key(&ids[0], &REF_NFT_OWNER_STORAGE_XOR)
            ),
            [0u8; 32]
        );
    }

    #[test]
    fn mint_batch_n_max_fits_production_call_gas() {
        assert!(
            crate::vm::GAS_PER_CONTRACT_CALL >= 2_500_000,
            "call budget must cover n={} owner+metadata SSTOREs; got {}",
            MAX_REFERENCE_NFT_MINT_BATCH,
            crate::vm::GAS_PER_CONTRACT_CALL
        );
        let deployer = AccountId([0x91u8; 32]);
        let to = AccountId([0x92u8; 32]);
        let collection = AccountId([0x93u8; 32]);
        let (mut state, code) = collection_fixture(deployer, collection);
        let n = MAX_REFERENCE_NFT_MINT_BATCH as usize;
        let ids: Vec<[u8; 32]> = (1..=n as u8).map(tid_word).collect();
        let hashes: Vec<[u8; 32]> = (1..=n as u8).map(hash_word).collect();
        let data = encode_mint_batch_calldata(&to, &ids, &hashes);
        let mut it = Interpreter::new(code, crate::vm::GAS_PER_CONTRACT_CALL);
        it.run(deployer, collection, &data, &mut state).unwrap();
        // Measured ~2_151_307 with owner+metadata SSTORE for n=50 (2026-10 template v2).
        assert!(
            (2_000_000..2_500_000).contains(&it.gas_used),
            "unexpected mint_batch n={n} gas_used {}",
            it.gas_used
        );
        assert!(
            it.gas_used <= crate::vm::GAS_PER_CONTRACT_CALL,
            "gas_used {} exceeds production call budget",
            it.gas_used
        );
        assert!(
            it.gas_used > 1_000_000,
            "unexpectedly cheap mint_batch n={n}: {}",
            it.gas_used
        );
    }

    #[test]
    fn reference_nft_collection_template_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = reference_nft_collection_template_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("nft"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for reference NFT collection bytecode, got {r:?}"
        );
    }
}
