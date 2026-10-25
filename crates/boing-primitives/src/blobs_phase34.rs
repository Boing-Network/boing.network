//! Phase 34: deployer-owned media-blob download ACL.
//!
//! - Payload indices **34** / **35**: [`crate::TransactionPayload::GrantBlobDownload`] /
//!   [`crate::TransactionPayload::RevokeBlobDownload`] (publisher-only).
//! - Download proofs: domain-separated Ed25519 over blob id + expiry + nonce.
//! - Sidecar ACL is node-local (same class as blob bytes); not in the account SMT.

use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::hash::hasher;
use crate::types::AccountId;
use crate::BlobId;

/// Env: when tip ≥ height, HTTP raw download is private-by-default (publisher/grantee/operator only).
pub const PHASE34_BLOB_ACL_ACTIVATION_ENV: &str = "BOING_BLOB_ACL_ACTIVATION_HEIGHT";

pub const PHASE34_GRANT_BLOB_DOWNLOAD_TX_INDEX: u32 = 34;
pub const PHASE34_REVOKE_BLOB_DOWNLOAD_TX_INDEX: u32 = 35;

pub const BLOB_DOWNLOAD_PROOF_DOMAIN: &[u8] = b"BOING_BLOB_DOWNLOAD_V1";
pub const BLOB_CAPABILITY_DOMAIN: &[u8] = b"BOING_BLOB_CAP_V1";

/// Soft cap on grantees per blob (sidecar ACL).
pub const PHASE34_MAX_BLOB_DOWNLOAD_GRANTEES: usize = 256;

/// Max clock skew for download proof expiry (seconds).
pub const PHASE34_DOWNLOAD_PROOF_MAX_TTL_SECS: u64 = 3600;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobDownloadAcl {
    /// Allowed downloaders (in addition to `BlobMetaRecord.publisher`).
    pub grantees: Vec<AccountId>,
    /// Bumped on every revoke (invalidates capability tickets that bind epoch).
    pub epoch: u64,
}

impl BlobDownloadAcl {
    pub fn contains(&self, id: &AccountId) -> bool {
        self.grantees.iter().any(|g| g == id)
    }

    pub fn grant(&mut self, grantee: AccountId) -> Result<bool, &'static str> {
        if self.contains(&grantee) {
            return Ok(false);
        }
        if self.grantees.len() >= PHASE34_MAX_BLOB_DOWNLOAD_GRANTEES {
            return Err("blob download ACL is full");
        }
        self.grantees.push(grantee);
        Ok(true)
    }

    pub fn revoke(&mut self, grantee: &AccountId) -> bool {
        let before = self.grantees.len();
        self.grantees.retain(|g| g != grantee);
        if self.grantees.len() < before {
            self.epoch = self.epoch.saturating_add(1);
            true
        } else {
            false
        }
    }
}

/// Preimage for a short-lived download authorization signature.
pub fn blob_download_proof_preimage(
    chain_id: u64,
    blob_id: &BlobId,
    expires_at_unix: u64,
    nonce: &[u8; 32],
) -> [u8; 32] {
    let mut h = hasher();
    h.update(BLOB_DOWNLOAD_PROOF_DOMAIN);
    h.update(&chain_id.to_le_bytes());
    h.update(blob_id.as_slice());
    h.update(&expires_at_unix.to_le_bytes());
    h.update(nonce);
    *h.finalize().as_bytes()
}

/// Verify Ed25519 download proof. `signer` AccountId is the verifying key.
pub fn verify_blob_download_proof(
    signer: &AccountId,
    chain_id: u64,
    blob_id: &BlobId,
    expires_at_unix: u64,
    nonce: &[u8; 32],
    signature: &[u8],
    now_unix: u64,
) -> bool {
    if expires_at_unix < now_unix {
        return false;
    }
    if expires_at_unix.saturating_sub(now_unix) > PHASE34_DOWNLOAD_PROOF_MAX_TTL_SECS {
        return false;
    }
    if signature.len() != 64 {
        return false;
    }
    let Ok(vk) = VerifyingKey::from_bytes(&signer.0) else {
        return false;
    };
    let Ok(sig) = Ed25519Signature::from_slice(signature) else {
        return false;
    };
    let msg = blob_download_proof_preimage(chain_id, blob_id, expires_at_unix, nonce);
    vk.verify(&msg, &sig).is_ok()
}

