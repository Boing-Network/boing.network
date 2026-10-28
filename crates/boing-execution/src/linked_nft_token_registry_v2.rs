//! **Linked NFT ↔ token registry v2**: only the asset's creator can become its claimer.
//!
//! v1 ([`crate::linked_nft_token_registry`]) lets the **first caller** of `claim_asset(asset)` become the
//! claimer of any AccountId, including a collection or token that someone else deployed (or is about to
//! deploy at a predictable address). v2 keeps the v1 storage layout, link selectors and events, and
//! changes only who may claim:
//!
//! - [`SELECTOR_CLAIM_SELF_V2`] `claim_self(claimer)`: `CALLER` is the asset itself. A contract can only
//!   call this from its own code, so an asset's init code (`0xFD` deploy) can register its creator
//!   (`CALLER` of the init frame = deploy tx sender) in the same transaction that creates it. There is
//!   no window in which anybody else can claim.
//! - [`crate::linked_nft_token_registry::SELECTOR_CLAIM_ASSET`] `claim_asset(asset)`: allowed only when the
//!   registry holds a **deployer record** for `asset` and `CALLER` is that deployer. Deployer records
//!   are written once by the deploy-time init of the registry itself (seed: assets that existed when v2
//!   was deployed, with the sender of their deploy tx), see [`linked_nft_token_registry_v2_deploy_bytecode`].
//! - Write paths that fail (not claimer, already claimed, bad index, unknown selector) **fault**
//!   (`InvalidJump`) so the transaction fails visibly. Read paths keep the v1 behaviour (empty return).
//!
//! `register_link` / `unlink_at` / `transfer_asset_claimer` / reads are unchanged from v1.
//! See `docs/BOING-LINKED-NFT-TOKEN.md`.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::linked_nft_token_registry::{
    linked_nft_token_claimer_storage_key, linked_nft_token_registry_count_key,
    linked_nft_token_registry_pair_base_word, linked_nft_token_registry_pair_storage_key,
    LINKED_NFT_TOKEN_CLAIMER_XOR, LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS,
    LINKED_NFT_TOKEN_TOPIC_REGISTER, LINKED_NFT_TOKEN_TOPIC_UNLINK, SELECTOR_CLAIM_ASSET,
    SELECTOR_GET_ASSET_CLAIMER, SELECTOR_GET_LINK_AT, SELECTOR_LINKS_COUNT, SELECTOR_REGISTER_LINK,
    SELECTOR_TRANSFER_ASSET_CLAIMER, SELECTOR_UNLINK_AT,
};
use crate::reference_token::selector_word;

/// `claim_self(claimer)` — **64** bytes. `CALLER` (the asset) gets `claimer` if it has none yet.
pub const SELECTOR_CLAIM_SELF_V2: u8 = 0xE7;
/// `get_asset_deployer(asset)` — **64** bytes; returns the seeded deployer record (zero if none).
pub const SELECTOR_GET_ASSET_DEPLOYER_V2: u8 = 0xE8;

/// `Log2` topic0 after a successful claim (either path). topic1 = asset, data = claimer.
pub const LINKED_NFT_TOKEN_TOPIC_CLAIM_V2: [u8; 32] =
    *b"BOING_NFT_TOKEN_LINK_CLM2\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for the v2 registry.
pub const LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2: [u8; 32] =
    *b"BOING_NFT_TOKEN_LINK_REG_V2\x00\x00\x00\x00\x00";

/// XOR mask for deployer-record slots: `key = asset ^ MASK`.
pub const LINKED_NFT_TOKEN_DEPLOYER_XOR_V2: [u8; 32] =
    *b"BOING_LNK_DEPLOYER_V2\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Marker byte that makes a deploy run init code (`RETURN` data becomes the stored code).
const DEPLOY_INIT_MARKER: u8 = 0xFD;

#[must_use]
pub fn linked_nft_token_deployer_storage_key_v2(asset: &AccountId) -> [u8; 32] {
    let mut o = [0u8; 32];
    for (i, b) in o.iter_mut().enumerate() {
        *b = asset.0[i] ^ LINKED_NFT_TOKEN_DEPLOYER_XOR_V2[i];
    }
    o
}

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

/// `PUSH32 <dest> JUMPI` with the destination patched later; records the fix-up position.
fn jumpi_to(code: &mut Vec<u8>, fixes: &mut Vec<usize>) {
    fixes.push(code.len());
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);
}

