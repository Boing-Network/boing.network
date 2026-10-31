//! Intent-based transaction format — users declare goals, solvers execute.
//!
//! See ENHANCEMENT-VISION.md for full design.

use serde::{Deserialize, Serialize};

use crate::hash::{hasher, Hash};
use crate::AccountId;

/// Domain tag bound into the signed intent preimage (audit #61 item 35).
pub const INTENT_SIGN_DOMAIN: &[u8] = b"BOING_INTENT_V1";

/// High-level intent kind — what the user wants to achieve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentKind {
    /// Swap token/asset X for Y at best available price (cross-chain).
    Swap {
        from_asset: AccountId,
        to_asset: AccountId,
        amount: u128,
        min_out: Option<u128>,
    },
    /// Transfer amount to recipient.
    Transfer { to: AccountId, amount: u128 },
    /// Custom intent (opaque payload for solver interpretation).
    Custom(Vec<u8>),
}

/// Unsigned intent — declarative goal before solver execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    pub signer: AccountId,
    pub kind: IntentKind,
    pub nonce: u64,
    pub deadline_block: u64,
    /// Optional chain id binding to prevent cross-chain replay (audit #61 item 35).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u64>,
}

impl Intent {
    pub fn id(&self) -> Hash {
        let mut h = hasher();
        let bytes = bincode::serialize(self).unwrap_or_else(|_| {
            // Serialization of Intent is infallible for the fixed schema; empty digest would collide.
            debug_assert!(false, "intent serialize failed");
            Vec::new()
        });
        h.update(&bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        Hash(out)
    }

    /// Preimage digest signed by the intent author (domain + optional chain id + intent body).
    pub fn signable_hash(&self) -> [u8; 32] {
        let body = bincode::serialize(self).expect("intent serialize");
        let mut h = hasher();
        h.update(INTENT_SIGN_DOMAIN);
        match self.chain_id {
            Some(cid) => {
                h.update(b"cid");
                h.update(&cid.to_le_bytes());
            }
            None => {
                h.update(b"cid0");
            }
        }
        h.update(&body);
        *h.finalize().as_bytes()
    }
}

/// Signed intent — user has attested to the goal; solver may execute.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedIntent {
    pub intent: Intent,
    pub signature: crate::signature::Signature,
}

impl SignedIntent {
    /// Human-readable summary for wallet signing UI (intent display).
    pub fn display_for_signing(&self) -> String {
        let kind_str = match &self.intent.kind {
            IntentKind::Swap {
                from_asset,
                to_asset,
                amount,
                min_out,
            } => {
                let min = min_out
                    .map(|m| format!(" (min out: {})", m))
                    .unwrap_or_default();
                format!(
                    "Swap {} from {} to {}{}",
                    amount,
                    hex::encode(&from_asset.0[..8]),
                    hex::encode(&to_asset.0[..8]),
                    min
                )
            }
            IntentKind::Transfer { to, amount } => {
                format!("Transfer {} to {}", amount, hex::encode(&to.0[..8]))
            }
            IntentKind::Custom(_) => "Custom intent".into(),
        };
        let chain = self
            .intent
            .chain_id
            .map(|c| format!(" | Chain: {c}"))
            .unwrap_or_default();
        format!(
            "From: {} | Nonce: {} | Deadline: #{}{} | {}",
            hex::encode(&self.intent.signer.0[..8]),
            self.intent.nonce,
            self.intent.deadline_block,
            chain,
            kind_str
        )
    }

    /// Verify the signature over the intent. Uses `verify_strict` and a domain-tagged preimage.
    pub fn verify(&self) -> Result<(), crate::signature::SignatureError> {
        let msg_hash = self.intent.signable_hash();
        let pk = ed25519_dalek::VerifyingKey::from_bytes(&self.intent.signer.0)
            .map_err(|_| crate::signature::SignatureError::InvalidPublicKey)?;
        let ed_sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        pk.verify_strict(&msg_hash, &ed_sig)
            .map_err(|_| crate::signature::SignatureError::InvalidSignature)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    #[test]
    fn verify_strict_with_chain_id() {
        let key = SigningKey::generate(&mut OsRng);
        let intent = Intent {
            signer: AccountId(key.verifying_key().to_bytes()),
            kind: IntentKind::Custom(vec![1, 2, 3]),
            nonce: 1,
            deadline_block: 10,
            chain_id: Some(6913),
        };
        let digest = intent.signable_hash();
        let sig = key.sign(&digest);
        let signed = SignedIntent {
            intent,
            signature: crate::signature::Signature(sig.to_bytes()),
        };
        signed.verify().unwrap();
        let mut bad = signed;
        bad.intent.chain_id = Some(1);
        assert!(bad.verify().is_err());
    }
}
