//! On-chain **builder attestation** registry (enforced builder signal).
//!
//! Dual asset-claimer auth (same pattern as [`crate::linked_nft_token_registry`]).
//! `attest` requires claimer of both collection and token; `revoke_at` allows the
//! recorded builder **or** the current dual claimer. No cross-contract companion check.
//!
//! See `docs/BOING-BUILDER-ATTESTATION.md`.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// `claim_asset(asset)` — **64** bytes (selector + AccountId).
pub const SELECTOR_BUILDER_ATTEST_CLAIM_ASSET: u8 = 0xE0;
/// `attest(collection, token, note_hash)` — **128** bytes.
pub const SELECTOR_BUILDER_ATTEST_ATTEST: u8 = 0xE1;
/// `revoke_at(index)` — **64** bytes (tombstone clear).
pub const SELECTOR_BUILDER_ATTEST_REVOKE_AT: u8 = 0xE2;
/// `attestations_count` — **32** bytes; returns one word (count in low **8** bytes).
pub const SELECTOR_BUILDER_ATTEST_COUNT: u8 = 0xE3;
/// `get_attestation_at(index)` — **64** bytes; returns **128** bytes
/// (collection, token, builder, note_hash).
pub const SELECTOR_BUILDER_ATTEST_GET_AT: u8 = 0xE4;
/// `get_asset_claimer(asset)` — **64** bytes; returns claimer AccountId word.
pub const SELECTOR_BUILDER_ATTEST_GET_CLAIMER: u8 = 0xE5;
/// `transfer_asset_claimer(asset, new_claimer)` — **96** bytes.
pub const SELECTOR_BUILDER_ATTEST_TRANSFER_CLAIMER: u8 = 0xE6;

/// `Log3` topic0 after successful attest.
/// topic1 = collection, topic2 = token, data = builder (CALLER).
pub const BUILDER_ATTEST_TOPIC_ATTEST: [u8; 32] =
    *b"BOING_BLDR_ATTEST_REG1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// `Log3` topic0 after successful revoke.
/// topic1 = collection, topic2 = token, data = recorded builder.
pub const BUILDER_ATTEST_TOPIC_REVOKE: [u8; 32] =
    *b"BOING_BLDR_ATTEST_UNL1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for [`builder_attestation_registry_bytecode`].
pub const BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_BUILDER_ATTEST_REG_V1\x00\x00\x00\x00\x00";

/// Inclusive upper bound on stored attestation slots (includes tombstones).
pub const BUILDER_ATTESTATION_REGISTRY_MAX_SLOTS: u64 = 4096;

fn push32(code: &mut Vec<u8>, w: &[u8; 32]) {
    code.push(Opcode::Push32 as u8);
    code.extend_from_slice(w);
}

fn patch_push32_dest(code: &mut [u8], push32_opcode_at: usize, dest: usize) {
    code[push32_opcode_at + 1..push32_opcode_at + 33].copy_from_slice(&word_u64(dest as u64));
}

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

/// Storage key: number of attestation slots (append index; tombstones remain).
#[must_use]
pub fn builder_attestation_registry_count_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[16..24].copy_from_slice(b"BOINGATT");
    k[28..32].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
    k
}

/// Base word for slot keys: `key = BASE + (index * 4 + field)`
/// (field 0 = collection, 1 = token, 2 = builder, 3 = note_hash).
#[must_use]
pub fn builder_attestation_registry_slot_base_word() -> [u8; 32] {
    let mut w = [0u8; 32];
    w[0..12].copy_from_slice(b"BOINGATTSLOT");
    w
}

/// XOR mask for asset claimer slots: `key = asset ^ MASK` (distinct from companions).
pub const BUILDER_ATTEST_CLAIMER_XOR: [u8; 32] =
    *b"BOING_ATT_CLAIMER_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

#[must_use]
pub fn builder_attestation_claimer_storage_key(asset: &AccountId) -> [u8; 32] {
    xor32(&asset.0, &BUILDER_ATTEST_CLAIMER_XOR)
}

#[must_use]
pub fn builder_attestation_registry_slot_storage_key(index: u64, field: u8) -> [u8; 32] {
    assert!(field <= 3);
    let mut w = builder_attestation_registry_slot_base_word();
    let mut addend: u128 = u128::from(index) * 4 + u128::from(field);
    let mut carry: u128 = 0;
    for i in (0..32).rev() {
        let s = u128::from(w[i]) + (addend & 0xff) + carry;
        w[i] = (s & 0xff) as u8;
        carry = s >> 8;
        addend >>= 8;
    }
    w
}

