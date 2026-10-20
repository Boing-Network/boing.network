//! L1 media blobs: content-addressed ids, retention policy, and PutBlob caps.
//!
//! Bytes live in a node sidecar (not account SMT / SSTORE). Consensus records metadata via
//! [`crate::TransactionPayload::PutBlob`]. See `docs/BOING-L1-MEDIA-BLOBS.md`.

use serde::{Deserialize, Serialize};

use crate::hasher;

/// Domain / URI scheme for client resolvers (`boing://blob/0x…`).
pub const MEDIA_BLOB_URI_PREFIX: &str = "boing://blob/";

/// MVP max payload size (256 KiB). Raise in later phases; do not claim video-scale here.
pub const MAX_MEDIA_BLOB_BYTES: usize = 256 * 1024;

/// Max UTF-8 length of the optional MIME / content-type hint.
pub const MAX_MEDIA_BLOB_CONTENT_TYPE_LEN: usize = 64;

/// Soft cap on prunable TTL (blocks) for MVP admission.
pub const MAX_MEDIA_BLOB_TTL_BLOCKS: u64 = 1_000_000;

/// Base gas for a PutBlob before size and retention scaling.
pub const MEDIA_BLOB_GAS_BASE: u64 = 21_000;

/// Gas per 32-byte word of payload (sidecar data — far cheaper than SSTORE).
pub const MEDIA_BLOB_GAS_PER_WORD: u64 = 16;

/// Forever retention multiplies blob gas by this factor (prepaid best-effort in MVP).
pub const MEDIA_BLOB_FOREVER_MULTIPLIER: u64 = 10;

/// Content-addressed blob id: `BLAKE3(payload bytes)`.
pub type BlobId = [u8; 32];

/// Retention policy encoded in PutBlob and sidecar metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlobRetention {
    /// Node may delete after `included_height + ttl_blocks`.
    Prunable { ttl_blocks: u64 },
    /// Prepaid “best-effort forever” on full nodes (higher fee). Not a cryptographic eternity guarantee in MVP.
    Forever,
}

impl BlobRetention {
    /// Multiplier applied to size-scaled blob gas.
    pub fn gas_multiplier(self) -> u64 {
        match self {
            BlobRetention::Prunable { .. } => 1,
            BlobRetention::Forever => MEDIA_BLOB_FOREVER_MULTIPLIER,
        }
    }

    /// Structural validation (independent of activation height).
    pub fn validate(self) -> Result<(), BlobError> {
        match self {
            BlobRetention::Forever => Ok(()),
            BlobRetention::Prunable { ttl_blocks } => {
                if ttl_blocks == 0 {
                    return Err(BlobError::TtlZero);
                }
                if ttl_blocks > MAX_MEDIA_BLOB_TTL_BLOCKS {
                    return Err(BlobError::TtlTooLarge {
                        ttl_blocks,
                        max: MAX_MEDIA_BLOB_TTL_BLOCKS,
                    });
                }
                Ok(())
            }
        }
    }