/// Capability ticket preimage (optional share-link path).
pub fn blob_capability_preimage(
    chain_id: u64,
    blob_id: &BlobId,
    epoch: u64,
    expires_at_unix: u64,
    optional_grantee: Option<&AccountId>,
) -> [u8; 32] {
    let mut h = hasher();
    h.update(BLOB_CAPABILITY_DOMAIN);
    h.update(&chain_id.to_le_bytes());
    h.update(blob_id.as_slice());
    h.update(&epoch.to_le_bytes());
    h.update(&expires_at_unix.to_le_bytes());
    match optional_grantee {
        Some(g) => {
            h.update(&[1u8]);
            h.update(g.0.as_slice());
        }
        None => {
            h.update(&[0u8]);
        }
    }
    *h.finalize().as_bytes()
}

pub fn verify_blob_capability(
    publisher: &AccountId,
    chain_id: u64,
    blob_id: &BlobId,
    epoch: u64,
    expires_at_unix: u64,
    optional_grantee: Option<&AccountId>,
    signature: &[u8],
    now_unix: u64,
    current_epoch: u64,
) -> bool {
    if epoch != current_epoch {
        return false;
    }
    if expires_at_unix < now_unix {
        return false;
    }
    if signature.len() != 64 {
        return false;
    }
    let Ok(vk) = VerifyingKey::from_bytes(&publisher.0) else {
        return false;
    };
    let Ok(sig) = Ed25519Signature::from_slice(signature) else {
        return false;
    };
    let msg = blob_capability_preimage(chain_id, blob_id, epoch, expires_at_unix, optional_grantee);
    vk.verify(&msg, &sig).is_ok()
}

pub fn phase34_blob_acl_activation_height() -> Option<u64> {
    std::env::var(PHASE34_BLOB_ACL_ACTIVATION_ENV)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn phase34_blob_acl_active_at_tip(tip: u64) -> bool {
    phase34_blob_acl_activation_height().is_some_and(|h| tip >= h)
}

pub fn phase34_notes() -> &'static str {
    "Phase 34: deployer-owned blob download ACL — GrantBlobDownload (34) / RevokeBlobDownload (35); signed download proofs; optional capability tickets; BOING_BLOB_ACL_ACTIVATION_HEIGHT for private-by-default HTTP download."
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn download_proof_roundtrip() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk = AccountId(sk.verifying_key().to_bytes());
        let blob = [9u8; 32];
        let nonce = [3u8; 32];
        let exp = 1_700_000_100u64;
        let msg = blob_download_proof_preimage(6913, &blob, exp, &nonce);
        let sig = sk.sign(&msg);
        assert!(verify_blob_download_proof(
            &pk,
            6913,
            &blob,
            exp,
            &nonce,
            sig.to_bytes().as_slice(),
            1_700_000_000
        ));
        assert!(!verify_blob_download_proof(
            &pk,
            6913,
            &blob,
            exp,
            &nonce,
            sig.to_bytes().as_slice(),
            1_700_000_200
        ));
    }

    #[test]
    fn acl_grant_revoke_bumps_epoch() {
        let mut acl = BlobDownloadAcl::default();
        let g = AccountId([1u8; 32]);
        assert_eq!(acl.grant(g).unwrap(), true);
        assert_eq!(acl.epoch, 0);
        assert!(acl.revoke(&g));
        assert_eq!(acl.epoch, 1);
        assert!(!acl.contains(&g));
    }
}
