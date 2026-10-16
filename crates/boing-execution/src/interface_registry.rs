//! On-chain **interface registry**: a contract's deployer publishes the hash and URI of an
//! interface JSON (function names, params, kinds) so explorers can show authoritative names.
//!
//! Auth: first-caller `claim_asset(contract)` (same pattern as
//! [`crate::builder_attestation_registry`]); only the claimer may `set_interface`. The VM cannot
//! see who deployed a contract, so **clients must check off-chain that the claimer is the
//! contract's deployer** (deploy tx sender) before showing a "Published by deployer" badge.
//!
//! The interface hash is BLAKE3 of the exact JSON bytes served at the URI. The URI is stored
//! inline (UTF-8, up to [`INTERFACE_REGISTRY_MAX_URI_BYTES`] bytes, zero padded).
//!
//! See `docs/INTERFACE-REGISTRY.md`.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::selector_word;

/// `claim_asset(contract)`: **64** bytes. First caller becomes claimer; re-claim by claimer is a no-op.
pub const SELECTOR_IFACE_CLAIM_ASSET: u8 = 0xD0;
/// `set_interface(contract, interface_hash, uri_len, uri[6 words])`: **320** bytes. Claimer only.
pub const SELECTOR_IFACE_SET_INTERFACE: u8 = 0xD1;
/// `get_interface(contract)`: **64** bytes; returns **320** bytes
/// (claimer, publisher, interface_hash, uri_len, uri[6 words]).
pub const SELECTOR_IFACE_GET_INTERFACE: u8 = 0xD2;
/// `get_asset_claimer(contract)`: **64** bytes; returns the claimer word.
pub const SELECTOR_IFACE_GET_CLAIMER: u8 = 0xD3;
/// `transfer_asset_claimer(contract, new_claimer)`: **96** bytes. Claimer only.
pub const SELECTOR_IFACE_TRANSFER_CLAIMER: u8 = 0xD4;

/// Max URI length in bytes (6 storage words).
pub const INTERFACE_REGISTRY_MAX_URI_BYTES: usize = 192;
const URI_WORDS: usize = INTERFACE_REGISTRY_MAX_URI_BYTES / 32;

/// `Log3` topic0 after `set_interface`: topic1 = contract, topic2 = interface_hash, data = URI words.
pub const IFACE_TOPIC_SET: [u8; 32] =
    *b"BOING_IFACE_SET_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for [`interface_registry_bytecode`].
pub const INTERFACE_REGISTRY_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_INTERFACE_REGISTRY_V1\x00\x00\x00\x00\x00";

/// XOR mask for claimer slots: `key = contract ^ MASK`.
pub const IFACE_CLAIMER_XOR: [u8; 32] =
    *b"BOING_IFACE_CLAIMER_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Base XOR mask for record fields: `key = contract ^ (MASK with byte 31 = field)`.
pub const IFACE_FIELD_XOR_BASE: [u8; 32] =
    *b"BOING_IFACE_FIELD_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Field ids: 0 = interface_hash, 1 = uri_len, 2..=7 = uri words, 8 = publisher.
pub const IFACE_FIELD_HASH: u8 = 0;
pub const IFACE_FIELD_URI_LEN: u8 = 1;
pub const IFACE_FIELD_URI0: u8 = 2;
pub const IFACE_FIELD_PUBLISHER: u8 = 8;

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

fn xor32(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..32 {
        o[i] = a[i] ^ b[i];
    }
    o
}

fn field_mask(field: u8) -> [u8; 32] {
    let mut m = IFACE_FIELD_XOR_BASE;
    m[31] = field;
    m
}

#[must_use]
pub fn interface_registry_claimer_storage_key(contract: &AccountId) -> [u8; 32] {
    xor32(&contract.0, &IFACE_CLAIMER_XOR)
}

#[must_use]
pub fn interface_registry_field_storage_key(contract: &AccountId, field: u8) -> [u8; 32] {
    assert!(field <= IFACE_FIELD_PUBLISHER);
    xor32(&contract.0, &field_mask(field))
}

