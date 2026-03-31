//! Reference **NFT** calldata layout (Boing-defined) for wallets and indexers.
//!
//! Not consensus-enforced — see `docs/BOING-REFERENCE-NFT.md`.

use boing_primitives::AccountId;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_nft_calldata_lengths() {
        let tid = [1u8; 32];
        let to = AccountId([2u8; 32]);
        let h = [3u8; 32];
        assert_eq!(encode_owner_of_calldata(&tid).len(), 96);
        assert_eq!(encode_transfer_nft_calldata(&to, &tid).len(), 96);
        assert_eq!(encode_set_metadata_hash_calldata(&tid, &h).len(), 96);
    }
}