#[must_use]
pub fn encode_claim_self_calldata_v2(claimer: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_CLAIM_SELF_V2).to_vec();
    v.extend_from_slice(&claimer.0);
    v
}

#[must_use]
pub fn encode_get_asset_deployer_calldata_v2(asset: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_GET_ASSET_DEPLOYER_V2).to_vec();
    v.extend_from_slice(&asset.0);
    v
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

/// Fault unless `SLOAD(mem[asset_off] ^ CLAIMER_XOR) == CALLER`.
fn append_require_caller_is_claimer(code: &mut Vec<u8>, asset_off: u64, faults: &mut Vec<usize>) {
    push32(code, &word_u64(asset_off));
    code.push(Opcode::MLoad as u8);
    push32(code, &LINKED_NFT_TOKEN_CLAIMER_XOR);
    code.push(Opcode::Xor as u8);
    code.push(Opcode::SLoad as u8);
    code.push(Opcode::Caller as u8);
    code.push(Opcode::Eq as u8);
    code.push(Opcode::IsZero as u8);
    jumpi_to(code, faults);
}

fn append_log3_caller_topics_ct(code: &mut Vec<u8>, topic0: &[u8; 32], mem_log: u64) {
    code.push(Opcode::Caller as u8);
    push32(code, &word_u64(mem_log));
    code.push(Opcode::MStore as u8);

    push32(code, topic0);
    push32(code, &word_u64(32));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(64));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(32));
    push32(code, &word_u64(mem_log));
    code.push(Opcode::Log3 as u8);
}

/// `Log2(CLAIM, asset)` with data = claimer; expects asset at `mem_asset`, claimer at `mem_claimer`.
fn append_log_claim(code: &mut Vec<u8>, mem_asset: u64, mem_claimer: u64) {
    push32(code, &LINKED_NFT_TOKEN_TOPIC_CLAIM_V2);
    push32(code, &word_u64(mem_asset));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(32));
    push32(code, &word_u64(mem_claimer));
    code.push(Opcode::Log2 as u8);
}

