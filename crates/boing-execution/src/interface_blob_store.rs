//! On-chain **interface blob store**: built-in hosting for interface JSON ("Host on Boing").
//!
//! Publishers who have no web hosting store the (optionally deflate-compressed) interface JSON in
//! contract storage and record a `boing://iface-blob/<writer>/<id>` URI in the
//! [interface registry](crate::interface_registry). Explorers read the words back with
//! `boing_getContractStorage`, decode, and verify BLAKE3 against the registry record, exactly as
//! for an https URI. No consensus change: this is an ordinary contract.
//!
//! **Namespaces.** Word `i` of blob `id` written by `writer` lives at
//! `key = writer ^ BLOB_TAG ^ (id << 32) ^ i` (`id`: u64, `i`: u32). `writer` is the VM `CALLER`,
//! so an account can only ever write its own namespace: colliding with another writer would need
//! two Ed25519 public keys that agree in their top 20 bytes. Clients use the first 8 bytes of the
//! interface hash as `id`, so each version gets fresh slots and old versions stay readable.
//!
//! **Layout.** Word 0 is a header: bytes `0..4` = `b"BIF1"`, byte `4` = encoding
//! ([`IFACE_BLOB_ENCODING_RAW`] or [`IFACE_BLOB_ENCODING_DEFLATE_RAW`]), bytes `24..32` = stored
//! byte length (u64 BE). Words `1..` hold the stored bytes, zero padded.
//!
//! **Fees.** Each word is one `SSTORE` (20 000 gas), so cost scales with size; clients cap blobs
//! at [`IFACE_BLOB_MAX_STORED_BYTES`].
//!
//! See `docs/INTERFACE-REGISTRY.md` (Built-in hosting).

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// `put(id, start, count, words[count])`: `128 + 32 * count` bytes; writes the caller's namespace.
/// Reverts (invalid jump) when `id >= 2^64`, `start >= 2^32 - 64`, or `count` is not `1..=64`.
pub const SELECTOR_IFACE_BLOB_PUT: u8 = 0xE0;
/// Max words per `put` call.
pub const IFACE_BLOB_MAX_WORDS_PER_PUT: usize = 64;
/// Client-side cap on stored bytes (after compression): 24 KiB, i.e. 768 words + header.
pub const IFACE_BLOB_MAX_STORED_BYTES: usize = 24 * 1024;
/// Header encodings.
pub const IFACE_BLOB_ENCODING_RAW: u8 = 0;
pub const IFACE_BLOB_ENCODING_DEFLATE_RAW: u8 = 1;
/// Header magic.
pub const IFACE_BLOB_MAGIC: [u8; 4] = *b"BIF1";
/// URI prefix recorded in the interface registry.
pub const IFACE_BLOB_URI_PREFIX: &str = "boing://iface-blob/";

/// XOR tag: occupies bytes 0..20 so namespaces of different writers cannot overlap.
pub const IFACE_BLOB_TAG: [u8; 32] =
    *b"BOING_IFACE_BLOB_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for [`interface_blob_store_bytecode`].
pub const INTERFACE_BLOB_STORE_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_IFACE_BLOB_STORE_V1\x00\x00\x00\x00\x00\x00\x00";

fn push32(code: &mut Vec<u8>, w: &[u8; 32]) {
    code.push(Opcode::Push32 as u8);
    code.extend_from_slice(w);
}

fn patch_push32_dest(code: &mut [u8], at: usize, dest: usize) {
    code[at + 1..at + 33].copy_from_slice(&word_u64(dest as u64));
}

fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

fn word_pow2(bits: u32) -> [u8; 32] {
    let mut w = [0u8; 32];
    let byte = 31 - (bits / 8) as usize;
    w[byte] = 1u8 << (bits % 8);
    w
}

/// Storage key of word `index` of blob `id` written by `writer`.
#[must_use]
pub fn interface_blob_storage_key(writer: &AccountId, id: u64, index: u32) -> [u8; 32] {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = writer.0[i] ^ IFACE_BLOB_TAG[i];
    }
    for (i, b) in id.to_be_bytes().iter().enumerate() {
        k[20 + i] ^= b;
    }
    for (i, b) in index.to_be_bytes().iter().enumerate() {
        k[28 + i] ^= b;
    }
    k
}

