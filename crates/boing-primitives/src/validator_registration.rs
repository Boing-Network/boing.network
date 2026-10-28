//! Open validator registration: proof that the node (consensus) key consents to being bonded by `owner`.
//!
//! The registering transaction is signed by `owner` (who pays the bond). The payload carries
//! `node_signature`, an Ed25519 signature by the **node key** over
//! [`validator_registration_message`]`(owner, node_id)`, so a node key can only be registered by
//! the owner it signed for and nobody can register someone else's node.

use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey};

use crate::hash::hasher;
use crate::types::AccountId;

/// Domain separator for the node-key proof of possession.
pub const VALIDATOR_REGISTRATION_DOMAIN: &[u8] = b"BOING_OPEN_VALIDATOR_REGISTER_V1";

/// BLAKE3(`domain || owner || node_id`) — the 32-byte message the node key signs.
pub fn validator_registration_message(owner: &AccountId, node_id: &AccountId) -> [u8; 32] {
    let mut h = hasher();
    h.update(VALIDATOR_REGISTRATION_DOMAIN);
    h.update(&owner.0);
    h.update(&node_id.0);
    *h.finalize().as_bytes()
}

/// Domain of the chain-bound (v2) proof (security review #14).
pub const VALIDATOR_REGISTRATION_DOMAIN_V2: &[u8] = b"BOING_OPEN_VALIDATOR_REGISTER_V2";

/// BLAKE3(`domain_v2 || chain_id_le8 || owner || node_id`): the v2 message, bound to one chain so a
/// proof made for testnet cannot be replayed on another network.
pub fn validator_registration_message_v2(
    owner: &AccountId,
    node_id: &AccountId,
    chain_id: u64,
) -> [u8; 32] {
    let mut h = hasher();
    h.update(VALIDATOR_REGISTRATION_DOMAIN_V2);
    h.update(&chain_id.to_le_bytes());
    h.update(&owner.0);
    h.update(&node_id.0);
    *h.finalize().as_bytes()
}

/// Sign the v2 (chain-bound) registration proof with the node's consensus key.
pub fn sign_validator_registration_v2(
    owner: &AccountId,
    node_key: &SigningKey,
    chain_id: u64,
) -> Vec<u8> {
    let node_id = AccountId(node_key.verifying_key().to_bytes());
    node_key
        .sign(&validator_registration_message_v2(
            owner, &node_id, chain_id,
        ))
        .to_bytes()
        .to_vec()
}

/// Verify a v2 proof for `chain_id`.
pub fn verify_validator_registration_v2(
    owner: &AccountId,
    node_id: &AccountId,
    node_signature: &[u8],
    chain_id: u64,
) -> bool {
    verify_msg(
        node_id,
        node_signature,
        &validator_registration_message_v2(owner, node_id, chain_id),
    )
}

fn verify_msg(node_id: &AccountId, node_signature: &[u8], msg: &[u8; 32]) -> bool {
    if node_signature.len() != 64 {
        return false;
    }
    let Ok(pk) = VerifyingKey::from_bytes(&node_id.0) else {
        return false;
    };
    let mut arr = [0u8; 64];
    arr.copy_from_slice(node_signature);
    pk.verify_strict(msg, &Ed25519Signature::from_bytes(&arr))
        .is_ok()
}

/// Sign the registration proof with the node's consensus key.
pub fn sign_validator_registration(owner: &AccountId, node_key: &SigningKey) -> Vec<u8> {
    let node_id = AccountId(node_key.verifying_key().to_bytes());
    let msg = validator_registration_message(owner, &node_id);
    node_key.sign(&msg).to_bytes().to_vec()
}

/// Verify `node_signature` was produced by `node_id` for `owner`.
pub fn verify_validator_registration(
    owner: &AccountId,
    node_id: &AccountId,
    node_signature: &[u8],
) -> bool {
    if node_signature.len() != 64 {
        return false;
    }
    let Ok(pk) = VerifyingKey::from_bytes(&node_id.0) else {
        return false;
    };
    let mut arr = [0u8; 64];
    arr.copy_from_slice(node_signature);
    let sig = Ed25519Signature::from_bytes(&arr);
    pk.verify_strict(&validator_registration_message(owner, node_id), &sig)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_roundtrip_and_binding() {
        let node = SigningKey::from_bytes(&[7u8; 32]);
        let node_id = AccountId(node.verifying_key().to_bytes());
        let owner = AccountId([9u8; 32]);
        let other = AccountId([8u8; 32]);
        let sig = sign_validator_registration(&owner, &node);
        assert!(verify_validator_registration(&owner, &node_id, &sig));
        // Bound to owner: replaying for another owner fails.
        assert!(!verify_validator_registration(&other, &node_id, &sig));
        // Wrong node key fails.
        let fake = SigningKey::from_bytes(&[6u8; 32]);
        let fake_sig = sign_validator_registration(&owner, &fake);
        assert!(!verify_validator_registration(&owner, &node_id, &fake_sig));
        assert!(!verify_validator_registration(&owner, &node_id, &sig[..63]));
    }

    #[test]
    fn v2_proof_is_chain_bound() {
        let node = SigningKey::from_bytes(&[7u8; 32]);
        let node_id = AccountId(node.verifying_key().to_bytes());
        let owner = AccountId([9u8; 32]);
        let sig = sign_validator_registration_v2(&owner, &node, 6913);
        assert!(verify_validator_registration_v2(
            &owner, &node_id, &sig, 6913
        ));
        assert!(
            !verify_validator_registration_v2(&owner, &node_id, &sig, 6914),
            "other chain"
        );
        assert!(
            !verify_validator_registration(&owner, &node_id, &sig),
            "not a v1 proof"
        );
        let v1 = sign_validator_registration(&owner, &node);
        assert!(
            !verify_validator_registration_v2(&owner, &node_id, &v1, 6913),
            "v1 is not v2"
        );
        // Golden vector shared with the SDK and Boing Express.
        assert_eq!(
            hex::encode(validator_registration_message_v2(&owner, &node_id, 6913)),
            GOLDEN_V2
        );
    }

    const GOLDEN_V2: &str = "2d9b960878d673d6e84ded770590a0242fdc66ac4f34ca9d158078c7bf4b59d9";

    /// Golden vector shared with `boing-sdk/tests/boingOpenValidator.test.ts`.
    #[test]
    fn golden_message() {
        let node = SigningKey::from_bytes(&[7u8; 32]);
        let node_id = AccountId(node.verifying_key().to_bytes());
        assert_eq!(
            hex::encode(node_id.0),
            "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c"
        );
        assert_eq!(
            hex::encode(validator_registration_message(
                &AccountId([9u8; 32]),
                &node_id
            )),
            "f8bbe9cbc961743405a6154a8dd07192efa73a3e9368271ec4b11dd0b66562b5"
        );
    }
}
