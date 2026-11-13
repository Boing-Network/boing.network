//! Consensus-ordered liveness evidence (H2): quorum attestations that a scheduled leader missed.
//!
//! Wall-clock burns outside block execution diverged state roots. This path mirrors
//! [`crate::EquivocationEvidence`]: peers collect signed miss attestations, then a
//! [`crate::TransactionPayload::ReportLiveness`] burns stake only during execution.

use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::hash::hasher;
use crate::signature::{Signature, SignatureError};
use crate::types::AccountId;

/// Bincode / payload index for [`crate::TransactionPayload::ReportLiveness`] (append-only after 39).
pub const REPORT_LIVENESS_TX_INDEX: u32 = 40;

/// Domain-separated attestation prefix (BLAKE3 input).
pub const LIVENESS_MISS_ATTESTATION_DOMAIN: &[u8] = b"boing.liveness.miss.v1\0";

/// Env var for the height at which `ReportLiveness` is admitted (unset / `never` / `off` → never).
pub const LIVENESS_SLASH_ACTIVATION_ENV: &str = "BOING_LIVENESS_SLASH_ACTIVATION_HEIGHT";

/// Signable digest: `BLAKE3(domain || height_le || offender || validator_count_le)`.
pub fn liveness_miss_signable_hash(
    height: u64,
    offender: &AccountId,
    validator_count: u32,
) -> [u8; 32] {
    let mut h = hasher();
    h.update(LIVENESS_MISS_ATTESTATION_DOMAIN);
    h.update(&height.to_le_bytes());
    h.update(offender.0.as_slice());
    h.update(&validator_count.to_le_bytes());
    *h.finalize().as_bytes()
}

/// One validator attested that `offender` missed proposing at `height`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LivenessMissAttestation {
    pub height: u64,
    pub offender: AccountId,
    /// Consensus set size observed when attesting (must match across the evidence bundle).
    pub validator_count: u32,
    pub attester: AccountId,
    pub signature: Signature,
}

impl LivenessMissAttestation {
    pub fn sign(
        height: u64,
        offender: AccountId,
        validator_count: u32,
        signing_key: &SigningKey,
    ) -> Self {
        let attester = AccountId(signing_key.verifying_key().to_bytes());
        let msg = liveness_miss_signable_hash(height, &offender, validator_count);
        let sig = signing_key.sign(&msg);
        Self {
            height,
            offender,
            validator_count,
            attester,
            signature: Signature(sig.to_bytes()),
        }
    }

    pub fn verify(&self) -> Result<(), LivenessError> {
        if self.attester == self.offender {
            return Err(LivenessError::SelfAttestation);
        }
        if self.validator_count < 2 {
            return Err(LivenessError::ValidatorCountTooSmall);
        }
        let pk = VerifyingKey::from_bytes(&self.attester.0)
            .map_err(|_| LivenessError::BadSignature(SignatureError::InvalidPublicKey))?;
        let msg = liveness_miss_signable_hash(self.height, &self.offender, self.validator_count);
        let ed_sig = Ed25519Signature::from_bytes(&self.signature.0);
        pk.verify_strict(&msg, &ed_sig)
            .map_err(|_| LivenessError::BadSignature(SignatureError::InvalidSignature))?;
        Ok(())
    }
}

/// Quorum of miss attestations for one `(offender, height)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LivenessEvidence {
    pub height: u64,
    pub offender: AccountId,
    pub validator_count: u32,
    pub attestations: Vec<LivenessMissAttestation>,
}

impl LivenessEvidence {
    /// HotStuff-style quorum size: `2f+1` with `f = (n-1)/3`.
    pub fn quorum_size(validator_count: u32) -> usize {
        let n = validator_count.max(1) as usize;
        let f = (n - 1) / 3;
        2 * f + 1
    }

    pub fn try_from_attestations(
        attestations: Vec<LivenessMissAttestation>,
    ) -> Result<Self, LivenessError> {
        let Some(first) = attestations.first() else {
            return Err(LivenessError::EmptyAttestations);
        };
        let height = first.height;
        let offender = first.offender;
        let validator_count = first.validator_count;
        let need = Self::quorum_size(validator_count);
        if attestations.len() < need {
            return Err(LivenessError::InsufficientAttestations {
                got: attestations.len(),
                need,
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for a in &attestations {
            if a.height != height || a.offender != offender || a.validator_count != validator_count
            {
                return Err(LivenessError::InconsistentAttestation);
            }
            a.verify()?;
            if !seen.insert(a.attester) {
                return Err(LivenessError::DuplicateAttester);
            }
        }
        Ok(Self {
            height,
            offender,
            validator_count,
            attestations,
        })
    }

    pub fn verify(&self) -> Result<(), LivenessError> {
        Self::try_from_attestations(self.attestations.clone()).map(|_| ())
    }

    pub fn validator(&self) -> AccountId {
        self.offender
    }

    pub fn round(&self) -> u64 {
        self.height
    }
}

/// Parse activation height from an env-style string (`None` / empty / `never` / `off` → inactive).
pub fn parse_activation_height(raw: Option<&str>) -> Result<Option<u64>, String> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.eq_ignore_ascii_case("never") || s.eq_ignore_ascii_case("off") {
        return Ok(None);
    }
    s.parse::<u64>()
        .map(Some)
        .map_err(|e| format!("invalid {LIVENESS_SLASH_ACTIVATION_ENV}: {e}"))
}

#[derive(Debug, thiserror::Error)]
pub enum LivenessError {
    #[error("no attestations")]
    EmptyAttestations,
    #[error("need {need} attestations, got {got}")]
    InsufficientAttestations { got: usize, need: usize },
    #[error("attestations disagree on height/offender/validator_count")]
    InconsistentAttestation,
    #[error("duplicate attester")]
    DuplicateAttester,
    #[error("offender cannot attest their own miss")]
    SelfAttestation,
    #[error("validator_count must be >= 2")]
    ValidatorCountTooSmall,
    #[error("bad attestation signature: {0}")]
    BadSignature(SignatureError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn quorum_evidence_verifies() {
        let offender_key = SigningKey::generate(&mut OsRng);
        let offender = AccountId(offender_key.verifying_key().to_bytes());
        let a_key = SigningKey::generate(&mut OsRng);
        let b_key = SigningKey::generate(&mut OsRng);
        let c_key = SigningKey::generate(&mut OsRng);
        // n=4 → f=1 → quorum=3
        let atts = vec![
            LivenessMissAttestation::sign(10, offender, 4, &a_key),
            LivenessMissAttestation::sign(10, offender, 4, &b_key),
            LivenessMissAttestation::sign(10, offender, 4, &c_key),
        ];
        let ev = LivenessEvidence::try_from_attestations(atts).unwrap();
        assert!(ev.verify().is_ok());
        assert_eq!(ev.round(), 10);
    }

    #[test]
    fn dual_validator_quorum_is_one() {
        assert_eq!(LivenessEvidence::quorum_size(2), 1);
        let offender = AccountId([9u8; 32]);
        let attester = SigningKey::generate(&mut OsRng);
        let ev = LivenessEvidence::try_from_attestations(vec![LivenessMissAttestation::sign(
            3, offender, 2, &attester,
        )])
        .unwrap();
        assert!(ev.verify().is_ok());
    }

    #[test]
    fn rejects_self_attestation() {
        let key = SigningKey::generate(&mut OsRng);
        let id = AccountId(key.verifying_key().to_bytes());
        let bad = LivenessMissAttestation::sign(1, id, 2, &key);
        assert!(matches!(bad.verify(), Err(LivenessError::SelfAttestation)));
    }
}
