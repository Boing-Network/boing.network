//! Ed25519 signatures for transaction authorization.

use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::atomic::{AtomicU64, Ordering};

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
        // Accepts a byte string (bincode, CBOR) or a sequence of numbers (JSON, legacy encoders).
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Signature;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("64 signature bytes")
            }
            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Signature, E> {
                Signature::from_slice(v).ok_or_else(|| E::invalid_length(v.len(), &"64"))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Signature, A::Error> {
                let mut bytes = Vec::with_capacity(64);
                while let Some(b) = seq.next_element::<u8>()? {
                    if bytes.len() == 64 {
                        return Err(serde::de::Error::invalid_length(65, &"64"));
                    }
                    bytes.push(b);
                }
                Signature::from_slice(&bytes)
                    .ok_or_else(|| serde::de::Error::invalid_length(bytes.len(), &"64"))
            }
        }
        d.deserialize_bytes(V)
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

/// Domain tag prefixed to the v2 signing preimage (security review #2: cross-chain replay).
pub const TX_SIGNING_DOMAIN_V2: &[u8] = b"BOING_TX_V2";

/// Domain tag when a non-zero priority tip is bound into the signature (optional fee market).
/// Tip `0` keeps the historical v2 preimage so untipped txs stay compatible.
pub const TX_SIGNING_DOMAIN_PRIORITY: &[u8] = b"BOING_TX_PRIORITY";

/// v2 signing digest: BLAKE3(`"BOING_TX_V2"` || chain_id_le(8) || nonce_le || sender || bincode(payload)
/// || bincode(access_list)). Binds the signature to one chain, so a tx signed for testnet (6913)
/// cannot be replayed on mainnet (6914) or a devnet, and the tag separates tx signatures from any
/// other message signed with the same key.
pub fn signable_transaction_hash_v2(tx: &Transaction, chain_id: u64) -> [u8; 32] {
    let mut h = hasher();
    h.update(TX_SIGNING_DOMAIN_V2);
    h.update(&chain_id.to_le_bytes());
    h.update(&tx.nonce.to_le_bytes());
    h.update(tx.sender.0.as_slice());
    h.update(&bincode::serialize(&tx.payload).unwrap_or_default());
    h.update(&bincode::serialize(&tx.access_list).unwrap_or_default());
    *h.finalize().as_bytes()
}

/// Signable digest including an optional priority tip.
///
/// - `priority_tip == 0` → identical to [`signable_transaction_hash_v2`] (existing wallets keep working).
/// - `priority_tip > 0` → BLAKE3(`"BOING_TX_PRIORITY"` || chain_id || nonce || sender || payload ||
///   access_list || tip_le16).
pub fn signable_transaction_hash_with_tip(
    tx: &Transaction,
    chain_id: u64,
    priority_tip: u128,
) -> [u8; 32] {
    if priority_tip == 0 {
        return signable_transaction_hash_v2(tx, chain_id);
    }
    let mut h = hasher();
    h.update(TX_SIGNING_DOMAIN_PRIORITY);
    h.update(&chain_id.to_le_bytes());
    h.update(&tx.nonce.to_le_bytes());
    h.update(tx.sender.0.as_slice());
    h.update(&bincode::serialize(&tx.payload).unwrap_or_default());
    h.update(&bincode::serialize(&tx.access_list).unwrap_or_default());
    h.update(&priority_tip.to_le_bytes());
    *h.finalize().as_bytes()
}

/// Sign with the v2 (chain-bound) domain.
pub fn sign_transaction_v2(tx: &Transaction, signing_key: &SigningKey, chain_id: u64) -> Signature {
    Signature(
        signing_key
            .sign(&signable_transaction_hash_v2(tx, chain_id))
            .to_bytes(),
    )
}