fn append_build_slot_key(code: &mut Vec<u8>, mem_idx: u64, field: u8) {
    push32(code, &word_u64(mem_idx));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(2));
    code.push(Opcode::Shl as u8); // index * 4
    push32(code, &word_u64(u64::from(field)));
    code.push(Opcode::Add as u8);
    push32(code, &builder_attestation_registry_slot_base_word());
    code.push(Opcode::Add as u8);
}

fn append_sstore_slot_field(code: &mut Vec<u8>, mem_idx: u64, field: u8, calldata_word_off: u64) {
    push32(code, &word_u64(calldata_word_off));
    code.push(Opcode::MLoad as u8);
    append_build_slot_key(code, mem_idx, field);
    code.push(Opcode::SStore as u8);
}

fn append_sstore_slot_caller(code: &mut Vec<u8>, mem_idx: u64, field: u8) {
    code.push(Opcode::Caller as u8);
    append_build_slot_key(code, mem_idx, field);
    code.push(Opcode::SStore as u8);
}

/// Abort if `SLOAD(calldata[asset_off] ^ CLAIMER_XOR) != CALLER`.
fn append_require_caller_is_claimer(code: &mut Vec<u8>, asset_off: u64, fix_aborts: &mut Vec<usize>) {
    push32(code, &word_u64(asset_off));
    code.push(Opcode::MLoad as u8);
    push32(code, &BUILDER_ATTEST_CLAIMER_XOR);
    code.push(Opcode::Xor as u8);
    code.push(Opcode::SLoad as u8);
    code.push(Opcode::Caller as u8);
    code.push(Opcode::Eq as u8);
    code.push(Opcode::IsZero as u8);
    let fix = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);
    fix_aborts.push(fix);
}

/// Log3: topic0, topic1=mem[32] collection, topic2=mem[64] token, data=32 bytes at `mem_data`.
fn append_log3_topics_ct_data_mem(code: &mut Vec<u8>, topic0: &[u8; 32], mem_data: u64) {
    push32(code, topic0);
    push32(code, &word_u64(32));
    code.push(Opcode::MLoad as u8); // collection
    push32(code, &word_u64(64));
    code.push(Opcode::MLoad as u8); // token
    push32(code, &word_u64(32));
    push32(code, &word_u64(mem_data));
    code.push(Opcode::Log3 as u8);
}

#[must_use]
pub fn encode_builder_attest_claim_asset_calldata(asset: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_BUILDER_ATTEST_CLAIM_ASSET).to_vec();
    v.extend_from_slice(&asset.0);
    v
}

#[must_use]
pub fn encode_builder_attest_attest_calldata(
    collection: &AccountId,
    token: &AccountId,
    note_hash: &[u8; 32],
) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_BUILDER_ATTEST_ATTEST).to_vec();
    v.extend_from_slice(&collection.0);
    v.extend_from_slice(&token.0);
    v.extend_from_slice(note_hash);
    v
}

#[must_use]
pub fn encode_builder_attest_revoke_at_calldata(index: u64) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_BUILDER_ATTEST_REVOKE_AT).to_vec();
    v.extend_from_slice(&word_u64(index));
    v
}

#[must_use]
pub fn encode_builder_attest_count_calldata() -> Vec<u8> {
    selector_word(SELECTOR_BUILDER_ATTEST_COUNT).to_vec()
}

#[must_use]
pub fn encode_builder_attest_get_at_calldata(index: u64) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_BUILDER_ATTEST_GET_AT).to_vec();
    v.extend_from_slice(&word_u64(index));
    v
}

#[must_use]
pub fn encode_builder_attest_get_claimer_calldata(asset: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_BUILDER_ATTEST_GET_CLAIMER).to_vec();
    v.extend_from_slice(&asset.0);
    v
}

#[must_use]
pub fn encode_builder_attest_transfer_claimer_calldata(
    asset: &AccountId,
    new_claimer: &AccountId,
) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_BUILDER_ATTEST_TRANSFER_CLAIMER).to_vec();
    v.extend_from_slice(&asset.0);
    v.extend_from_slice(&new_claimer.0);
    v
}

