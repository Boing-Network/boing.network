//! Inventory-free Boing ↔ Base bridge (testnet-first).
//!
//! Wire indices (append-only after [`crate::TransactionPayload::ReportLiveness`] = 40):
//! - [`BRIDGE_LOCK_TO_BASE_TX_INDEX`] = 41 — lock native BOING for mint on Base
//! - [`BRIDGE_CLAIM_FROM_BASE_TX_INDEX`] = 42 — claim unlock after Base burn attestation
//!
//! Activation: `BOING_BRIDGE_ACTIVATION_HEIGHT` unset / `never` / `off` → never admitted.
//! Stake quorum: ⌈2W/3⌉ of bonded stake among consensus validators (same integer compare as
//! Phase 17 slash-appeal resolve).

use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::blobs_phase17::{
    phase17_resolve_threshold_met, PHASE17_RESOLVE_THRESHOLD_DEN, PHASE17_RESOLVE_THRESHOLD_NUM,
};
use crate::hash::hasher;
use crate::signature::{Signature, SignatureError};
use crate::types::AccountId;
use crate::Hash;

/// Bincode / payload index for [`crate::TransactionPayload::BridgeLockToBase`].
pub const BRIDGE_LOCK_TO_BASE_TX_INDEX: u32 = 41;
/// Bincode / payload index for [`crate::TransactionPayload::BridgeClaimFromBase`].
pub const BRIDGE_CLAIM_FROM_BASE_TX_INDEX: u32 = 42;

/// Env var for the height at which bridge txs are admitted (`None` / empty / `never` / `off` → off).
pub const BRIDGE_ACTIVATION_ENV: &str = "BOING_BRIDGE_ACTIVATION_HEIGHT";

/// Platform fee in basis points (50 = 0.5%).
pub const BRIDGE_FEE_BPS: u16 = 50;
pub const BRIDGE_BPS_DENOM: u16 = 10_000;

/// Base Sepolia chain id (destination for testnet locks).
pub const BASE_SEPOLIA_CHAIN_ID: u64 = 84532;
/// Base mainnet (not enabled by default UI).
pub const BASE_MAINNET_CHAIN_ID: u64 = 8453;

/// Domain for lock → Base mint attestations (Ed25519 on Boing; ECDSA digest on Base vault).
pub const BRIDGE_LOCK_ATTEST_DOMAIN: &[u8] = b"boing.bridge.lock.v1\0";
/// Domain for Base burn → Boing claim attestations.
pub const BRIDGE_BURN_ATTEST_DOMAIN: &[u8] = b"boing.bridge.burn.v1\0";

/// System escrow that holds locked native BOING backing bridged supply.
pub const BRIDGE_ESCROW_DOMAIN: &[u8] = b"BOING_BRIDGE_ESCROW_V1";

#[must_use]
pub fn bridge_escrow_account() -> AccountId {
    let mut h = hasher();
    h.update(BRIDGE_ESCROW_DOMAIN);
    AccountId(*h.finalize().as_bytes())
}

/// Parse activation height (`None` / empty / `never` / `off` → inactive).
pub fn parse_bridge_activation_height(raw: Option<&str>) -> Result<Option<u64>, String> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.eq_ignore_ascii_case("never") || s.eq_ignore_ascii_case("off") {
        return Ok(None);
    }
    s.parse::<u64>()
        .map(Some)
        .map_err(|e| format!("invalid {BRIDGE_ACTIVATION_ENV}: {e}"))
}

#[must_use]
pub fn bridge_fee_split(amount: u128) -> (u128, u128) {
    let fee = amount.saturating_mul(BRIDGE_FEE_BPS as u128) / BRIDGE_BPS_DENOM as u128;
    (amount.saturating_sub(fee), fee)
}