/// Blob id for an interface hash: its first 8 bytes (big endian).
#[must_use]
pub fn interface_blob_id(interface_hash: &[u8; 32]) -> u64 {
    u64::from_be_bytes(interface_hash[..8].try_into().unwrap())
}

/// `boing://iface-blob/0x<writer>/<id as 16 hex digits>` (102 bytes; fits the 192-byte URI field).
#[must_use]
pub fn interface_blob_uri(writer: &AccountId, id: u64) -> String {
    let hex: String = writer.0.iter().map(|b| format!("{b:02x}")).collect();
    format!("{IFACE_BLOB_URI_PREFIX}0x{hex}/{id:016x}")
}

/// Parse a `boing://iface-blob/...` URI into `(writer, id)`.
#[must_use]
pub fn parse_interface_blob_uri(uri: &str) -> Option<(AccountId, u64)> {
    let rest = uri.strip_prefix(IFACE_BLOB_URI_PREFIX)?;
    let (w, id) = rest.split_once('/')?;
    let w = w.strip_prefix("0x").unwrap_or(w);
    if w.len() != 64 || id.len() != 16 {
        return None;
    }
    let mut a = [0u8; 32];
    for (i, slot) in a.iter_mut().enumerate() {
        *slot = u8::from_str_radix(w.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some((AccountId(a), u64::from_str_radix(id, 16).ok()?))
}

/// Header word + data words for `stored` bytes (already encoded as `encoding`).
pub fn interface_blob_words(stored: &[u8], encoding: u8) -> Result<Vec<[u8; 32]>, String> {
    if stored.is_empty() || stored.len() > IFACE_BLOB_MAX_STORED_BYTES {
        return Err(format!(
            "blob is {} bytes; must be 1..={IFACE_BLOB_MAX_STORED_BYTES}",
            stored.len()
        ));
    }
    let mut header = [0u8; 32];
    header[..4].copy_from_slice(&IFACE_BLOB_MAGIC);
    header[4] = encoding;
    header[24..].copy_from_slice(&(stored.len() as u64).to_be_bytes());
    let mut out = vec![header];
    for c in stored.chunks(32) {
        let mut w = [0u8; 32];
        w[..c.len()].copy_from_slice(c);
        out.push(w);
    }
    Ok(out)
}

/// Inverse of [`interface_blob_words`]: `(encoding, stored bytes)`, or `None` on a bad header.
#[must_use]
pub fn decode_interface_blob_words(words: &[[u8; 32]]) -> Option<(u8, Vec<u8>)> {
    let h = words.first()?;
    if h[..4] != IFACE_BLOB_MAGIC {
        return None;
    }
    let len = u64::from_be_bytes(h[24..].try_into().unwrap()) as usize;
    if len == 0 || len > IFACE_BLOB_MAX_STORED_BYTES || words.len() < 1 + len.div_ceil(32) {
        return None;
    }
    let mut b: Vec<u8> = words[1..].iter().flatten().copied().collect();
    b.truncate(len);
    Some((h[4], b))
}

/// `put` calldata chunks (each at most [`IFACE_BLOB_MAX_WORDS_PER_PUT`] words) for `words`.
#[must_use]
pub fn encode_iface_blob_put_calldatas(id: u64, words: &[[u8; 32]]) -> Vec<Vec<u8>> {
    words
        .chunks(IFACE_BLOB_MAX_WORDS_PER_PUT)
        .enumerate()
        .map(|(n, chunk)| {
            let mut v = selector_word(SELECTOR_IFACE_BLOB_PUT).to_vec();
            v.extend_from_slice(&word_u64(id));
            v.extend_from_slice(&word_u64((n * IFACE_BLOB_MAX_WORDS_PER_PUT) as u64));
            v.extend_from_slice(&word_u64(chunk.len() as u64));
            for w in chunk {
                v.extend_from_slice(w);
            }
            v
        })
        .collect()
}

/// Canonical interface blob store bytecode (v1). CREATE2: [`INTERFACE_BLOB_STORE_CREATE2_SALT_V1`].
#[must_use]
pub fn interface_blob_store_bytecode() -> Vec<u8> {
    // Calldata occupies memory 0..(128 + 64*32 = 2176); scratch well above it.
    const MEM_BASE: u64 = 8192;
    const MEM_I: u64 = 8224;
    let mut c: Vec<u8> = Vec::new();
    let mut fix_reverts: Vec<usize> = Vec::new();

    let revert_if_zero = |c: &mut Vec<u8>, fixes: &mut Vec<usize>| {
        c.push(Opcode::IsZero as u8);
        fixes.push(c.len());
        push32(c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
    };

    // dispatch
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_IFACE_BLOB_PUT));
    c.push(Opcode::Eq as u8);
    let fix_put = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);

    // ========== put ==========
    let off_put = c.len();
    patch_push32_dest(&mut c, fix_put, off_put);
    // id < 2^64
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_pow2(64));
    c.push(Opcode::Lt as u8);
    revert_if_zero(&mut c, &mut fix_reverts);
    // start < 2^32 - 64
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64((1u64 << 32) - IFACE_BLOB_MAX_WORDS_PER_PUT as u64));
    c.push(Opcode::Lt as u8);
    revert_if_zero(&mut c, &mut fix_reverts);
    // count < 65
    push32(&mut c, &word_u64(96));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(IFACE_BLOB_MAX_WORDS_PER_PUT as u64 + 1));
    c.push(Opcode::Lt as u8);
    revert_if_zero(&mut c, &mut fix_reverts);
    // count != 0
    push32(&mut c, &word_u64(96));
    c.push(Opcode::MLoad as u8);
    revert_if_zero(&mut c, &mut fix_reverts);
    // base = CALLER ^ TAG ^ (id << 32)
    c.push(Opcode::Caller as u8);
    push32(&mut c, &IFACE_BLOB_TAG);
    c.push(Opcode::Xor as u8);
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(32));
    c.push(Opcode::Shl as u8);
    c.push(Opcode::Xor as u8);
    push32(&mut c, &word_u64(MEM_BASE));
    c.push(Opcode::MStore as u8);
    // i = 0
    push32(&mut c, &word_u64(0));
    push32(&mut c, &word_u64(MEM_I));
    c.push(Opcode::MStore as u8);

    let off_loop = c.len();
    // if i == count: stop
    push32(&mut c, &word_u64(MEM_I));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(96));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Eq as u8);
    let fix_end = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    // value = MLOAD(128 + 32 * i)
    push32(&mut c, &word_u64(128));
    push32(&mut c, &word_u64(MEM_I));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(32));
    c.push(Opcode::Mul as u8);
    c.push(Opcode::Add as u8);
    c.push(Opcode::MLoad as u8);
    // key = base ^ (start + i)
    push32(&mut c, &word_u64(MEM_BASE));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_I));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);
    // i += 1
    push32(&mut c, &word_u64(MEM_I));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(1));
    c.push(Opcode::Add as u8);
    push32(&mut c, &word_u64(MEM_I));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(off_loop as u64));
    c.push(Opcode::Jump as u8);

    let off_end = c.len();
    patch_push32_dest(&mut c, fix_end, off_end);
    c.push(Opcode::Stop as u8);

    // revert: unconditional jump past the end of code -> VmError::InvalidJump (whole tx fails)
    let off_revert = c.len();
    push32(&mut c, &word_u64(u64::MAX));
    c.push(Opcode::Jump as u8);
    for at in fix_reverts {
        patch_push32_dest(&mut c, at, off_revert);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;

    fn aid(byte: u8) -> AccountId {
        AccountId([byte; 32])
    }

    fn put(state: &mut StateStore, store: AccountId, caller: AccountId, cd: &[u8]) -> bool {
        let mut it = Interpreter::new(interface_blob_store_bytecode(), 8_000_000);
        it.run(caller, store, cd, state).is_ok()
    }

    fn read(state: &StateStore, store: AccountId, writer: &AccountId, id: u64, n: usize) -> Vec<[u8; 32]> {
        (0..n)
            .map(|i| state.get_contract_storage(&store, &interface_blob_storage_key(writer, id, i as u32)))
            .collect()
    }

    #[test]
    fn salt_uri_and_words_roundtrip() {
        assert_eq!(&INTERFACE_BLOB_STORE_CREATE2_SALT_V1[..25], b"BOING_IFACE_BLOB_STORE_V1");
        let w = aid(0xab);
        let uri = interface_blob_uri(&w, 0x0123_4567_89ab_cdef);
        assert!(uri.len() <= 192);
        // cross-checked in boing-sdk tests/interfaceBlobStore.test.ts
        let k: String = interface_blob_storage_key(&w, 0x0123_4567_89ab_cdef, 5).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(k, "e9e4e2e5ecf4e2edeae8eef4e9e7e4e9f4fd9aabaa88eecc22006644abababae");
        assert_eq!(parse_interface_blob_uri(&uri), Some((w, 0x0123_4567_89ab_cdef)));
        assert_eq!(parse_interface_blob_uri("https://x"), None);
        let data: Vec<u8> = (0..100u8).collect();
        let words = interface_blob_words(&data, IFACE_BLOB_ENCODING_DEFLATE_RAW).unwrap();
        assert_eq!(words.len(), 1 + 4);
        assert_eq!(decode_interface_blob_words(&words), Some((1, data)));
        assert!(interface_blob_words(&[], 0).is_err());
        assert!(interface_blob_words(&vec![1; IFACE_BLOB_MAX_STORED_BYTES + 1], 0).is_err());
    }

    #[test]
    fn put_roundtrip_multi_chunk_and_namespaces() {
        let store = aid(0xfb);
        let alice = aid(0xa1);
        let mallory = aid(0x66);
        let mut state = StateStore::new();
        state.set_contract_code(store, interface_blob_store_bytecode());

        let json: Vec<u8> = br#"{"functions":[{"selector":1,"name":"transfer"}]}"#.repeat(60);
        let h = crate::interface_json_hash(&json);
        let id = interface_blob_id(&h);
        let words = interface_blob_words(&json, IFACE_BLOB_ENCODING_RAW).unwrap();
        let cds = encode_iface_blob_put_calldatas(id, &words);
        assert!(cds.len() > 1, "exercise several put calls");
        for cd in &cds {
            assert!(put(&mut state, store, alice, cd));
        }
        let back = read(&state, store, &alice, id, words.len());
        assert_eq!(back, words);
        let (enc, bytes) = decode_interface_blob_words(&back).unwrap();
        assert_eq!(enc, IFACE_BLOB_ENCODING_RAW);
        assert_eq!(crate::interface_json_hash(&bytes), h);

        // mallory writing the same id only touches mallory's namespace
        let junk = interface_blob_words(b"junk", 0).unwrap();
        for cd in encode_iface_blob_put_calldatas(id, &junk) {
            assert!(put(&mut state, store, mallory, &cd));
        }
        assert_eq!(read(&state, store, &alice, id, words.len()), words);
        assert_eq!(read(&state, store, &mallory, id, junk.len()), junk);
    }

    #[test]
    fn put_rejects_bad_arguments() {
        let store = aid(0xfb);
        let alice = aid(0xa1);
        let mut state = StateStore::new();
        state.set_contract_code(store, interface_blob_store_bytecode());
        let mk = |id: [u8; 32], start: u64, count: u64, n: usize| {
            let mut v = selector_word(SELECTOR_IFACE_BLOB_PUT).to_vec();
            v.extend_from_slice(&id);
            v.extend_from_slice(&word_u64(start));
            v.extend_from_slice(&word_u64(count));
            v.extend(std::iter::repeat(7u8).take(32 * n));
            v
        };
        assert!(put(&mut state, store, alice, &mk(word_u64(1), 0, 1, 1)));
        assert!(!put(&mut state, store, alice, &mk(word_u64(1), 0, 0, 0)), "count 0");
        assert!(!put(&mut state, store, alice, &mk(word_u64(1), 0, 65, 65)), "count 65");
        assert!(!put(&mut state, store, alice, &mk(word_pow2(64), 0, 1, 1)), "id >= 2^64");
        assert!(!put(&mut state, store, alice, &mk(word_u64(1), (1 << 32) - 64, 1, 1)), "start too big");
        assert!(put(&mut state, store, alice, &mk(word_u64(1), 0, 64, 64)), "64 words ok");
        // unknown selector: no-op success
        let mut other = selector_word(0x01).to_vec();
        other.extend_from_slice(&[0u8; 96]);
        assert!(put(&mut state, store, alice, &other));
    }

    #[test]
    fn gas_scales_with_size() {
        let store = aid(0xfb);
        let mut state = StateStore::new();
        state.set_contract_code(store, interface_blob_store_bytecode());
        let words = vec![[9u8; 32]; 64];
        let cd = &encode_iface_blob_put_calldatas(5, &words)[0];
        let mut it = Interpreter::new(interface_blob_store_bytecode(), 8_000_000);
        it.run(aid(1), store, cd, &mut state).unwrap();
        let used = it.gas_used;
        assert!(used >= 64 * 20_000 && used < 2_000_000, "gas used {used}");
    }
}