/// Runtime bytecode of the v2 registry (what is stored on chain).
#[must_use]
pub fn linked_nft_token_registry_v2_runtime_bytecode() -> Vec<u8> {
    const MEM_IDX: u64 = 128;
    const MEM_TMP: u64 = 160;
    const MEM_LOG: u64 = 192;
    const MEM_ASSET: u64 = 224;
    const MEM_RET: u64 = 256;

    let mut c: Vec<u8> = Vec::new();
    let mut stops: Vec<usize> = Vec::new();
    let mut faults: Vec<usize> = Vec::new();

    let selectors = [
        SELECTOR_CLAIM_ASSET,
        SELECTOR_REGISTER_LINK,
        SELECTOR_UNLINK_AT,
        SELECTOR_LINKS_COUNT,
        SELECTOR_GET_LINK_AT,
        SELECTOR_GET_ASSET_CLAIMER,
        SELECTOR_TRANSFER_ASSET_CLAIMER,
        SELECTOR_CLAIM_SELF_V2,
        SELECTOR_GET_ASSET_DEPLOYER_V2,
    ];
    let mut fix_jumps = [0usize; 9];
    for (i, sel) in selectors.into_iter().enumerate() {
        push32(&mut c, &word_u64(0));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &selector_word(sel));
        c.push(Opcode::Eq as u8);
        fix_jumps[i] = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
    }
    // Unknown selector → fault.
    let off_fault = c.len();
    push32(&mut c, &[0xffu8; 32]);
    c.push(Opcode::Jump as u8);
    let off_stop = c.len();
    c.push(Opcode::Stop as u8);

    // ========== claim_asset(asset): only the recorded deployer ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[0], off);

        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        jumpi_to(&mut c, &mut faults);

        // claimer key → MEM_IDX; existing → MEM_TMP
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &LINKED_NFT_TOKEN_CLAIMER_XOR);
        c.push(Opcode::Xor as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MStore as u8);

        // existing == 0 → check deployer record
        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        let fix_unclaimed = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);

        // already claimed: by CALLER → no-op; else fault
        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::Caller as u8);
        c.push(Opcode::Eq as u8);
        c.push(Opcode::IsZero as u8);
        jumpi_to(&mut c, &mut faults);
        c.push(Opcode::Stop as u8);

        let off_unclaimed = c.len();
        patch_push32_dest(&mut c, fix_unclaimed, off_unclaimed);
        // require SLOAD(asset ^ DEPLOYER_XOR) == CALLER
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &LINKED_NFT_TOKEN_DEPLOYER_XOR_V2);
        c.push(Opcode::Xor as u8);
        c.push(Opcode::SLoad as u8);
        c.push(Opcode::Caller as u8);
        c.push(Opcode::Eq as u8);
        c.push(Opcode::IsZero as u8);
        jumpi_to(&mut c, &mut faults);

        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SStore as u8);

        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_LOG));
        c.push(Opcode::MStore as u8);
        append_log_claim(&mut c, 32, MEM_LOG);
        c.push(Opcode::Stop as u8);
    }

    // ========== register_link ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[1], off);

        for w in [32u64, 64] {
            push32(&mut c, &word_u64(w));
            c.push(Opcode::MLoad as u8);
            c.push(Opcode::IsZero as u8);
            jumpi_to(&mut c, &mut faults);
        }
        append_require_caller_is_claimer(&mut c, 32, &mut faults);
        append_require_caller_is_claimer(&mut c, 64, &mut faults);

        push32(&mut c, &linked_nft_token_registry_count_key());
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS));
        c.push(Opcode::Lt as u8);
        c.push(Opcode::IsZero as u8);
        jumpi_to(&mut c, &mut faults);

        append_sstore_pair_field(&mut c, MEM_IDX, 0, 32);
        append_sstore_pair_field(&mut c, MEM_IDX, 1, 64);

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
        jumpi_to(&mut c, &mut faults);

        append_build_pair_key(&mut c, MEM_IDX, 0);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MStore as u8);
        append_build_pair_key(&mut c, MEM_IDX, 1);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(64));
        c.push(Opcode::MStore as u8);

        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        jumpi_to(&mut c, &mut faults);

        append_require_caller_is_claimer(&mut c, 32, &mut faults);
        append_require_caller_is_claimer(&mut c, 64, &mut faults);

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

    // ========== get_link_at (out of range → empty, like v1) ==========
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
        jumpi_to(&mut c, &mut stops);

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

    // ========== get_asset_claimer / get_asset_deployer ==========
    for (jump_ix, mask) in [
        (5usize, LINKED_NFT_TOKEN_CLAIMER_XOR),
        (8, LINKED_NFT_TOKEN_DEPLOYER_XOR_V2),
    ] {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[jump_ix], off);
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &mask);
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

        push32(&mut c, &word_u64(64));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        jumpi_to(&mut c, &mut faults);
        append_require_caller_is_claimer(&mut c, 32, &mut faults);

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

    // ========== claim_self(claimer): CALLER is the asset ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[7], off);

        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        jumpi_to(&mut c, &mut faults);

        // key = CALLER ^ CLAIMER_XOR → MEM_IDX; asset → MEM_ASSET
        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_ASSET));
        c.push(Opcode::MStore as u8);
        c.push(Opcode::Caller as u8);
        push32(&mut c, &LINKED_NFT_TOKEN_CLAIMER_XOR);
        c.push(Opcode::Xor as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MStore as u8);

        // fault if already claimed
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SLoad as u8);
        c.push(Opcode::IsZero as u8);
        c.push(Opcode::IsZero as u8);
        jumpi_to(&mut c, &mut faults);

        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_IDX));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SStore as u8);

        append_log_claim(&mut c, MEM_ASSET, 32);
        c.push(Opcode::Stop as u8);
    }

    for fix in stops {
        patch_push32_dest(&mut c, fix, off_stop);
    }
    for fix in faults {
        patch_push32_dest(&mut c, fix, off_fault);
    }
    c
}

/// State copied into a v2 registry by its deploy-time init (written once; there is no admin).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkedNftTokenRegistryV2Seed {
    /// Existing claims `(asset, claimer)` (e.g. copied from v1).
    pub claims: Vec<(AccountId, AccountId)>,
    /// Existing link slots in order `(collection, token)`; `(0, 0)` keeps a tombstone.
    pub links: Vec<(AccountId, AccountId)>,
    /// Deployer records `(asset, deployer)`: lets `deployer` call `claim_asset(asset)` later.
    pub deployers: Vec<(AccountId, AccountId)>,
}

fn append_sstore_const(code: &mut Vec<u8>, key: &[u8; 32], value: &[u8; 32]) {
    push32(code, value);
    push32(code, key);
    code.push(Opcode::SStore as u8);
}