/// Signable digest for a Boing lock (validators sign this for Base mint).
#[must_use]
pub fn bridge_lock_signable_hash(
    source_chain_id: u64,
    source_tx_hash: &Hash,
    lock_nonce: u64,
    dest_chain_id: u64,
    dest_recipient: &[u8; 20],
    amount: u128,
) -> [u8; 32] {
    let mut h = hasher();
    h.update(BRIDGE_LOCK_ATTEST_DOMAIN);
    h.update(&source_chain_id.to_le_bytes());
    h.update(source_tx_hash.0.as_slice());
    h.update(&lock_nonce.to_le_bytes());
    h.update(&dest_chain_id.to_le_bytes());
    h.update(dest_recipient.as_slice());
    h.update(&amount.to_le_bytes());
    *h.finalize().as_bytes()
}

/// Signable digest for a Base burn claim on Boing.
#[must_use]
pub fn bridge_burn_signable_hash(
    source_chain_id: u64,
    burn_id: &[u8; 32],
    burn_nonce: u64,
    recipient: &AccountId,
    amount: u128,
) -> [u8; 32] {
    let mut h = hasher();
    h.update(BRIDGE_BURN_ATTEST_DOMAIN);
    h.update(&source_chain_id.to_le_bytes());
    h.update(burn_id.as_slice());
    h.update(&burn_nonce.to_le_bytes());
    h.update(recipient.0.as_slice());
    h.update(&amount.to_le_bytes());
    *h.finalize().as_bytes()
}

/// One validator attestation (Ed25519) over a bridge digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeAttestation {
    pub attester: AccountId,
    pub signature: Signature,
}

impl BridgeAttestation {
    pub fn sign(digest: &[u8; 32], signing_key: &SigningKey) -> Self {
        let attester = AccountId(signing_key.verifying_key().to_bytes());
        let sig = signing_key.sign(digest);
        Self {
            attester,
            signature: Signature(sig.to_bytes()),
        }
    }

    pub fn verify(&self, digest: &[u8; 32]) -> Result<(), BridgeError> {
        let pk = VerifyingKey::from_bytes(&self.attester.0)
            .map_err(|_| BridgeError::BadSignature(SignatureError::InvalidPublicKey))?;
        let ed_sig = Ed25519Signature::from_bytes(&self.signature.0);
        pk.verify_strict(digest, &ed_sig)
            .map_err(|_| BridgeError::BadSignature(SignatureError::InvalidSignature))?;
        Ok(())
    }
}

/// Stake-weighted attestation bundle (≥ ⌈2W/3⌉ of `electorate_stake`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeAttestationEvidence {
    pub digest: [u8; 32],
    pub attestations: Vec<BridgeAttestation>,
}

impl BridgeAttestationEvidence {
    /// Verify signatures and that signed stake meets ⌈2W/3⌉ of `electorate`.
    /// `electorate` is `(validator_id, bonded_stake)` for the current consensus set.
    pub fn verify_stake_quorum(
        &self,
        electorate: &[(AccountId, u128)],
    ) -> Result<u128, BridgeError> {
        if self.attestations.is_empty() {
            return Err(BridgeError::EmptyAttestations);
        }
        let mut stake_by_id = std::collections::BTreeMap::new();
        let mut total = 0u128;
        for (id, stake) in electorate {
            if *stake == 0 {
                continue;
            }
            stake_by_id.insert(*id, *stake);
            total = total.saturating_add(*stake);
        }
        if total == 0 {
            return Err(BridgeError::EmptyElectorate);
        }

        let mut seen = std::collections::BTreeSet::new();
        let mut signed = 0u128;
        for a in &self.attestations {
            if !seen.insert(a.attester) {
                return Err(BridgeError::DuplicateAttester);
            }
            a.verify(&self.digest)?;
            let Some(w) = stake_by_id.get(&a.attester).copied() else {
                return Err(BridgeError::AttesterNotInSet);
            };
            signed = signed.saturating_add(w);
        }

        if !phase17_resolve_threshold_met(signed, total) {
            return Err(BridgeError::QuorumNotMet {
                signed,
                total,
                num: PHASE17_RESOLVE_THRESHOLD_NUM,
                den: PHASE17_RESOLVE_THRESHOLD_DEN,
            });
        }
        Ok(signed)
    }
}