    /// Height at which a prunable blob may be GC'd (`None` = Forever).
    pub fn expiry_height(self, included_height: u64) -> Option<u64> {
        match self {
            BlobRetention::Forever => None,
            BlobRetention::Prunable { ttl_blocks } => {
                Some(included_height.saturating_add(ttl_blocks))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BlobError {
    #[error("media blob exceeds MAX_MEDIA_BLOB_BYTES ({MAX_MEDIA_BLOB_BYTES})")]
    TooLarge { size: usize },
    #[error("media blob content_type exceeds {MAX_MEDIA_BLOB_CONTENT_TYPE_LEN} UTF-8 bytes")]
    ContentTypeTooLong { len: usize },
    #[error("prunable retention requires ttl_blocks >= 1")]
    TtlZero,
    #[error("prunable ttl_blocks {ttl_blocks} exceeds max {max}")]
    TtlTooLarge { ttl_blocks: u64, max: u64 },
    #[error("blob id mismatch: expected {expected}, got {got}")]
    IdMismatch { expected: String, got: String },
}

/// `BLAKE3(data)` as [`BlobId`].
#[must_use]
pub fn blob_id_of(data: &[u8]) -> BlobId {
    let mut h = hasher();
    h.update(data);
    *h.finalize().as_bytes()
}

/// Hex form without `0x` (64 chars).
#[must_use]
pub fn blob_id_hex(id: &BlobId) -> String {
    hex::encode(id)
}

/// `boing://blob/0x{64 hex}`.
#[must_use]
pub fn media_blob_uri(id: &BlobId) -> String {
    format!("{MEDIA_BLOB_URI_PREFIX}0x{}", blob_id_hex(id))
}

/// Parse `boing://blob/0x…` or bare 64-hex / `0x` hex into a [`BlobId`].
pub fn parse_media_blob_id(s: &str) -> Option<BlobId> {
    let s = s.trim();
    let hex_part = if let Some(rest) = s.strip_prefix(MEDIA_BLOB_URI_PREFIX) {
        rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")).unwrap_or(rest)
    } else {
        s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s)
    };
    if hex_part.len() != 64 {
        return None;
    }
    let bytes = hex::decode(hex_part).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut id = [0u8; 32];
    id.copy_from_slice(&bytes);
    Some(id)
}

/// Validate PutBlob fields before admission / execution.
pub fn validate_put_blob(retention: BlobRetention, content_type: &str, data: &[u8]) -> Result<BlobId, BlobError> {
    retention.validate()?;
    if data.len() > MAX_MEDIA_BLOB_BYTES {
        return Err(BlobError::TooLarge { size: data.len() });
    }
    if content_type.len() > MAX_MEDIA_BLOB_CONTENT_TYPE_LEN {
        return Err(BlobError::ContentTypeTooLong {
            len: content_type.len(),
        });
    }
    Ok(blob_id_of(data))
}

/// Size-scaled blob gas before retention multiplier.
#[must_use]
pub fn media_blob_gas_raw(size: usize) -> u64 {
    let words = (size as u64).div_ceil(32).max(1);
    MEDIA_BLOB_GAS_BASE.saturating_add(words.saturating_mul(MEDIA_BLOB_GAS_PER_WORD))
}

/// Effective gas charged for PutBlob (raw × retention multiplier).
#[must_use]
pub fn media_blob_gas(size: usize, retention: BlobRetention) -> u64 {
    media_blob_gas_raw(size).saturating_mul(retention.gas_multiplier())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_blake3() {
        let data = b"hello media blob";
        let id = blob_id_of(data);
        let mut h = hasher();
        h.update(data);
        assert_eq!(id, *h.finalize().as_bytes());
        let uri = media_blob_uri(&id);
        assert!(uri.starts_with(MEDIA_BLOB_URI_PREFIX));
        assert_eq!(parse_media_blob_id(&uri), Some(id));
        assert_eq!(parse_media_blob_id(&blob_id_hex(&id)), Some(id));
    }

    #[test]
    fn caps_and_gas() {
        assert!(validate_put_blob(BlobRetention::Forever, "image/png", &[0u8; 10]).is_ok());
        assert!(matches!(
            validate_put_blob(BlobRetention::Prunable { ttl_blocks: 0 }, "", &[1]),
            Err(BlobError::TtlZero)
        ));
        let big = vec![0u8; MAX_MEDIA_BLOB_BYTES + 1];
        assert!(matches!(
            validate_put_blob(BlobRetention::Forever, "", &big),
            Err(BlobError::TooLarge { .. })
        ));
        let g = media_blob_gas(32, BlobRetention::Forever);
        assert_eq!(g, media_blob_gas_raw(32) * MEDIA_BLOB_FOREVER_MULTIPLIER);
        assert_eq!(
            BlobRetention::Prunable { ttl_blocks: 10 }.expiry_height(100),
            Some(110)
        );
        assert_eq!(BlobRetention::Forever.expiry_height(100), None);
    }
}