/// URI bytes → `(uri_len, 6 zero-padded words)`. Errors when longer than 192 bytes.
pub fn interface_registry_uri_words(uri: &str) -> Result<(u64, [[u8; 32]; URI_WORDS]), String> {
    let b = uri.as_bytes();
    if b.len() > INTERFACE_REGISTRY_MAX_URI_BYTES {
        return Err(format!(
            "interface URI is {} bytes; max {INTERFACE_REGISTRY_MAX_URI_BYTES}",
            b.len()
        ));
    }
    let mut words = [[0u8; 32]; URI_WORDS];
    for (i, chunk) in b.chunks(32).enumerate() {
        words[i][..chunk.len()].copy_from_slice(chunk);
    }
    Ok((b.len() as u64, words))
}

/// BLAKE3 of the exact interface JSON bytes.
#[must_use]
pub fn interface_json_hash(json_bytes: &[u8]) -> [u8; 32] {
    let mut h = boing_primitives::hasher();
    h.update(json_bytes);
    *h.finalize().as_bytes()
}

#[must_use]
pub fn encode_iface_claim_asset_calldata(contract: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_IFACE_CLAIM_ASSET).to_vec();
    v.extend_from_slice(&contract.0);
    v
}

/// `set_interface` calldata (320 bytes).
pub fn encode_iface_set_interface_calldata(
    contract: &AccountId,
    interface_hash: &[u8; 32],
    uri: &str,
) -> Result<Vec<u8>, String> {
    let (len, words) = interface_registry_uri_words(uri)?;
    let mut v = selector_word(SELECTOR_IFACE_SET_INTERFACE).to_vec();
    v.extend_from_slice(&contract.0);
    v.extend_from_slice(interface_hash);
    v.extend_from_slice(&word_u64(len));
    for w in &words {
        v.extend_from_slice(w);
    }
    Ok(v)
}

#[must_use]
pub fn encode_iface_get_interface_calldata(contract: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_IFACE_GET_INTERFACE).to_vec();
    v.extend_from_slice(&contract.0);
    v
}

#[must_use]
pub fn encode_iface_get_claimer_calldata(contract: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_IFACE_GET_CLAIMER).to_vec();
    v.extend_from_slice(&contract.0);
    v
}

#[must_use]
pub fn encode_iface_transfer_claimer_calldata(
    contract: &AccountId,
    new_claimer: &AccountId,
) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_IFACE_TRANSFER_CLAIMER).to_vec();
    v.extend_from_slice(&contract.0);
    v.extend_from_slice(&new_claimer.0);
    v
}

/// Decoded `get_interface` return data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceRecord {
    pub claimer: AccountId,
    pub publisher: AccountId,
    pub interface_hash: [u8; 32],
    pub uri: String,
}

/// Decode 320-byte `get_interface` return data. `None` when nothing was published.
#[must_use]
pub fn decode_iface_get_interface_return(data: &[u8]) -> Option<InterfaceRecord> {
    if data.len() < 320 {
        return None;
    }
    let w = |i: usize| -> [u8; 32] { data[i * 32..i * 32 + 32].try_into().unwrap() };
    let hash = w(2);
    if hash == [0u8; 32] {
        return None;
    }
    let mut len_b = [0u8; 8];
    len_b.copy_from_slice(&data[3 * 32 + 24..4 * 32]);
    let len = (u64::from_be_bytes(len_b) as usize).min(INTERFACE_REGISTRY_MAX_URI_BYTES);
    let uri = String::from_utf8_lossy(&data[4 * 32..4 * 32 + len]).into_owned();
    Some(InterfaceRecord {
        claimer: AccountId(w(0)),
        publisher: AccountId(w(1)),
        interface_hash: hash,
        uri,
    })
}

/// Abort unless `SLOAD(calldata[32] ^ CLAIMER_XOR) == CALLER`.
fn append_require_caller_is_claimer(code: &mut Vec<u8>, fix_aborts: &mut Vec<usize>) {
    push32(code, &word_u64(32));
    code.push(Opcode::MLoad as u8);
    push32(code, &IFACE_CLAIMER_XOR);
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

/// Push storage key `calldata[32] ^ field_mask(field)`.
fn append_field_key(code: &mut Vec<u8>, field: u8) {
    push32(code, &word_u64(32));
    code.push(Opcode::MLoad as u8);
    push32(code, &field_mask(field));
    code.push(Opcode::Xor as u8);
}

fn append_abort_if_top_nonzero_is_zero(code: &mut Vec<u8>, fix_aborts: &mut Vec<usize>) {
    code.push(Opcode::IsZero as u8);
    let fix = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);
    fix_aborts.push(fix);
}

