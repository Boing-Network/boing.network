//! On-chain **builder attestation** registry — scaffold (selectors, salts, encoders).
//!
//! Dual asset-claimer auth (same pattern as [`crate::linked_nft_token_registry`]).
//! Full bytecode assembler is intentionally deferred until the product design is locked;
//! see Agent Store `docs/builder-attestations-onchain.md` and `docs/BOING-BUILDER-ATTESTATION.md`.
//!
//! Selectors are **per-contract** (no collision with companions `0xE0`–`0xE6` on a different AccountId).

use boing_primitives::AccountId;

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

/// `Log3` topic0 after successful attest (scaffold; packing finalized with bytecode).
/// topic1 = collection, topic2 = token.
pub const BUILDER_ATTEST_TOPIC_ATTEST: [u8; 32] =
    *b"BOING_BLDR_ATTEST_REG1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// `Log3` topic0 after successful revoke.
pub const BUILDER_ATTEST_TOPIC_REVOKE: [u8; 32] =
    *b"BOING_BLDR_ATTEST_UNL1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for the future attestation-registry bytecode.
pub const BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_BUILDER_ATTEST_REG_V1\x00\x00\x00\x00\x00";

/// Inclusive upper bound on stored attestation slots (includes tombstones).
pub const BUILDER_ATTESTATION_REGISTRY_MAX_SLOTS: u64 = 4096;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn aid(byte: u8) -> AccountId {
        AccountId([byte; 32])
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
        // field 0 index 1 == base + 4; field 0 index 0 == base
        assert_eq!(k4[31].wrapping_sub(k0[31]), 4);
    }

    #[test]
    fn claimer_xor_distinct_from_zero() {
        let a = aid(0xab);
        let key = builder_attestation_claimer_storage_key(&a);
        assert_ne!(key, a.0);
        assert_ne!(key, [0u8; 32]);
    }
}
