//! On-chain **NFT collection ↔ fungible token** link registry (enforced edges).
//!
//! Many-to-many, mutable add/remove. Auth: **dual asset claimer** — first caller to
//! [`SELECTOR_CLAIM_ASSET`] for an AccountId becomes its claimer; [`SELECTOR_REGISTER_LINK`] /
//! [`SELECTOR_UNLINK_AT`] require `CALLER` to be claimer of **both** collection and token.
//!
//! See `docs/BOING-LINKED-NFT-TOKEN.md`.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// `claim_asset(asset)` — **64** bytes (selector + AccountId).
pub const SELECTOR_CLAIM_ASSET: u8 = 0xE0;
/// `register_link(collection, token)` — **96** bytes.
pub const SELECTOR_REGISTER_LINK: u8 = 0xE1;
/// `unlink_at(index)` — **64** bytes (tombstone clear).
pub const SELECTOR_UNLINK_AT: u8 = 0xE2;
/// `links_count` — **32** bytes; returns one word (count in low **8** bytes).
pub const SELECTOR_LINKS_COUNT: u8 = 0xE3;
/// `get_link_at(index)` — **64** bytes; returns **64** bytes (collection, token).
pub const SELECTOR_GET_LINK_AT: u8 = 0xE4;
/// `get_asset_claimer(asset)` — **64** bytes; returns claimer AccountId word.
pub const SELECTOR_GET_ASSET_CLAIMER: u8 = 0xE5;
/// `transfer_asset_claimer(asset, new_claimer)` — **96** bytes.
pub const SELECTOR_TRANSFER_ASSET_CLAIMER: u8 = 0xE6;

/// `Log3` topic0 after successful [`SELECTOR_REGISTER_LINK`].
/// topic1 = collection, topic2 = token, data = CALLER.
pub const LINKED_NFT_TOKEN_TOPIC_REGISTER: [u8; 32] =
    *b"BOING_NFT_TOKEN_LINK_REG1\x00\x00\x00\x00\x00\x00\x00";

/// `Log3` topic0 after successful [`SELECTOR_UNLINK_AT`].
/// topic1 = collection, topic2 = token, data = CALLER.
pub const LINKED_NFT_TOKEN_TOPIC_UNLINK: [u8; 32] =
    *b"BOING_NFT_TOKEN_LINK_UNL1\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for [`linked_nft_token_registry_bytecode`].
pub const LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_NFT_TOKEN_LINK_REG_V1\x00\x00\x00\x00\x00";

/// Inclusive upper bound on stored link slots (includes tombstones).
pub const LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS: u64 = 4096;

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

/// Storage key: number of link slots (append index; tombstones remain).
#[must_use]
pub fn linked_nft_token_registry_count_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[16..24].copy_from_slice(b"BOINGLNK");
    k[28..32].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
    k
}

/// Base word for pair keys: `key = BASE + (index * 2 + field)` (field 0 = collection, 1 = token).
#[must_use]
pub fn linked_nft_token_registry_pair_base_word() -> [u8; 32] {
    let mut w = [0u8; 32];
    w[0..12].copy_from_slice(b"BOINGLNKPAIR");
    w
}

/// XOR mask for asset claimer slots: `key = asset ^ MASK`.
pub const LINKED_NFT_TOKEN_CLAIMER_XOR: [u8; 32] =
    *b"BOING_LNK_CLAIMER_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

#[must_use]
pub fn linked_nft_token_claimer_storage_key(asset: &AccountId) -> [u8; 32] {
    xor32(&asset.0, &LINKED_NFT_TOKEN_CLAIMER_XOR)
}

#[must_use]
pub fn linked_nft_token_registry_pair_storage_key(index: u64, field: u8) -> [u8; 32] {
    assert!(field <= 1);
    let mut w = linked_nft_token_registry_pair_base_word();
    let mut addend: u128 = u128::from(index) * 2 + u128::from(field);
    let mut carry: u128 = 0;
    for i in (0..32).rev() {
        let s = u128::from(w[i]) + (addend & 0xff) + carry;
        w[i] = (s & 0xff) as u8;
        carry = s >> 8;
        addend >>= 8;
    }
    w
}