/// `0xFD` deploy bytecode: init writes `seed`, then returns [`linked_nft_token_registry_v2_runtime_bytecode`].
#[must_use]
pub fn linked_nft_token_registry_v2_deploy_bytecode(
    seed: &LinkedNftTokenRegistryV2Seed,
) -> Vec<u8> {
    let runtime = linked_nft_token_registry_v2_runtime_bytecode();
    let mut init: Vec<u8> = Vec::new();
    for (asset, claimer) in &seed.claims {
        append_sstore_const(
            &mut init,
            &linked_nft_token_claimer_storage_key(asset),
            &claimer.0,
        );
    }
    for (asset, deployer) in &seed.deployers {
        append_sstore_const(
            &mut init,
            &linked_nft_token_deployer_storage_key_v2(asset),
            &deployer.0,
        );
    }
    for (i, (coll, tok)) in seed.links.iter().enumerate() {
        append_sstore_const(
            &mut init,
            &linked_nft_token_registry_pair_storage_key(i as u64, 0),
            &coll.0,
        );
        append_sstore_const(
            &mut init,
            &linked_nft_token_registry_pair_storage_key(i as u64, 1),
            &tok.0,
        );
    }
    if !seed.links.is_empty() {
        append_sstore_const(
            &mut init,
            &linked_nft_token_registry_count_key(),
            &word_u64(seed.links.len() as u64),
        );
    }
    for (i, chunk) in runtime.chunks(32).enumerate() {
        let mut w = [0u8; 32];
        w[..chunk.len()].copy_from_slice(chunk);
        push32(&mut init, &w);
        push32(&mut init, &word_u64((i * 32) as u64));
        init.push(Opcode::MStore as u8);
    }
    push32(&mut init, &word_u64(runtime.len() as u64));
    push32(&mut init, &word_u64(0));
    init.push(Opcode::Return as u8);
    init.push(Opcode::Stop as u8);

    let mut out = Vec::with_capacity(init.len() + 1);
    out.push(DEPLOY_INIT_MARKER);
    out.extend_from_slice(&init);
    out
}

