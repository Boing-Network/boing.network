//! Ed25519 signatures for transaction authorization.

use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::hash::hasher;
use crate::types::{AccountId, Transaction};

/// 64-byte Ed25519 signature.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Signature(pub [u8; 64]);

impl Serialize for Signature {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bytes: Vec<u8> = serde::de::Deserialize::deserialize(d)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::invalid_length(bytes.len(), &"64"));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Signature(arr))
    }
}

impl Signature {
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 64 {
            return None;
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(bytes);
        Some(Signature(arr))
    }

    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signature({}...)", hex::encode(&self.0[..8]))
    }
}

/// BLAKE3 digest over `nonce_le || sender || bincode(payload) || bincode(access_list)`.
/// Matches what [`sign_transaction`] and [`verify_signature`] use; exposed for SDKs and golden tests.
pub fn signable_transaction_hash(tx: &Transaction) -> [u8; 32] {
    signable_hash(tx)
}

/// Message hash that gets signed (deterministic serialization of tx fields).
fn signable_hash(tx: &Transaction) -> [u8; 32] {
    let mut h = hasher();
    h.update(&tx.nonce.to_le_bytes());
    h.update(tx.sender.0.as_slice());
    h.update(&bincode::serialize(&tx.payload).unwrap_or_default());
    h.update(&bincode::serialize(&tx.access_list).unwrap_or_default());
    *h.finalize().as_bytes()
}

/// Sign a transaction with an Ed25519 signing key.
pub fn sign_transaction(tx: &Transaction, signing_key: &SigningKey) -> Signature {
    let msg = signable_hash(tx);
    let sig = signing_key.sign(&msg);
    Signature(sig.to_bytes())
}

/// Verify a transaction signature. `sender` must be the 32-byte public key (AccountId).
pub fn verify_signature(tx: &Transaction, sig: &Signature, sender: &AccountId) -> Result<(), SignatureError> {
    let pk = VerifyingKey::from_bytes(&sender.0).map_err(|_| SignatureError::InvalidPublicKey)?;
    let msg = signable_hash(tx);
    let ed_sig = Ed25519Signature::from_bytes(&sig.0);
    pk.verify_strict(&msg, &ed_sig).map_err(|_| SignatureError::InvalidSignature)?;
    Ok(())
}

/// Signed transaction — transaction + Ed25519 signature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedTransaction {
    pub tx: Transaction,
    pub signature: Signature,
}

impl SignedTransaction {
    pub fn new(tx: Transaction, signing_key: &SigningKey) -> Self {
        let signature = sign_transaction(&tx, signing_key);
        Self { tx, signature }
    }

    /// Create from a transaction and raw signature (e.g. when deserializing).
    pub fn from_parts(tx: Transaction, signature: Signature) -> Self {
        Self { tx, signature }
    }

    /// Verify the signature. Sender (AccountId) must equal the public key of the signer.
    pub fn verify(&self) -> Result<(), SignatureError> {
        verify_signature(&self.tx, &self.signature, &self.tx.sender)
    }

    /// Human-readable summary for wallet signing UI.
    pub fn display_for_signing(&self) -> String {
        self.tx.display_for_signing()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid signature")]
    InvalidSignature,
}