fn append_build_pair_key(code: &mut Vec<u8>, mem_idx: u64, field: u8) {
    push32(code, &word_u64(mem_idx));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(1));
    code.push(Opcode::Shl as u8);
    push32(code, &word_u64(u64::from(field)));
    code.push(Opcode::Add as u8);
    push32(code, &linked_nft_token_registry_pair_base_word());
    code.push(Opcode::Add as u8);
}

fn append_sstore_pair_field(code: &mut Vec<u8>, mem_idx: u64, field: u8, calldata_word_off: u64) {
    push32(code, &word_u64(calldata_word_off));
    code.push(Opcode::MLoad as u8);
    append_build_pair_key(code, mem_idx, field);
    code.push(Opcode::SStore as u8);
}

/// Abort if `SLOAD(calldata[asset_off] ^ CLAIMER_XOR) != CALLER`.
fn append_require_caller_is_claimer(code: &mut Vec<u8>, asset_off: u64, fix_aborts: &mut Vec<usize>) {
    push32(code, &word_u64(asset_off));
    code.push(Opcode::MLoad as u8);
    push32(code, &LINKED_NFT_TOKEN_CLAIMER_XOR);
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

fn append_log3_caller_topics_ct(code: &mut Vec<u8>, topic0: &[u8; 32], mem_log: u64) {
    // data = CALLER at mem_log
    code.push(Opcode::Caller as u8);
    push32(code, &word_u64(mem_log));
    code.push(Opcode::MStore as u8);

    push32(code, topic0);
    push32(code, &word_u64(32));
    code.push(Opcode::MLoad as u8); // collection
    push32(code, &word_u64(64));
    code.push(Opcode::MLoad as u8); // token
    push32(code, &word_u64(32));
    push32(code, &word_u64(mem_log));
    code.push(Opcode::Log3 as u8);
}

#[must_use]
pub fn encode_claim_asset_calldata(asset: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_CLAIM_ASSET).to_vec();
    v.extend_from_slice(&asset.0);
    v
}

#[must_use]
pub fn encode_register_link_calldata(collection: &AccountId, token: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_REGISTER_LINK).to_vec();
    v.extend_from_slice(&collection.0);
    v.extend_from_slice(&token.0);
    v
}

#[must_use]
pub fn encode_unlink_at_calldata(index: u64) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_UNLINK_AT).to_vec();
    v.extend_from_slice(&word_u64(index));
    v
}

#[must_use]
pub fn encode_links_count_calldata() -> Vec<u8> {
    selector_word(SELECTOR_LINKS_COUNT).to_vec()
}

#[must_use]
pub fn encode_get_link_at_calldata(index: u64) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_GET_LINK_AT).to_vec();
    v.extend_from_slice(&word_u64(index));
    v
}

#[must_use]
pub fn encode_get_asset_claimer_calldata(asset: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_GET_ASSET_CLAIMER).to_vec();
    v.extend_from_slice(&asset.0);
    v
}

#[must_use]
pub fn encode_transfer_asset_claimer_calldata(asset: &AccountId, new_claimer: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_TRANSFER_ASSET_CLAIMER).to_vec();
    v.extend_from_slice(&asset.0);
    v.extend_from_slice(&new_claimer.0);
    v
}