/// Sign binding a priority tip (uses v2 when tip is 0).
pub fn sign_transaction_with_tip(
    tx: &Transaction,
    signing_key: &SigningKey,
    chain_id: u64,
    priority_tip: u128,
) -> Signature {
    Signature(
        signing_key
            .sign(&signable_transaction_hash_with_tip(
                tx,
                chain_id,
                priority_tip,
            ))
            .to_bytes(),
    )
}

/// Which signature formats a verifier accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TxSigPolicy {
    /// Chain id for v2 signatures; `None` = v2 cannot be checked (unknown chain), legacy only.
    pub chain_id: Option<u64>,
    /// After the activation height: legacy (domain-less) signatures are rejected.
    pub require_v2: bool,
}

/// Verify under `policy`: a v2 signature for `policy.chain_id` is always accepted; a legacy one only
/// while `!policy.require_v2`. Tip must be `0` (use [`verify_signature_with_tip`] for tipped txs).
pub fn verify_signature_with(
    tx: &Transaction,
    sig: &Signature,
    sender: &AccountId,
    policy: TxSigPolicy,
) -> Result<(), SignatureError> {
    verify_signature_with_tip(tx, sig, sender, policy, 0)
}

/// Like [`verify_signature_with`], but accepts a priority tip bound into the signature when non-zero.
pub fn verify_signature_with_tip(
    tx: &Transaction,
    sig: &Signature,
    sender: &AccountId,
    policy: TxSigPolicy,
    priority_tip: u128,
) -> Result<(), SignatureError> {
    let pk = VerifyingKey::from_bytes(&sender.0).map_err(|_| SignatureError::InvalidPublicKey)?;
    let ed_sig = Ed25519Signature::from_bytes(&sig.0);
    if let Some(cid) = policy.chain_id {
        // `with_tip(..., 0)` is identical to the plain v2 digest — one verify covers both (issue #61 item 34).
        if pk
            .verify_strict(
                &signable_transaction_hash_with_tip(tx, cid, priority_tip),
                &ed_sig,
            )
            .is_ok()
        {
            return Ok(());
        }
    }
    if priority_tip > 0 {
        // Tips require a chain-bound priority signature; never fall back to legacy.
        return Err(SignatureError::InvalidSignature);
    }
    if policy.require_v2 {
        return Err(if pk.verify_strict(&signable_hash(tx), &ed_sig).is_ok() {
            SignatureError::LegacySignatureRejected
        } else {
            SignatureError::InvalidSignature
        });
    }
    pk.verify_strict(&signable_hash(tx), &ed_sig)
        .map_err(|_| SignatureError::InvalidSignature)
}

// Process-wide policy (one chain per node / CLI process), packed into a single atomic so
// `chain_id` and `require_v2` cannot tear across concurrent readers/writers (issue #61 item 34).
// Layout: bit 63 = require_v2; bits 0..62 = chain_id (0 = unset).
const VERIFY_REQUIRE_V2_BIT: u64 = 1 << 63;
const VERIFY_CHAIN_ID_MASK: u64 = VERIFY_REQUIRE_V2_BIT - 1;
static VERIFY_POLICY_PACKED: AtomicU64 = AtomicU64::new(0);
static SIGNING_CHAIN_ID: AtomicU64 = AtomicU64::new(0);

fn pack_tx_verify_policy(policy: TxSigPolicy) -> u64 {
    let mut v = policy.chain_id.unwrap_or(0) & VERIFY_CHAIN_ID_MASK;
    if policy.require_v2 {
        v |= VERIFY_REQUIRE_V2_BIT;
    }
    v
}

fn unpack_tx_verify_policy(v: u64) -> TxSigPolicy {
    let c = v & VERIFY_CHAIN_ID_MASK;
    TxSigPolicy {
        chain_id: (c != 0).then_some(c),
        require_v2: (v & VERIFY_REQUIRE_V2_BIT) != 0,
    }
}