/// Canonical builder-attestation registry bytecode (v1).
/// CREATE2: [`BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1`].
#[must_use]
pub fn builder_attestation_registry_bytecode() -> Vec<u8> {
    const MEM_IDX: u64 = 160;
    const MEM_TMP: u64 = 192;
    const MEM_LOG: u64 = 224;
    const MEM_RET: u64 = 256;

    let mut c: Vec<u8> = Vec::new();
    let mut fix_aborts: Vec<usize> = Vec::new();

    // --- dispatch ---
    let mut fix_jumps = [0usize; 7];
    for (i, sel) in [
        SELECTOR_BUILDER_ATTEST_CLAIM_ASSET,
        SELECTOR_BUILDER_ATTEST_ATTEST,
        SELECTOR_BUILDER_ATTEST_REVOKE_AT,
        SELECTOR_BUILDER_ATTEST_COUNT,
        SELECTOR_BUILDER_ATTEST_GET_AT,
        SELECTOR_BUILDER_ATTEST_GET_CLAIMER,
        SELECTOR_BUILDER_ATTEST_TRANSFER_CLAIMER,
    ]
    .into_iter()
    .enumerate()
    {
        push32(&mut c, &word_u64(0));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &selector_word(sel));
        c.push(Opcode::Eq as u8);
        fix_jumps[i] = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
    }
    let off_abort = c.len();
    c.push(Opcode::Stop as u8);

    // ========== claim_asset ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[0], off);

        // abort if asset == 0
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        // key = asset ^ CLAIMER_XOR → MEM_IDX
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &BUILDER_ATTEST_CLAIMER_XOR);
        c.push(Opcode::Xor as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        // existing → MEM_TMP
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MStore as u8);

        // if existing == 0 → set; else require == CALLER
        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        let fix_set = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);

        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::Caller as u8);
        c.push(Opcode::Eq as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }
        c.push(Opcode::Stop as u8);

        let off_set = c.len();
        patch_push32_dest(&mut c, fix_set, off_set);
        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SStore as u8);
        c.push(Opcode::Stop as u8);
    }

    // ========== attest ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[1], off);

        // abort if collection == 0 or token == 0
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }
        push32(&mut c, &word_u64(64));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        append_require_caller_is_claimer(&mut c, 32, &mut fix_aborts);
        append_require_caller_is_claimer(&mut c, 64, &mut fix_aborts);

        // cnt → MEM_IDX; abort if !(cnt < MAX)
        push32(&mut c, &builder_attestation_registry_count_key());
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(BUILDER_ATTESTATION_REGISTRY_MAX_SLOTS));
        c.push(Opcode::Lt as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        // store collection, token, builder=CALLER, note_hash
        append_sstore_slot_field(&mut c, MEM_IDX, 0, 32);
        append_sstore_slot_field(&mut c, MEM_IDX, 1, 64);
        append_sstore_slot_caller(&mut c, MEM_IDX, 2);
        append_sstore_slot_field(&mut c, MEM_IDX, 3, 96);

        // count + 1
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(1));
        c.push(Opcode::Add as u8);
        push32(&mut c, &builder_attestation_registry_count_key());
        c.push(Opcode::SStore as u8);

        // log data = CALLER (builder)
        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_LOG));
        c.push(Opcode::MStore as u8);
        append_log3_topics_ct_data_mem(&mut c, &BUILDER_ATTEST_TOPIC_ATTEST, MEM_LOG);
        c.push(Opcode::Stop as u8);
    }

    // ========== revoke_at ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[2], off);

        // ix → MEM_IDX
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        // require ix < count
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &builder_attestation_registry_count_key());
        c.push(Opcode::SLoad as u8);
        c.push(Opcode::Lt as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        // load collection / token / builder into 32 / 64 / 96
        append_build_slot_key(&mut c, MEM_IDX, 0);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MStore as u8);
        append_build_slot_key(&mut c, MEM_IDX, 1);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(64));
        c.push(Opcode::MStore as u8);
        append_build_slot_key(&mut c, MEM_IDX, 2);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(96));
        c.push(Opcode::MStore as u8);

        // abort if already tombstone (collection == 0)
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        // Auth: CALLER == builder → ok; else dual claimer
        push32(&mut c, &word_u64(96));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::Caller as u8);
        c.push(Opcode::Eq as u8);
        let fix_auth_ok = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);

        append_require_caller_is_claimer(&mut c, 32, &mut fix_aborts);
        append_require_caller_is_claimer(&mut c, 64, &mut fix_aborts);

        let off_auth_ok = c.len();
        patch_push32_dest(&mut c, fix_auth_ok, off_auth_ok);

        // tombstone all four fields
        for field in 0u8..4 {
            push32(&mut c, &word_u64(0));
            append_build_slot_key(&mut c, MEM_IDX, field);
            c.push(Opcode::SStore as u8);
        }

        // log data = recorded builder (mem 96)
        push32(&mut c, &word_u64(96));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_LOG));
        c.push(Opcode::MStore as u8);
        append_log3_topics_ct_data_mem(&mut c, &BUILDER_ATTEST_TOPIC_REVOKE, MEM_LOG);
        c.push(Opcode::Stop as u8);
    }

    // ========== attestations_count ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[3], off);
        push32(&mut c, &builder_attestation_registry_count_key());
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::MStore as u8);
        push32(&mut c, &word_u64(32));
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::Return as u8);
        c.push(Opcode::Stop as u8);
    }

    // ========== get_attestation_at ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[4], off);

        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &builder_attestation_registry_count_key());
        c.push(Opcode::SLoad as u8);
        c.push(Opcode::Lt as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        for (field, off) in [(0u8, 0u64), (1, 32), (2, 64), (3, 96)] {
            append_build_slot_key(&mut c, MEM_IDX, field);
            c.push(Opcode::SLoad as u8);
            push32(&mut c, &word_u64(MEM_RET + off));
            c.push(Opcode::MStore as u8);
        }
        push32(&mut c, &word_u64(128));
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::Return as u8);
        c.push(Opcode::Stop as u8);
    }

    // ========== get_asset_claimer ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[5], off);
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &BUILDER_ATTEST_CLAIMER_XOR);
        c.push(Opcode::Xor as u8);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::MStore as u8);
        push32(&mut c, &word_u64(32));
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::Return as u8);
        c.push(Opcode::Stop as u8);
    }

    // ========== transfer_asset_claimer ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[6], off);

        // new_claimer != 0
        push32(&mut c, &word_u64(64));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }
        append_require_caller_is_claimer(&mut c, 32, &mut fix_aborts);

        // key → MEM_IDX; store new
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &BUILDER_ATTEST_CLAIMER_XOR);
        c.push(Opcode::Xor as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        push32(&mut c, &word_u64(64));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SStore as u8);
        c.push(Opcode::Stop as u8);
    }

    for fix in fix_aborts {
        patch_push32_dest(&mut c, fix, off_abort);
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::AccountId;
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;

    fn aid(byte: u8) -> AccountId {
        AccountId([byte; 32])
    }

    fn run(state: &mut StateStore, registry: AccountId, caller: AccountId, calldata: &[u8]) -> Interpreter {
        let mut it = Interpreter::new(builder_attestation_registry_bytecode(), 8_000_000);
        it.run(caller, registry, calldata, state).unwrap();
        it
    }

    #[test]
    fn create2_salt_label() {
        assert_eq!(
            &BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1[..27],
            b"BOING_BUILDER_ATTEST_REG_V1"
        );
    }

    #[test]
    fn encode_lengths_and_selectors() {
        let a = aid(0x11);
        let b = aid(0x22);
        let note = [0x33u8; 32];

        let claim = encode_builder_attest_claim_asset_calldata(&a);
        assert_eq!(claim.len(), 64);
        assert_eq!(claim[31], SELECTOR_BUILDER_ATTEST_CLAIM_ASSET);

        let attest = encode_builder_attest_attest_calldata(&a, &b, &note);
        assert_eq!(attest.len(), 128);
        assert_eq!(attest[31], SELECTOR_BUILDER_ATTEST_ATTEST);

        let revoke = encode_builder_attest_revoke_at_calldata(7);
        assert_eq!(revoke.len(), 64);
        assert_eq!(revoke[31], SELECTOR_BUILDER_ATTEST_REVOKE_AT);

        assert_eq!(encode_builder_attest_count_calldata().len(), 32);
        assert_eq!(encode_builder_attest_get_at_calldata(0).len(), 64);
        assert_eq!(encode_builder_attest_get_claimer_calldata(&a).len(), 64);
        assert_eq!(
            encode_builder_attest_transfer_claimer_calldata(&a, &b).len(),
            96
        );
    }

    #[test]
    fn slot_keys_stride_four_fields() {
        let k0 = builder_attestation_registry_slot_storage_key(0, 0);
        let k1 = builder_attestation_registry_slot_storage_key(0, 1);
        let k4 = builder_attestation_registry_slot_storage_key(1, 0);
        assert_ne!(k0, k1);
        assert_ne!(k0, k4);
        assert_eq!(k4[31].wrapping_sub(k0[31]), 4);
    }

    #[test]
    fn claimer_xor_distinct_from_zero() {
        let a = aid(0xab);
        let key = builder_attestation_claimer_storage_key(&a);
        assert_ne!(key, a.0);
        assert_ne!(key, [0u8; 32]);
    }

    #[test]
    fn claim_attest_get_revoke_roundtrip() {
        let registry = aid(0xfa);
        let alice = aid(0xa1);
        let bob = aid(0xb0);
        let coll = aid(0x11);
        let tok = aid(0x22);
        let note = [0x33u8; 32];

        let mut state = StateStore::new();
        state.set_contract_code(registry, builder_attestation_registry_bytecode());

        // bob cannot attest without claims
        let it = run(
            &mut state,
            registry,
            bob,
            &encode_builder_attest_attest_calldata(&coll, &tok, &note),
        );
        assert!(it.logs.is_empty());

        run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_claim_asset_calldata(&coll),
        );
        run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_claim_asset_calldata(&tok),
        );

        let it = run(
            &mut state,
            registry,
            bob,
            &encode_builder_attest_attest_calldata(&coll, &tok, &note),
        );
        assert!(it.logs.is_empty());

        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_attest_calldata(&coll, &tok, &note),
        );
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], BUILDER_ATTEST_TOPIC_ATTEST);
        assert_eq!(it.logs[0].topics[1], coll.0);
        assert_eq!(it.logs[0].topics[2], tok.0);
        assert_eq!(it.logs[0].data, alice.0.to_vec());

        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_count_calldata(),
        );
        let mut exp = [0u8; 32];
        exp[31] = 1;
        assert_eq!(it.return_data.as_deref(), Some(&exp[..]));

        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_get_at_calldata(0),
        );
        let mut want = Vec::new();
        want.extend_from_slice(&coll.0);
        want.extend_from_slice(&tok.0);
        want.extend_from_slice(&alice.0);
        want.extend_from_slice(&note);
        assert_eq!(it.return_data.as_deref(), Some(want.as_slice()));

        // bob cannot revoke (not builder, not claimer)
        let it = run(
            &mut state,
            registry,
            bob,
            &encode_builder_attest_revoke_at_calldata(0),
        );
        assert!(it.logs.is_empty());

        // builder can revoke
        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_revoke_at_calldata(0),
        );
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], BUILDER_ATTEST_TOPIC_REVOKE);
        assert_eq!(it.logs[0].data, alice.0.to_vec());

        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_get_at_calldata(0),
        );
        assert_eq!(it.return_data.as_deref(), Some(&[0u8; 128][..]));

        // re-attest appends new slot
        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_attest_calldata(&coll, &tok, &note),
        );
        assert_eq!(it.logs.len(), 1);
        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_count_calldata(),
        );
        let mut exp2 = [0u8; 32];
        exp2[31] = 2;
        assert_eq!(it.return_data.as_deref(), Some(&exp2[..]));
    }

    #[test]
    fn dual_claimer_can_revoke_after_transfer_and_duplicates_allowed() {
        let registry = aid(0xfa);
        let alice = aid(0xa1);
        let carol = aid(0xc1);
        let coll = aid(0x11);
        let tok = aid(0x22);
        let note = [0u8; 32];

        let mut state = StateStore::new();
        state.set_contract_code(registry, builder_attestation_registry_bytecode());

        for a in [&coll, &tok] {
            run(
                &mut state,
                registry,
                alice,
                &encode_builder_attest_claim_asset_calldata(a),
            );
        }

        // two attestations (duplicates allowed)
        run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_attest_calldata(&coll, &tok, &note),
        );
        run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_attest_calldata(&coll, &tok, &[0x44; 32]),
        );
        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_count_calldata(),
        );
        let mut exp = [0u8; 32];
        exp[31] = 2;
        assert_eq!(it.return_data.as_deref(), Some(&exp[..]));

        // transfer claimers to carol; alice (builder) can still revoke slot 0
        run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_transfer_claimer_calldata(&coll, &carol),
        );
        run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_transfer_claimer_calldata(&tok, &carol),
        );
        let it = run(
            &mut state,
            registry,
            alice,
            &encode_builder_attest_revoke_at_calldata(0),
        );
        assert_eq!(it.logs.len(), 1);

        // carol (dual claimer, not builder) can revoke slot 1
        let it = run(
            &mut state,
            registry,
            carol,
            &encode_builder_attest_revoke_at_calldata(1),
        );
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], BUILDER_ATTEST_TOPIC_REVOKE);
        assert_eq!(it.logs[0].data, alice.0.to_vec()); // recorded builder
    }

    #[test]
    fn builder_attestation_registry_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = builder_attestation_registry_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for builder attestation registry bytecode, got {r:?}"
        );
    }
}