/// Init-code snippet for an **asset** deploy: `claim_self(CALLER)` on `registry`, so the deploy tx sender
/// becomes the asset's claimer in the same transaction. Uses memory `[0, 64)` and leaves one word (the
/// CALL success flag) on the stack. Append it to an asset's init before it writes its runtime to memory.
#[must_use]
pub fn linked_nft_token_registry_v2_claim_self_init_snippet(registry: &AccountId) -> Vec<u8> {
    let mut c = Vec::new();
    push32(&mut c, &selector_word(SELECTOR_CLAIM_SELF_V2));
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MStore as u8);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &registry.0); // target
    push32(&mut c, &word_u64(0)); // args offset
    push32(&mut c, &word_u64(64)); // args size
    push32(&mut c, &word_u64(0)); // ret offset
    push32(&mut c, &word_u64(0)); // ret size
    c.push(Opcode::Call as u8);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::Interpreter;
    use crate::linked_nft_token_registry::{
        encode_claim_asset_calldata, encode_get_asset_claimer_calldata,
        encode_get_link_at_calldata, encode_links_count_calldata, encode_register_link_calldata,
        encode_transfer_asset_claimer_calldata, encode_unlink_at_calldata,
    };
    use crate::vm::{Vm, VmError};
    use boing_primitives::{
        create2_contract_address, AccessList, Account, AccountState, Transaction,
        TransactionPayload,
    };
    use boing_state::StateStore;

    fn run(
        state: &mut StateStore,
        registry: AccountId,
        caller: AccountId,
        calldata: &[u8],
    ) -> Result<Interpreter, VmError> {
        let code = state.get_contract_code(&registry).cloned().unwrap();
        let mut it = Interpreter::new(code, 8_000_000);
        it.run(caller, registry, calldata, state)?;
        Ok(it)
    }

    fn word(a: &AccountId) -> Vec<u8> {
        a.0.to_vec()
    }

    fn fund(state: &mut StateStore, id: AccountId) {
        state.insert(Account {
            id,
            state: AccountState {
                balance: u128::MAX / 4,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
    }

    fn deploy_tx(sender: AccountId, nonce: u64, bytecode: Vec<u8>, salt: [u8; 32]) -> Transaction {
        Transaction {
            nonce,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode,
                create2_salt: Some(salt),
            },
            access_list: AccessList::default(),
        }
    }

    /// Deploys a seeded v2 registry through the real VM deploy path and returns its address.
    fn deploy_registry(
        state: &mut StateStore,
        deployer: AccountId,
        seed: &LinkedNftTokenRegistryV2Seed,
    ) -> AccountId {
        let code = linked_nft_token_registry_v2_deploy_bytecode(seed);
        let addr =
            create2_contract_address(&deployer, &LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2, &code);
        let nonce = state.get(&deployer).map(|a| a.nonce).unwrap_or(0);
        Vm::new()
            .execute(
                &deploy_tx(
                    deployer,
                    nonce,
                    code,
                    LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2,
                ),
                state,
            )
            .unwrap();
        assert_eq!(
            state.get_contract_code(&addr).cloned().unwrap(),
            linked_nft_token_registry_v2_runtime_bytecode()
        );
        addr
    }

    /// Asset deploy whose init claims itself in `registry` for the tx sender, then returns `runtime`.
    fn asset_deploy_bytecode(registry: &AccountId, runtime: &[u8]) -> Vec<u8> {
        let mut init = linked_nft_token_registry_v2_claim_self_init_snippet(registry);
        for (i, chunk) in runtime.chunks(32).enumerate() {
            let mut w = [0u8; 32];
            w[..chunk.len()].copy_from_slice(chunk);
            push32(&mut init, &w);
            push32(&mut init, &word_u64((i * 32) as u64));
            init.push(Opcode::MStore as u8);
        }
        push32(&mut init, &word_u64(runtime.len() as u64));
        push32(&mut init, &word_u64(0));
        init.push(Opcode::Return as u8);
        let mut out = vec![DEPLOY_INIT_MARKER];
        out.extend_from_slice(&init);
        out
    }

    #[test]
    fn asset_claims_itself_at_deploy_and_nobody_else_can_claim() {
        let reg_deployer = AccountId([0x0d; 32]);
        let alice = AccountId([0xa1; 32]);
        let mallory = AccountId([0x66; 32]);
        let mut state = StateStore::new();
        for a in [reg_deployer, alice, mallory] {
            fund(&mut state, a);
        }
        let registry = deploy_registry(
            &mut state,
            reg_deployer,
            &LinkedNftTokenRegistryV2Seed::default(),
        );

        let runtime = crate::reference_token_v2::reference_fungible_template_v2_bytecode();
        let coll_runtime = crate::reference_nft::reference_nft_collection_template_bytecode();
        let tok_code = asset_deploy_bytecode(&registry, &runtime);
        let coll_code = asset_deploy_bytecode(&registry, &coll_runtime);
        let salt = [0x51; 32];
        let tok = create2_contract_address(&alice, &salt, &tok_code);
        let coll = create2_contract_address(&alice, &salt, &coll_code);

        // mallory front-runs with claim_asset on the predicted addresses: faults (no deployer record).
        for a in [tok, coll] {
            assert!(run(
                &mut state,
                registry,
                mallory,
                &encode_claim_asset_calldata(&a)
            )
            .is_err());
        }
        // mallory cannot claim_self for someone else's address either: CALLER is mallory.
        run(
            &mut state,
            registry,
            mallory,
            &encode_claim_self_calldata_v2(&mallory),
        )
        .unwrap();
        let r = run(
            &mut state,
            registry,
            mallory,
            &encode_get_asset_claimer_calldata(&tok),
        )
        .unwrap();
        assert_eq!(r.return_data.unwrap(), vec![0u8; 32]);

        let vm = Vm::new();
        vm.execute(&deploy_tx(alice, 0, tok_code, salt), &mut state)
            .unwrap();
        vm.execute(&deploy_tx(alice, 1, coll_code, salt), &mut state)
            .unwrap();
        assert_eq!(state.get_contract_code(&tok).cloned().unwrap(), runtime);
        assert_eq!(
            state.get_contract_code(&coll).cloned().unwrap(),
            coll_runtime
        );
        for a in [tok, coll] {
            let r = run(
                &mut state,
                registry,
                mallory,
                &encode_get_asset_claimer_calldata(&a),
            )
            .unwrap();
            assert_eq!(r.return_data.unwrap(), word(&alice));
            // a second claim by anyone else faults; by alice it is a no-op
            assert!(run(
                &mut state,
                registry,
                mallory,
                &encode_claim_asset_calldata(&a)
            )
            .is_err());
            run(
                &mut state,
                registry,
                alice,
                &encode_claim_asset_calldata(&a),
            )
            .unwrap();
        }
        // mallory cannot link alice's assets; alice can.
        assert!(run(
            &mut state,
            registry,
            mallory,
            &encode_register_link_calldata(&coll, &tok)
        )
        .is_err());
        let it = run(
            &mut state,
            registry,
            alice,
            &encode_register_link_calldata(&coll, &tok),
        )
        .unwrap();
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], LINKED_NFT_TOKEN_TOPIC_REGISTER);
        let it = run(&mut state, registry, alice, &encode_get_link_at_calldata(0)).unwrap();
        assert_eq!(it.return_data.unwrap(), [word(&coll), word(&tok)].concat());
        // unlink by mallory faults, by alice works
        assert!(run(&mut state, registry, mallory, &encode_unlink_at_calldata(0)).is_err());
        run(&mut state, registry, alice, &encode_unlink_at_calldata(0)).unwrap();
        // out-of-range read stays an empty return (v1 behaviour)
        let it = run(&mut state, registry, alice, &encode_get_link_at_calldata(9)).unwrap();
        assert!(it.return_data.is_none());
    }

    #[test]
    fn claim_log_and_second_self_claim_faults() {
        let reg_deployer = AccountId([0x0d; 32]);
        let alice = AccountId([0xa1; 32]);
        let mut state = StateStore::new();
        fund(&mut state, reg_deployer);
        fund(&mut state, alice);
        let registry = deploy_registry(
            &mut state,
            reg_deployer,
            &LinkedNftTokenRegistryV2Seed::default(),
        );
        let runtime = crate::reference_token_v2::reference_fungible_template_v2_bytecode();
        let code = asset_deploy_bytecode(&registry, &runtime);
        let salt = [0x52; 32];
        let tok = create2_contract_address(&alice, &salt, &code);
        let out = Vm::new()
            .execute(&deploy_tx(alice, 0, code.clone(), salt), &mut state)
            .unwrap();
        assert_eq!(out.logs.len(), 1);
        assert_eq!(out.logs[0].topics[0], LINKED_NFT_TOKEN_TOPIC_CLAIM_V2);
        assert_eq!(out.logs[0].topics[1], tok.0);
        assert_eq!(out.logs[0].data, word(&alice));
        // the asset (as CALLER) cannot claim a second time
        assert!(run(
            &mut state,
            registry,
            tok,
            &encode_claim_self_calldata_v2(&alice)
        )
        .is_err());
        // claim_self(0) faults
        assert!(run(
            &mut state,
            registry,
            AccountId([0x77; 32]),
            &encode_claim_self_calldata_v2(&AccountId([0; 32]))
        )
        .is_err());
    }

    #[test]
    fn deploy_fails_and_leaves_no_contract_when_the_self_claim_faults() {
        // Only reachable if the address already has a claimer (e.g. seeded from v1): the whole deploy fails.
        let reg_deployer = AccountId([0x0d; 32]);
        let alice = AccountId([0xa1; 32]);
        let mut state = StateStore::new();
        fund(&mut state, reg_deployer);
        fund(&mut state, alice);
        let runtime = crate::reference_token_v2::reference_fungible_template_v2_bytecode();
        let salt = [0x53; 32];
        let registry = deploy_registry(
            &mut state,
            reg_deployer,
            &LinkedNftTokenRegistryV2Seed::default(),
        );
        let tok =
            create2_contract_address(&alice, &salt, &asset_deploy_bytecode(&registry, &runtime));
        state.merge_contract_storage(
            registry,
            linked_nft_token_claimer_storage_key(&tok),
            [0x99; 32],
        );
        let code = asset_deploy_bytecode(&registry, &runtime);
        assert!(Vm::new()
            .execute(&deploy_tx(alice, 0, code, salt), &mut state)
            .is_err());
        assert!(state.get_contract_code(&tok).is_none());
    }

    #[test]
    fn seed_copies_claims_links_and_lets_only_the_recorded_deployer_claim() {
        let reg_deployer = AccountId([0x0d; 32]);
        let alice = AccountId([0xa1; 32]);
        let bob = AccountId([0xb0; 32]);
        let c1 = AccountId([0x11; 32]);
        let t1 = AccountId([0x21; 32]);
        let old = AccountId([0x31; 32]);
        let seed = LinkedNftTokenRegistryV2Seed {
            claims: vec![(c1, alice), (t1, alice)],
            links: vec![(c1, t1), (AccountId([0; 32]), AccountId([0; 32]))],
            deployers: vec![(old, bob)],
        };
        let mut state = StateStore::new();
        fund(&mut state, reg_deployer);
        let registry = deploy_registry(&mut state, reg_deployer, &seed);

        let it = run(&mut state, registry, bob, &encode_links_count_calldata()).unwrap();
        assert_eq!(it.return_data.unwrap(), word_u64(2).to_vec());
        let it = run(&mut state, registry, bob, &encode_get_link_at_calldata(0)).unwrap();
        assert_eq!(it.return_data.unwrap(), [word(&c1), word(&t1)].concat());
        let it = run(
            &mut state,
            registry,
            bob,
            &encode_get_asset_claimer_calldata(&c1),
        )
        .unwrap();
        assert_eq!(it.return_data.unwrap(), word(&alice));
        let it = run(
            &mut state,
            registry,
            bob,
            &encode_get_asset_deployer_calldata_v2(&old),
        )
        .unwrap();
        assert_eq!(it.return_data.unwrap(), word(&bob));

        // alice (not the deployer of `old`) cannot claim it; bob can, once.
        assert!(run(
            &mut state,
            registry,
            alice,
            &encode_claim_asset_calldata(&old)
        )
        .is_err());
        let it = run(
            &mut state,
            registry,
            bob,
            &encode_claim_asset_calldata(&old),
        )
        .unwrap();
        assert_eq!(it.logs[0].topics[0], LINKED_NFT_TOKEN_TOPIC_CLAIM_V2);
        let it = run(
            &mut state,
            registry,
            alice,
            &encode_get_asset_claimer_calldata(&old),
        )
        .unwrap();
        assert_eq!(it.return_data.unwrap(), word(&bob));
        // seeded claims keep working: alice can unlink and transfer her claim
        run(&mut state, registry, alice, &encode_unlink_at_calldata(0)).unwrap();
        run(
            &mut state,
            registry,
            alice,
            &encode_transfer_asset_claimer_calldata(&c1, &bob),
        )
        .unwrap();
        assert!(run(
            &mut state,
            registry,
            alice,
            &encode_transfer_asset_claimer_calldata(&c1, &bob)
        )
        .is_err());
        // unknown selector faults
        assert!(run(&mut state, registry, alice, &selector_word(0x42)).is_err());
    }

    #[test]
    fn claim_self_snippet_and_runtime_match_boing_sdk() {
        // boing-sdk buildLinkedNftTokenClaimSelfInitSnippetHex('0xbb…bb')
        let sdk_snippet = "7f00000000000000000000000000000000000000000000000000000000000000e77f000000000000000000000000000000000000000000000000000000000000000052337f0000000000000000000000000000000000000000000000000000000000000020527fbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb7f00000000000000000000000000000000000000000000000000000000000000007f00000000000000000000000000000000000000000000000000000000000000407f00000000000000000000000000000000000000000000000000000000000000007f0000000000000000000000000000000000000000000000000000000000000000f1";
        let ours: String =
            linked_nft_token_registry_v2_claim_self_init_snippet(&AccountId([0xbb; 32]))
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
        assert_eq!(ours, sdk_snippet);

        let sdk_file =
            include_str!("../../../boing-sdk/src/defaultLinkedNftTokenRegistryV2BytecodeHex.ts");
        let start = sdk_file.find("'0x").expect("hex literal") + 3;
        let end = start + sdk_file[start..].find('\'').expect("closing quote");
        let runtime: String = linked_nft_token_registry_v2_runtime_bytecode()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            &sdk_file[start..end],
            runtime,
            "regenerate the SDK file with dump_linked_nft_token_registry_v2"
        );
    }

    #[test]
    fn v2_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
        let registry = RuleRegistry::new();
        for code in [
            linked_nft_token_registry_v2_runtime_bytecode(),
            linked_nft_token_registry_v2_deploy_bytecode(&LinkedNftTokenRegistryV2Seed {
                claims: vec![(AccountId([1; 32]), AccountId([2; 32]))],
                links: vec![(AccountId([1; 32]), AccountId([3; 32]))],
                deployers: vec![(AccountId([4; 32]), AccountId([2; 32]))],
            }),
        ] {
            let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
            assert!(matches!(r, QaResult::Allow | QaResult::Unsure), "got {r:?}");
        }
    }
}