/// Set the policy used by [`SignedTransaction::verify`] (the node updates it as the chain grows).
///
/// Prefer passing an explicit [`TxSigPolicy`] into [`verify_signature_with`] /
/// [`SignedTransaction::verify_with`] on admission paths so height-boundary checks are not
/// TOCTOU against this process-wide value.
pub fn set_tx_verify_policy(policy: TxSigPolicy) {
    VERIFY_POLICY_PACKED.store(pack_tx_verify_policy(policy), Ordering::SeqCst);
}

/// Current process-wide verification policy (default: legacy only).
pub fn tx_verify_policy() -> TxSigPolicy {
    unpack_tx_verify_policy(VERIFY_POLICY_PACKED.load(Ordering::SeqCst))
}

/// Chain id [`SignedTransaction::new`] signs for (`None` = legacy format; CLI sets it from the node).
pub fn set_default_signing_chain_id(chain_id: Option<u64>) {
    SIGNING_CHAIN_ID.store(chain_id.unwrap_or(0), Ordering::SeqCst);
}

pub fn default_signing_chain_id() -> Option<u64> {
    let c = SIGNING_CHAIN_ID.load(Ordering::SeqCst);
    (c != 0).then_some(c)
}

/// Sign a transaction with an Ed25519 signing key (legacy, domain-less format).
pub fn sign_transaction(tx: &Transaction, signing_key: &SigningKey) -> Signature {
    let msg = signable_hash(tx);
    let sig = signing_key.sign(&msg);
    Signature(sig.to_bytes())
}

/// Verify a transaction signature. `sender` must be the 32-byte public key (AccountId).
pub fn verify_signature(
    tx: &Transaction,
    sig: &Signature,
    sender: &AccountId,
) -> Result<(), SignatureError> {
    let pk = VerifyingKey::from_bytes(&sender.0).map_err(|_| SignatureError::InvalidPublicKey)?;
    let msg = signable_hash(tx);
    let ed_sig = Ed25519Signature::from_bytes(&sig.0);
    pk.verify_strict(&msg, &ed_sig)
        .map_err(|_| SignatureError::InvalidSignature)?;
    Ok(())
}

fn is_zero_u128(v: &u128) -> bool {
    *v == 0
}

/// Signed transaction — transaction + Ed25519 signature (+ optional priority tip).
///
/// `priority_tip` is omitted from the wire encoding when zero so historical
/// `(tx, signature)` bincode/JSON stay byte-identical. Use [`SignedTransaction::from_bincode`]
/// when decoding untrusted bytes (RPC submit).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedTransaction {
    pub tx: Transaction,
    pub signature: Signature,
    /// Optional priority tip in ledger units (paid to the block proposer on inclusion).
    #[serde(default, skip_serializing_if = "is_zero_u128")]
    pub priority_tip: u128,
}

#[derive(Deserialize)]
struct LegacySignedTransaction {
    tx: Transaction,
    signature: Signature,
}

impl SignedTransaction {
    /// Signs for the process default chain (v2) when one is set, otherwise the legacy format.
    pub fn new(tx: Transaction, signing_key: &SigningKey) -> Self {
        match default_signing_chain_id() {
            Some(cid) => Self::new_for_chain(tx, signing_key, cid),
            None => Self::new_legacy(tx, signing_key),
        }
    }

    /// v2 (chain-bound) signature with tip `0`.
    pub fn new_for_chain(tx: Transaction, signing_key: &SigningKey, chain_id: u64) -> Self {
        Self::new_for_chain_with_tip(tx, signing_key, chain_id, 0)
    }

    /// Chain-bound signature that commits to `priority_tip` when non-zero.
    pub fn new_for_chain_with_tip(
        tx: Transaction,
        signing_key: &SigningKey,
        chain_id: u64,
        priority_tip: u128,
    ) -> Self {
        let signature = sign_transaction_with_tip(&tx, signing_key, chain_id, priority_tip);
        Self {
            tx,
            signature,
            priority_tip,
        }
    }