/// Canonical interface registry bytecode (v1). CREATE2: [`INTERFACE_REGISTRY_CREATE2_SALT_V1`].
#[must_use]
pub fn interface_registry_bytecode() -> Vec<u8> {
    // Calldata occupies memory 0..320; scratch above it.
    const MEM_KEY: u64 = 512;
    const MEM_TMP: u64 = 544;
    const MEM_RET: u64 = 640;

    let mut c: Vec<u8> = Vec::new();
    let mut fix_aborts: Vec<usize> = Vec::new();

    let sels = [
        SELECTOR_IFACE_CLAIM_ASSET,
        SELECTOR_IFACE_SET_INTERFACE,
        SELECTOR_IFACE_GET_INTERFACE,
        SELECTOR_IFACE_GET_CLAIMER,
        SELECTOR_IFACE_TRANSFER_CLAIMER,
    ];
    let mut fix_jumps = [0usize; 5];
    for (i, sel) in sels.into_iter().enumerate() {
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
        // abort if contract == 0
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        {
            let fix = c.len();
            push32(&mut c, &[0u8; 32]);
            c.push(Opcode::JumpI as u8);
            fix_aborts.push(fix);
        }
        // key → MEM_KEY
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &IFACE_CLAIMER_XOR);
        c.push(Opcode::Xor as u8);
        push32(&mut c, &word_u64(MEM_KEY));
        c.push(Opcode::MStore as u8);
        // existing → MEM_TMP
        push32(&mut c, &word_u64(MEM_KEY));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MStore as u8);
        // unclaimed → set
        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::IsZero as u8);
        let fix_set = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
        // claimed by someone else → abort; by caller → no-op
        push32(&mut c, &word_u64(MEM_TMP));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::Caller as u8);
        c.push(Opcode::Eq as u8);
        append_abort_if_top_nonzero_is_zero(&mut c, &mut fix_aborts);
        c.push(Opcode::Stop as u8);

        let off_set = c.len();
        patch_push32_dest(&mut c, fix_set, off_set);
        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_KEY));
        c.push(Opcode::MLoad as u8);
        c.push(Opcode::SStore as u8);
        c.push(Opcode::Stop as u8);
    }

    // ========== set_interface ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[1], off);
        append_require_caller_is_claimer(&mut c, &mut fix_aborts);
        // require interface_hash != 0
        push32(&mut c, &word_u64(64));
        c.push(Opcode::MLoad as u8);
        append_abort_if_top_nonzero_is_zero(&mut c, &mut fix_aborts);
        // require uri_len < MAX + 1
        push32(&mut c, &word_u64(96));
        c.push(Opcode::MLoad as u8);
        push32(
            &mut c,
            &word_u64(INTERFACE_REGISTRY_MAX_URI_BYTES as u64 + 1),
        );
        c.push(Opcode::Lt as u8);
        append_abort_if_top_nonzero_is_zero(&mut c, &mut fix_aborts);

        // hash, uri_len, uri words: SSTORE(key_f, calldata word)
        let mut fields: Vec<(u8, u64)> = vec![(IFACE_FIELD_HASH, 64), (IFACE_FIELD_URI_LEN, 96)];
        for i in 0..URI_WORDS {
            fields.push((IFACE_FIELD_URI0 + i as u8, 128 + 32 * i as u64));
        }
        for (field, cd_off) in fields {
            push32(&mut c, &word_u64(cd_off));
            c.push(Opcode::MLoad as u8);
            append_field_key(&mut c, field);
            c.push(Opcode::SStore as u8);
        }
        // publisher = CALLER
        c.push(Opcode::Caller as u8);
        append_field_key(&mut c, IFACE_FIELD_PUBLISHER);
        c.push(Opcode::SStore as u8);

        // Log3(topic0, contract, interface_hash; data = uri words)
        push32(&mut c, &IFACE_TOPIC_SET);
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(64));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(INTERFACE_REGISTRY_MAX_URI_BYTES as u64));
        push32(&mut c, &word_u64(128));
        c.push(Opcode::Log3 as u8);
        c.push(Opcode::Stop as u8);
    }

    // ========== get_interface ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[2], off);
        // claimer
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &IFACE_CLAIMER_XOR);
        c.push(Opcode::Xor as u8);
        c.push(Opcode::SLoad as u8);
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::MStore as u8);
        // publisher, hash, uri_len, uri words
        let mut order: Vec<u8> = vec![IFACE_FIELD_PUBLISHER, IFACE_FIELD_HASH, IFACE_FIELD_URI_LEN];
        for i in 0..URI_WORDS {
            order.push(IFACE_FIELD_URI0 + i as u8);
        }
        for (slot, field) in order.into_iter().enumerate() {
            append_field_key(&mut c, field);
            c.push(Opcode::SLoad as u8);
            push32(&mut c, &word_u64(MEM_RET + 32 * (slot as u64 + 1)));
            c.push(Opcode::MStore as u8);
        }
        push32(&mut c, &word_u64(320));
        push32(&mut c, &word_u64(MEM_RET));
        c.push(Opcode::Return as u8);
        c.push(Opcode::Stop as u8);
    }

    // ========== get_asset_claimer ==========
    {
        let off = c.len();
        patch_push32_dest(&mut c, fix_jumps[3], off);
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &IFACE_CLAIMER_XOR);
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
        patch_push32_dest(&mut c, fix_jumps[4], off);
        push32(&mut c, &word_u64(64));
        c.push(Opcode::MLoad as u8);
        append_abort_if_top_nonzero_is_zero(&mut c, &mut fix_aborts);
        append_require_caller_is_claimer(&mut c, &mut fix_aborts);
        push32(&mut c, &word_u64(64));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(32));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &IFACE_CLAIMER_XOR);
        c.push(Opcode::Xor as u8);
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
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;

    fn aid(byte: u8) -> AccountId {
        AccountId([byte; 32])
    }

    fn run(
        state: &mut StateStore,
        registry: AccountId,
        caller: AccountId,
        calldata: &[u8],
    ) -> Interpreter {
        let mut it = Interpreter::new(interface_registry_bytecode(), 8_000_000);
        it.run(caller, registry, calldata, state).unwrap();
        it
    }

    fn get(
        state: &mut StateStore,
        registry: AccountId,
        contract: &AccountId,
    ) -> Option<InterfaceRecord> {
        let it = run(
            state,
            registry,
            aid(0x99),
            &encode_iface_get_interface_calldata(contract),
        );
        let data = it.return_data.clone().expect("get_interface returns data");
        assert_eq!(data.len(), 320);
        decode_iface_get_interface_return(&data)
    }

    #[test]
    fn salt_and_encodings() {
        assert_eq!(
            &INTERFACE_REGISTRY_CREATE2_SALT_V1[..27],
            b"BOING_INTERFACE_REGISTRY_V1"
        );
        let a = aid(1);
        assert_eq!(encode_iface_claim_asset_calldata(&a).len(), 64);
        let set = encode_iface_set_interface_calldata(&a, &[7; 32], "https://x/y.json").unwrap();
        assert_eq!(set.len(), 320);
        assert_eq!(set[31], SELECTOR_IFACE_SET_INTERFACE);
        assert_eq!(encode_iface_get_interface_calldata(&a).len(), 64);
        assert_eq!(
            encode_iface_transfer_claimer_calldata(&a, &aid(2)).len(),
            96
        );
        assert!(encode_iface_set_interface_calldata(&a, &[7; 32], &"x".repeat(193)).is_err());
        assert!(encode_iface_set_interface_calldata(&a, &[7; 32], &"x".repeat(192)).is_ok());
    }

    #[test]
    fn storage_keys_distinct() {
        let a = aid(0xab);
        let mut keys = vec![interface_registry_claimer_storage_key(&a)];
        for f in 0..=IFACE_FIELD_PUBLISHER {
            keys.push(interface_registry_field_storage_key(&a, f));
        }
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n);
    }

    #[test]
    fn claim_set_get_roundtrip_and_auth() {
        let registry = aid(0xfa);
        let deployer = aid(0xd1);
        let mallory = aid(0x66);
        let contract = aid(0x11);
        let json = br#"{"functions":[]}"#;
        let h = interface_json_hash(json);
        let uri = "https://boing.network/interfaces/testnet/1111.json";

        let mut state = StateStore::new();
        state.set_contract_code(registry, interface_registry_bytecode());

        assert_eq!(get(&mut state, registry, &contract), None);

        // cannot set before claiming
        let cd = encode_iface_set_interface_calldata(&contract, &h, uri).unwrap();
        let it = run(&mut state, registry, deployer, &cd);
        assert!(it.logs.is_empty());
        assert_eq!(get(&mut state, registry, &contract), None);

        run(
            &mut state,
            registry,
            deployer,
            &encode_iface_claim_asset_calldata(&contract),
        );
        // second claimer cannot take it
        run(
            &mut state,
            registry,
            mallory,
            &encode_iface_claim_asset_calldata(&contract),
        );
        let it = run(
            &mut state,
            registry,
            mallory,
            &encode_iface_get_claimer_calldata(&contract),
        );
        assert_eq!(it.return_data.as_deref(), Some(&deployer.0[..]));

        // mallory cannot set
        let it = run(&mut state, registry, mallory, &cd);
        assert!(it.logs.is_empty());

        // zero hash rejected
        let zero = encode_iface_set_interface_calldata(&contract, &[0; 32], uri).unwrap();
        let it = run(&mut state, registry, deployer, &zero);
        assert!(it.logs.is_empty());

        let it = run(&mut state, registry, deployer, &cd);
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], IFACE_TOPIC_SET);
        assert_eq!(it.logs[0].topics[1], contract.0);
        assert_eq!(it.logs[0].topics[2], h);
        assert_eq!(&it.logs[0].data[..uri.len()], uri.as_bytes());

        let rec = get(&mut state, registry, &contract).unwrap();
        assert_eq!(
            rec,
            InterfaceRecord {
                claimer: deployer,
                publisher: deployer,
                interface_hash: h,
                uri: uri.to_string()
            }
        );

        // update with a longer URI, then a shorter one (stale bytes must not leak)
        let long = format!("https://example.com/{}", "a".repeat(170));
        let h2 = interface_json_hash(b"v2");
        run(
            &mut state,
            registry,
            deployer,
            &encode_iface_set_interface_calldata(&contract, &h2, &long).unwrap(),
        );
        assert_eq!(get(&mut state, registry, &contract).unwrap().uri, long);
        run(
            &mut state,
            registry,
            deployer,
            &encode_iface_set_interface_calldata(&contract, &h, "ipfs://x").unwrap(),
        );
        let rec = get(&mut state, registry, &contract).unwrap();
        assert_eq!(rec.uri, "ipfs://x");
        assert_eq!(rec.interface_hash, h);

        // over-long uri_len word rejected
        let mut bad = cd.clone();
        bad[96 + 31] = 200;
        let it = run(&mut state, registry, deployer, &bad);
        assert!(it.logs.is_empty());

        // transfer claimer; old claimer loses rights, publisher stays until next set
        run(
            &mut state,
            registry,
            deployer,
            &encode_iface_transfer_claimer_calldata(&contract, &mallory),
        );
        let it = run(&mut state, registry, deployer, &cd);
        assert!(it.logs.is_empty());
        let rec = get(&mut state, registry, &contract).unwrap();
        assert_eq!(rec.claimer, mallory);
        assert_eq!(rec.publisher, deployer);
        let it = run(&mut state, registry, mallory, &cd);
        assert_eq!(it.logs.len(), 1);
        assert_eq!(
            get(&mut state, registry, &contract).unwrap().publisher,
            mallory
        );
    }

    #[test]
    fn unknown_selector_and_zero_contract_are_noops() {
        let registry = aid(0xfa);
        let mut state = StateStore::new();
        state.set_contract_code(registry, interface_registry_bytecode());
        let it = run(&mut state, registry, aid(1), &selector_word(0xE0));
        assert!(it.logs.is_empty() && it.return_data.is_none());
        run(
            &mut state,
            registry,
            aid(1),
            &encode_iface_claim_asset_calldata(&AccountId([0; 32])),
        );
        let it = run(
            &mut state,
            registry,
            aid(1),
            &encode_iface_get_claimer_calldata(&AccountId([0; 32])),
        );
        assert_eq!(it.return_data.as_deref(), Some(&[0u8; 32][..]));
    }

    #[test]
    fn interface_registry_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
        let code = interface_registry_bytecode();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &RuleRegistry::new());
        assert!(matches!(r, QaResult::Allow | QaResult::Unsure), "got {r:?}");
    }
}
