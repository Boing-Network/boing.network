//! Intent-based transaction format — users declare goals, solvers execute.
//!
//! See ENHANCEMENT-VISION.md for full design.

use serde::{Deserialize, Serialize};

use crate::hash::{hasher, Hash};
use crate::AccountId;

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
}

impl Intent {
    pub fn id(&self) -> Hash {
        let mut h = hasher();
        h.update(&bincode::serialize(self).unwrap_or_default());
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        Hash(out)
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
        format!(
            "From: {} | Nonce: {} | Deadline: #{} | {}",
            hex::encode(&self.intent.signer.0[..8]),
            self.intent.nonce,
            self.intent.deadline_block,
            kind_str
        )
    }

    /// Verify the signature over the intent. Uses signer from intent.
    pub fn verify(&self) -> Result<(), crate::signature::SignatureError> {
        let msg = bincode::serialize(&self.intent)
            .map_err(|_| crate::signature::SignatureError::InvalidSignature)?;
        let msg_hash = {
            let mut h = hasher();
            h.update(&msg);
            h.finalize()
        };
        use ed25519_dalek::Verifier;
        let pk = ed25519_dalek::VerifyingKey::from_bytes(&self.intent.signer.0)
            .map_err(|_| crate::signature::SignatureError::InvalidPublicKey)?;
        let ed_sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        pk.verify(msg_hash.as_bytes(), &ed_sig)
            .map_err(|_| crate::signature::SignatureError::InvalidSignature)?;
        Ok(())
    }
}