    /// Legacy (domain-less) signature; rejected by nodes after the v2 activation height.
    /// Tips are not supported on the legacy path (`priority_tip` must stay 0).
    pub fn new_legacy(tx: Transaction, signing_key: &SigningKey) -> Self {
        let signature = sign_transaction(&tx, signing_key);
        Self {
            tx,
            signature,
            priority_tip: 0,
        }
    }

    /// Create from a transaction and raw signature (e.g. when deserializing). Tip defaults to 0.
    pub fn from_parts(tx: Transaction, signature: Signature) -> Self {
        Self {
            tx,
            signature,
            priority_tip: 0,
        }
    }

    pub fn from_parts_with_tip(tx: Transaction, signature: Signature, priority_tip: u128) -> Self {
        Self {
            tx,
            signature,
            priority_tip,
        }
    }

    /// Decode RPC / gossip bytes: tipped encoding or historical `(tx, signature)` only.
    pub fn from_bincode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        match bincode::deserialize::<SignedTransaction>(bytes) {
            Ok(s) => Ok(s),
            Err(e) => bincode::deserialize::<LegacySignedTransaction>(bytes)
                .map(|l| Self {
                    tx: l.tx,
                    signature: l.signature,
                    priority_tip: 0,
                })
                .map_err(|_| e),
        }
    }

    /// Verify the signature. Sender (AccountId) must equal the public key of the signer.
    /// Uses the process-wide [`tx_verify_policy`].
    pub fn verify(&self) -> Result<(), SignatureError> {
        self.verify_with(tx_verify_policy())
    }

    pub fn verify_with(&self, policy: TxSigPolicy) -> Result<(), SignatureError> {
        verify_signature_with_tip(
            &self.tx,
            &self.signature,
            &self.tx.sender,
            policy,
            self.priority_tip,
        )
    }

    /// Human-readable summary for wallet signing UI.
    pub fn display_for_signing(&self) -> String {
        if self.priority_tip == 0 {
            self.tx.display_for_signing()
        } else {
            format!(
                "{} (priority tip {})",
                self.tx.display_for_signing(),
                self.priority_tip
            )
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Legacy (pre-chain-id) signature rejected after the v2 activation height; re-sign with the chain id")]
    LegacySignatureRejected,
}

#[cfg(test)]
mod v2_tests {
    use super::*;
    use crate::types::{AccessList, TransactionPayload};

    fn tx(key: &SigningKey) -> Transaction {
        Transaction {
            nonce: 3,
            sender: AccountId(key.verifying_key().to_bytes()),
            payload: TransactionPayload::Transfer {
                to: AccountId([9u8; 32]),
                amount: 5,
            },
            access_list: AccessList::default(),
        }
    }

    #[test]
    fn cross_chain_replay_is_rejected() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let t = tx(&key);
        let testnet = SignedTransaction::new_for_chain(t.clone(), &key, 6913);
        let p = |c, r| TxSigPolicy {
            chain_id: Some(c),
            require_v2: r,
        };
        assert!(testnet.verify_with(p(6913, true)).is_ok());
        assert!(testnet.verify_with(p(6913, false)).is_ok());
        // same bytes submitted to mainnet (6914): rejected before and after activation
        assert!(testnet.verify_with(p(6914, false)).is_err());
        assert!(testnet.verify_with(p(6914, true)).is_err());
    }

    /// Regression: `priority_tip` is skipped on the wire when zero, which plain
    /// `bincode::deserialize::<SignedTransaction>` cannot read back (UnexpectedEof).
    /// Every decoder of untrusted/stored bytes (RPC, gossip, QA pool, mempool) must use
    /// `SignedTransaction::from_bincode`.
    #[test]
    fn from_bincode_roundtrips_zero_and_nonzero_tip() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let zero = SignedTransaction::new_for_chain(tx(&key), &key, 6913);
        let zero_bytes = bincode::serialize(&zero).unwrap();
        assert_eq!(SignedTransaction::from_bincode(&zero_bytes).unwrap(), zero);
        // Zero tip keeps the historical (tx, signature) encoding: exactly 16 bytes shorter.
        let tipped = SignedTransaction::new_for_chain_with_tip(tx(&key), &key, 6913, 42);
        let tipped_bytes = bincode::serialize(&tipped).unwrap();
        assert_eq!(tipped_bytes.len(), zero_bytes.len() + 16);
        let back = SignedTransaction::from_bincode(&tipped_bytes).unwrap();
        assert_eq!(back, tipped);
        assert_eq!(back.priority_tip, 42);
        assert!(back
            .verify_with(TxSigPolicy {
                chain_id: Some(6913),
                require_v2: true
            })
            .is_ok());
        // Truncated / empty input is an error, never a panic.
        assert!(SignedTransaction::from_bincode(&zero_bytes[..zero_bytes.len() - 1]).is_err());
        assert!(SignedTransaction::from_bincode(&[]).is_err());
    }

    #[test]
    fn legacy_accepted_only_before_activation() {
        let key = SigningKey::from_bytes(&[8u8; 32]);
        let legacy = SignedTransaction::new_legacy(tx(&key), &key);
        assert!(legacy
            .verify_with(TxSigPolicy {
                chain_id: Some(6913),
                require_v2: false
            })
            .is_ok());
        assert!(matches!(
            legacy.verify_with(TxSigPolicy {
                chain_id: Some(6913),
                require_v2: true
            }),
            Err(SignatureError::LegacySignatureRejected)
        ));
        assert!(legacy.verify_with(TxSigPolicy::default()).is_ok());
    }

    /// Golden vector shared with boing-sdk (`tests/txSigningV2.test.ts`) and boing.express.
    #[test]
    fn v2_golden_vector() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        assert_eq!(
            hex::encode(signable_transaction_hash_v2(&tx(&key), 6913)),
            "d88a23702aa0c376d9e0f3b6a2c2948058fecda14b95d26843f5f902ece87899"
        );
    }

    #[test]
    fn v2_digest_differs_from_legacy_and_by_chain() {
        let key = SigningKey::from_bytes(&[1u8; 32]);
        let t = tx(&key);
        assert_ne!(
            signable_transaction_hash(&t),
            signable_transaction_hash_v2(&t, 6913)
        );
        assert_ne!(
            signable_transaction_hash_v2(&t, 6913),
            signable_transaction_hash_v2(&t, 6914)
        );
    }

    #[test]
    fn packed_process_wide_policy_roundtrips() {
        let cases = [
            TxSigPolicy::default(),
            TxSigPolicy {
                chain_id: Some(6913),
                require_v2: false,
            },
            TxSigPolicy {
                chain_id: Some(6914),
                require_v2: true,
            },
            TxSigPolicy {
                chain_id: None,
                require_v2: true,
            },
        ];
        for p in cases {
            assert_eq!(unpack_tx_verify_policy(pack_tx_verify_policy(p)), p);
        }
        let prev = tx_verify_policy();
        set_tx_verify_policy(TxSigPolicy {
            chain_id: Some(6913),
            require_v2: true,
        });
        assert_eq!(
            tx_verify_policy(),
            TxSigPolicy {
                chain_id: Some(6913),
                require_v2: true,
            }
        );
        set_tx_verify_policy(prev);
    }

    #[test]
    fn tip_zero_v2_accepts_without_legacy_fallback() {
        let key = SigningKey::from_bytes(&[9u8; 32]);
        let signed = SignedTransaction::new_for_chain(tx(&key), &key, 6913);
        assert_eq!(signed.priority_tip, 0);
        assert!(signed
            .verify_with(TxSigPolicy {
                chain_id: Some(6913),
                require_v2: true,
            })
            .is_ok());
    }
}
