//! Cryptographic verification for automation execution.
//!
//! See [DEVELOPMENT-AND-ENHANCEMENTS.md](../../../docs/DEVELOPMENT-AND-ENHANCEMENTS.md#appendix-cryptographic-verification-for-decentralized-automation).

use boing_primitives::{hasher, signature::Signature, AccountId, Hash};
use ed25519_dalek::{Signature as Ed25519Sig, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Proof that an automation task was executed correctly.
/// Supports on-chain (attestation) and off-chain (ZKP, fraud proof) verification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExecutionProof {
    /// Executor attestation: signed execution report.
    Attestation(ExecutorAttestation),
    /// Zero-knowledge proof (placeholder; full ZKP integration TBD).
    Zkp(ZkpProof),
    /// Fraud proof: evidence of incorrect execution (for optimistic verification).
    FraudProof(FraudProof),
}

/// Executor attestation — cryptographically signed execution report.
/// Executor signs (task_id, result_hash, timestamp); protocol verifies and may slash on fraud.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutorAttestation {
    pub executor: AccountId,
    pub task_id: u64,
    pub result_hash: Hash,
    pub timestamp_secs: u64,
    pub signature: Signature,
}

impl ExecutorAttestation {
    /// Message that gets signed: hash of (executor, task_id, result_hash, timestamp).
    fn signable_hash(&self) -> [u8; 32] {
        let mut h = hasher();
        h.update(&self.executor.0);
        h.update(&self.task_id.to_le_bytes());
        h.update(self.result_hash.as_bytes());
        h.update(&self.timestamp_secs.to_le_bytes());
        *h.finalize().as_bytes()
    }

    /// Create and sign an attestation.
    pub fn new(
        executor: AccountId,
        task_id: u64,
        result_hash: Hash,
        timestamp_secs: u64,
        signing_key: &SigningKey,
    ) -> Self {
        let att = Self {
            executor,
            task_id,
            result_hash,
            timestamp_secs,
            signature: Signature([0u8; 64]),
        };
        let msg = att.signable_hash();
        let sig = signing_key.sign(&msg);
        Self {
            signature: Signature(sig.to_bytes()),
            ..att
        }
    }

    /// Verify the attestation signature.
    pub fn verify(&self) -> Result<(), VerificationError> {
        let pk = VerifyingKey::from_bytes(&self.executor.0)
            .map_err(|_| VerificationError::InvalidPublicKey)?;
        let msg = self.signable_hash();
        let sig = Ed25519Sig::from_bytes(&self.signature.0);
        pk.verify(&msg, &sig).map_err(|_| VerificationError::InvalidSignature)?;
        Ok(())
    }
}

/// Placeholder for ZKP verification (e.g. SNARK/STARK proof bytes).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ZkpProof {
    pub proof_bytes: Vec<u8>,
}

/// Fraud proof — evidence that an Executor submitted incorrect results.
/// Submitted during challenge period; triggers slashing if valid.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FraudProof {
    pub task_id: u64,
    pub attestation_id: AccountId,
    pub expected_result_hash: Hash,
    pub actual_result_hash: Hash,
    pub evidence: Vec<u8>,
}

/// Oracle attestation — external data signed by oracle nodes.
/// Used for conditions like "when token X hits price Y".
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleAttestation {
    pub data_hash: Hash,
    pub oracle_signatures: Vec<(AccountId, Signature)>,
    pub quorum: usize,
}

impl OracleAttestation {
    /// Check if we have at least quorum valid signatures.
    pub fn has_quorum(&self) -> bool {
        self.oracle_signatures.len() >= self.quorum
    }
}

#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid signature")]
    InvalidSignature,
}
