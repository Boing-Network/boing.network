//! Reference **NFT** calldata layout (Boing-defined) for wallets and indexers.
//!
//! Not consensus-enforced — see `docs/BOING-REFERENCE-NFT.md`.
//!
//! [`reference_nft_collection_template_bytecode`] is a minimal **collection** contract: lazy admin
//! (first caller), `owner_of` / `transfer_nft` / `set_metadata_hash` per the reference doc.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// `owner_of(token_id)` — read path; contract returns current holder `AccountId` (e.g. via `RETURN`).
pub const SELECTOR_OWNER_OF: u8 = 0x03;
/// `transfer_nft(to, token_id)` — move `token_id` to `to` if `CALLER` is authorized.
pub const SELECTOR_TRANSFER_NFT: u8 = 0x04;
/// Optional: bind a 32-byte metadata commitment (URI hash, etc.) to `token_id`.
pub const SELECTOR_SET_METADATA_HASH: u8 = 0x05;

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

// --- Minimal collection VM bytecode (scratch memory, big-endian words) ---

const MEM_SCRATCH_SEL: u64 = 384;
const MEM_SCRATCH_TO: u64 = 352;
const MEM_SCRATCH_TID: u64 = 416;
const MEM_SCRATCH_HASH: u64 = 448;
const MEM_SCRATCH_OWNER: u64 = 320;
const MEM_RET_OWNER_OF: u64 = 512;

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

/// Minimal **reference NFT collection** bytecode for `contract_deploy_meta` with purpose **`nft`** / **`NFT`**.
///
/// Semantics:
/// - **Admin:** first successful call initializes [`ref_nft_collection_admin_key`] to `CALLER`.
/// - **`owner_of`:** returns `SLOAD(token_id ^ [`REF_NFT_OWNER_STORAGE_XOR`])` (zero word if unminted).
/// - **`transfer_nft`:** if unowned, only admin may set owner to `to` (lazy mint). If owned, only owner may transfer.
/// - **`set_metadata_hash`:** admin **or** current owner may write `token_id ^ [`REF_NFT_METADATA_STORAGE_XOR`]`.
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

    let off_unknown = c.len();
    patch_push32_dest(&mut c, fix_skip_owner_of, off_unknown);
    c.push(Opcode::Stop as u8);

    c
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
        it.run(deployer, collection, &encode_owner_of_calldata(&tid), &mut state)
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