/// Storage key helpers (under [`bridge_escrow_account`] contract storage).
#[must_use]
pub fn bridge_claim_consumed_key(claim_id: &[u8; 32]) -> [u8; 32] {
    let mut h = hasher();
    h.update(b"BOING_BRIDGE_CLAIM_USED_V1");
    h.update(claim_id.as_slice());
    *h.finalize().as_bytes()
}

#[must_use]
pub fn bridge_lock_nonce_key(user: &AccountId) -> [u8; 32] {
    let mut h = hasher();
    h.update(b"BOING_BRIDGE_LOCK_NONCE_V1");
    h.update(user.0.as_slice());
    *h.finalize().as_bytes()
}

#[must_use]
pub fn u64_from_storage_word(word: &[u8; 32]) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&word[..8]);
    u64::from_le_bytes(b)
}

#[must_use]
pub fn u64_to_storage_word(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[..8].copy_from_slice(&n.to_le_bytes());
    w
}

#[must_use]
pub fn claim_id_from_burn(source_chain_id: u64, burn_id: &[u8; 32], burn_nonce: u64) -> [u8; 32] {
    let mut h = hasher();
    h.update(b"BOING_BRIDGE_CLAIM_ID_V1");
    h.update(&source_chain_id.to_le_bytes());
    h.update(burn_id.as_slice());
    h.update(&burn_nonce.to_le_bytes());
    *h.finalize().as_bytes()
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("no attestations")]
    EmptyAttestations,
    #[error("empty electorate")]
    EmptyElectorate,
    #[error("duplicate attester")]
    DuplicateAttester,
    #[error("attester not in validator set")]
    AttesterNotInSet,
    #[error("stake quorum not met: signed={signed} total={total} need {num}/{den}")]
    QuorumNotMet {
        signed: u128,
        total: u128,
        num: u64,
        den: u64,
    },
    #[error("bad attestation signature: {0}")]
    BadSignature(SignatureError),
    #[error("claim already consumed")]
    ClaimAlreadyConsumed,
    #[error("insufficient escrow")]
    InsufficientEscrow,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("zero amount")]
    ZeroAmount,
    #[error("unsupported destination chain")]
    UnsupportedDestChain,
    #[error("bridge not active")]
    NotActive,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn fee_split_half_percent() {
        let (net, fee) = bridge_fee_split(10_000);
        assert_eq!(fee, 50);
        assert_eq!(net, 9_950);
    }

    #[test]
    fn two_of_three_stake_quorum() {
        let k1 = SigningKey::generate(&mut OsRng);
        let k2 = SigningKey::generate(&mut OsRng);
        let k3 = SigningKey::generate(&mut OsRng);
        let a1 = AccountId(k1.verifying_key().to_bytes());
        let a2 = AccountId(k2.verifying_key().to_bytes());
        let a3 = AccountId(k3.verifying_key().to_bytes());
        let digest = [7u8; 32];
        let ev = BridgeAttestationEvidence {
            digest,
            attestations: vec![
                BridgeAttestation::sign(&digest, &k1),
                BridgeAttestation::sign(&digest, &k2),
            ],
        };
        let electorate = [(a1, 100), (a2, 100), (a3, 100)];
        assert!(ev.verify_stake_quorum(&electorate).is_ok());
    }

    #[test]
    fn one_of_three_rejected() {
        let k1 = SigningKey::generate(&mut OsRng);
        let k2 = SigningKey::generate(&mut OsRng);
        let k3 = SigningKey::generate(&mut OsRng);
        let digest = [9u8; 32];
        let ev = BridgeAttestationEvidence {
            digest,
            attestations: vec![BridgeAttestation::sign(&digest, &k1)],
        };
        let electorate = [
            (AccountId(k1.verifying_key().to_bytes()), 100),
            (AccountId(k2.verifying_key().to_bytes()), 100),
            (AccountId(k3.verifying_key().to_bytes()), 100),
        ];
        assert!(matches!(
            ev.verify_stake_quorum(&electorate),
            Err(BridgeError::QuorumNotMet { .. })
        ));
    }

    #[test]
    fn escrow_account_stable() {
        assert_eq!(bridge_escrow_account(), bridge_escrow_account());
    }
}