/// Canonical link-registry bytecode (v1). CREATE2: [`LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1`].
#[must_use]
pub fn linked_nft_token_registry_bytecode() -> Vec<u8> {
    const MEM_IDX: u64 = 128;
    const MEM_TMP: u64 = 160;
    const MEM_LOG: u64 = 192;
    const MEM_RET: u64 = 256;

    let mut c: Vec<u8> = Vec::new();
    let mut fix_aborts: Vec<usize> = Vec::new();

    // --- dispatch ---
    let mut fix_jumps = [0usize; 7];
    for (i, sel) in [
        SELECTOR_CLAIM_ASSET,
        SELECTOR_REGISTER_LINK,
        SELECTOR_UNLINK_AT,
        SELECTOR_LINKS_COUNT,
        SELECTOR_GET_LINK_AT,
        SELECTOR_GET_ASSET_CLAIMER,
        SELECTOR_TRANSFER_ASSET_CLAIMER,
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
        push32(&mut c, &LINKED_NFT_TOKEN_CLAIMER_XOR);
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

    // ========== register_link ==========
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

        // Note: no on-chain O(1) presence map (no hash opcode; XOR maps collide).
        // Clients should scan `get_link_at` / logs and skip duplicate active edges.

        // cnt → MEM_IDX; abort if !(cnt < MAX)
        push32(&mut c, &linked_nft_token_registry_count_key());
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS));
        c.push(Opcode::Lt as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        append_sstore_pair_field(&mut c, MEM_IDX, 0, 32);
        append_sstore_pair_field(&mut c, MEM_IDX, 1, 64);

        // count + 1
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(1));
        c.push(Opcode::Add as u8);
        push32(&mut c, &linked_nft_token_registry_count_key());
        c.push(Opcode::SStore as u8);

        append_log3_caller_topics_ct(&mut c, &LINKED_NFT_TOKEN_TOPIC_REGISTER, MEM_LOG);
        c.push(Opcode::Stop as u8);
    }

    // ========== unlink_at ==========
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
        push32(&mut c, &linked_nft_token_registry_count_key());
        c.push(Opcode::SLoad as u8);
        c.push(Opcode::Lt as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        // load collection/token into calldata mem slots 32/64 for auth + log helpers
        append_build_pair_key(&mut c, MEM_IDX, 0);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MStore as u8);
        append_build_pair_key(&mut c, MEM_IDX, 1);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(64));
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

        append_require_caller_is_claimer(&mut c, 32, &mut fix_aborts);
        append_require_caller_is_claimer(&mut c, 64, &mut fix_aborts);

        // tombstone pair slots
        push32(&mut c, &word_u64(0));
        append_build_pair_key(&mut c, MEM_IDX, 0);
        c.push(Opcode::SStore as u8);
        push32(&mut c, &word_u64(0));
        append_build_pair_key(&mut c, MEM_IDX, 1);
        c.push(Opcode::SStore as u8);

        append_log3_caller_topics_ct(&mut c, &LINKED_NFT_TOKEN_TOPIC_UNLINK, MEM_LOG);
        c.push(Opcode::Stop as u8);
    }

    // ========== links_count ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[3], off);
        push32(&mut c, &linked_nft_token_registry_count_key());
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::MStore as u8);
        push32(&mut c, &word_u64(32));
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::Return as u8);
        c.push(Opcode::Stop as u8);
    }

    // ========== get_link_at ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[4], off);

        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &linked_nft_token_registry_count_key());
        c.push(Opcode::SLoad as u8);
        c.push(Opcode::Lt as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }

        for (field, off) in [(0u8, 0u64), (1, 32)] {
            append_build_pair_key(&mut c, MEM_IDX, field);
            c.push(Opcode::SLoad as u8);
            push32(&mut c, &word_u64(MEM_RET + off));
            c.push(Opcode::MStore as u8);
        }
        push32(&mut c, &word_u64(64));
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
        push32(&mut c, &LINKED_NFT_TOKEN_CLAIMER_XOR);
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
        push32(&mut c, &LINKED_NFT_TOKEN_CLAIMER_XOR);
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

    fn run(state: &mut StateStore, registry: AccountId, caller: AccountId, calldata: &[u8]) -> Interpreter {
        let mut it = Interpreter::new(linked_nft_token_registry_bytecode(), 8_000_000);
        it.run(caller, registry, calldata, state).unwrap();
        it
    }

    #[test]
    fn claim_register_get_unlink_roundtrip() {
        let registry = AccountId([0xfa; 32]);
        let alice = AccountId([0xa1; 32]);
        let bob = AccountId([0xb0; 32]);
        let coll = AccountId([0x11; 32]);
        let tok = AccountId([0x22; 32]);

        let mut state = StateStore::new();
        state.set_contract_code(registry, linked_nft_token_registry_bytecode());

        // bob cannot register without claims
        let it = run(
            &mut state,
            registry,
            bob,
            &encode_register_link_calldata(&coll, &tok),
        );
        assert!(it.logs.is_empty());

        run(&mut state, registry, alice, &encode_claim_asset_calldata(&coll));
        run(&mut state, registry, alice, &encode_claim_asset_calldata(&tok));

        // bob still cannot register (not claimer)
        let it = run(
            &mut state,
            registry,
            bob,
            &encode_register_link_calldata(&coll, &tok),
        );
        assert!(it.logs.is_empty());

        let it = run(
            &mut state,
            registry,
            alice,
            &encode_register_link_calldata(&coll, &tok),
        );
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], LINKED_NFT_TOKEN_TOPIC_REGISTER);
        assert_eq!(it.logs[0].topics[1], coll.0);
        assert_eq!(it.logs[0].topics[2], tok.0);
        assert_eq!(it.logs[0].data, alice.0.to_vec());

        let it = run(&mut state, registry, alice, &encode_links_count_calldata());
        let mut exp = [0u8; 32];
        exp[31] = 1;
        assert_eq!(it.return_data.as_deref(), Some(&exp[..]));

        let it = run(&mut state, registry, alice, &encode_get_link_at_calldata(0));
        let mut want = Vec::new();
        want.extend_from_slice(&coll.0);
        want.extend_from_slice(&tok.0);
        assert_eq!(it.return_data.as_deref(), Some(want.as_slice()));

        let it = run(&mut state, registry, alice, &encode_get_asset_claimer_calldata(&coll));
        assert_eq!(it.return_data.as_deref(), Some(&alice.0[..]));

        // bob cannot unlink
        let it = run(&mut state, registry, bob, &encode_unlink_at_calldata(0));
        assert!(it.logs.is_empty());

        let it = run(&mut state, registry, alice, &encode_unlink_at_calldata(0));
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], LINKED_NFT_TOKEN_TOPIC_UNLINK);

        let it = run(&mut state, registry, alice, &encode_get_link_at_calldata(0));
        assert_eq!(it.return_data.as_deref(), Some(&[0u8; 64][..]));

        // can re-register after unlink (new slot; tombstones remain)
        let it = run(
            &mut state,
            registry,
            alice,
            &encode_register_link_calldata(&coll, &tok),
        );
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], LINKED_NFT_TOKEN_TOPIC_REGISTER);
        let it = run(&mut state, registry, alice, &encode_links_count_calldata());
        let mut exp2 = [0u8; 32];
        exp2[31] = 2;
        assert_eq!(it.return_data.as_deref(), Some(&exp2[..]));
    }

    #[test]
    fn transfer_claimer_and_many_to_many() {
        let registry = AccountId([0xfa; 32]);
        let alice = AccountId([0xa1; 32]);
        let carol = AccountId([0xc1; 32]);
        let c1 = AccountId([0x11; 32]);
        let c2 = AccountId([0x12; 32]);
        let t1 = AccountId([0x21; 32]);
        let t2 = AccountId([0x22; 32]);

        let mut state = StateStore::new();
        state.set_contract_code(registry, linked_nft_token_registry_bytecode());

        for a in [&c1, &c2, &t1, &t2] {
            run(&mut state, registry, alice, &encode_claim_asset_calldata(a));
        }
        run(
            &mut state,
            registry,
            alice,
            &encode_register_link_calldata(&c1, &t1),
        );
        run(
            &mut state,
            registry,
            alice,
            &encode_register_link_calldata(&c1, &t2),
        );
        run(
            &mut state,
            registry,
            alice,
            &encode_register_link_calldata(&c2, &t1),
        );

        let it = run(&mut state, registry, alice, &encode_links_count_calldata());
        let mut exp = [0u8; 32];
        exp[31] = 3;
        assert_eq!(it.return_data.as_deref(), Some(&exp[..]));

        run(
            &mut state,
            registry,
            alice,
            &encode_transfer_asset_claimer_calldata(&c1, &carol),
        );
        // alice can no longer unlink c1 edges
        let it = run(&mut state, registry, alice, &encode_unlink_at_calldata(0));
        assert!(it.logs.is_empty());
        // carol needs claimer on token too — still alice; transfer t1/t2 or claim... carol only has c1
        // give carol t1 as well via transfer
        run(
            &mut state,
            registry,
            alice,
            &encode_transfer_asset_claimer_calldata(&t1, &carol),
        );
        let it = run(&mut state, registry, carol, &encode_unlink_at_calldata(0));
        assert_eq!(it.logs.len(), 1);
    }

    #[test]
    fn linked_nft_token_registry_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = linked_nft_token_registry_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for link registry bytecode, got {r:?}"
        );
    }
}
