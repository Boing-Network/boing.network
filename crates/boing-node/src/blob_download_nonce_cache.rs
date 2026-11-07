//! Phase 34.1 (partial): single-use download-proof nonce cache.
//!
//! Download proofs bind a 32-byte nonce but historically verified signature + expiry only,
//! so a captured proof could be replayed until `Expires`. This process-local cache consumes
//! `(blob_id, nonce)` on the first successful authorization and rejects replays until the
//! entry's expiry (or eviction).
//!
//! **Residual:** not persisted across restart — a replay is possible after a node bounce until
//! the proof expires. Full ACL-epoch capability tickets remain Phase 34.1 follow-up.

use std::collections::HashMap;
use std::sync::Mutex;

use boing_primitives::BlobId;

/// Soft cap so a flood of unique nonces cannot grow unbounded (audit M8).
pub const BLOB_DOWNLOAD_NONCE_CACHE_MAX: usize = 65_536;

/// Key: blob id ‖ nonce (64 bytes).
type NonceKey = [u8; 64];

fn key(blob_id: &BlobId, nonce: &[u8; 32]) -> NonceKey {
    let mut k = [0u8; 64];
    k[..32].copy_from_slice(blob_id.as_slice());
    k[32..].copy_from_slice(nonce);
    k
}

#[derive(Default)]
struct Inner {
    /// Maps consumed proof keys → unix expiry (for prune).
    entries: HashMap<NonceKey, u64>,
}

/// Process-local single-use nonce set for Phase 34 download proofs.
#[derive(Default)]
pub struct BlobDownloadNonceCache {
    inner: Mutex<Inner>,
}

impl BlobDownloadNonceCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if this `(blob_id, nonce)` was not yet consumed; inserts it with `expires_at_unix`.
    /// Returns `false` on replay. Prunes expired entries opportunistically; may evict oldest when full.
    pub fn try_consume(&self, blob_id: &BlobId, nonce: &[u8; 32], expires_at_unix: u64, now_unix: u64) -> bool {
        let mut g = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        g.entries.retain(|_, exp| *exp >= now_unix);
        let k = key(blob_id, nonce);
        if g.entries.contains_key(&k) {
            return false;
        }
        if g.entries.len() >= BLOB_DOWNLOAD_NONCE_CACHE_MAX {
            // Evict the soonest-expiring entry to keep a bounded window.
            if let Some(victim) = g
                .entries
                .iter()
                .min_by_key(|(_, exp)| *exp)
                .map(|(k, _)| *k)
            {
                g.entries.remove(&victim);
            }
        }
        g.entries.insert(k, expires_at_unix);
        true
    }

    #[cfg(test)]
    pub fn len_for_test(&self) -> usize {
        self.inner.lock().map(|g| g.entries.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consume_once_rejects_replay() {
        let cache = BlobDownloadNonceCache::new();
        let blob = [9u8; 32];
        let nonce = [3u8; 32];
        assert!(cache.try_consume(&blob, &nonce, 1_700_000_100, 1_700_000_000));
        assert!(!cache.try_consume(&blob, &nonce, 1_700_000_100, 1_700_000_000));
        // Different nonce still ok.
        let nonce2 = [4u8; 32];
        assert!(cache.try_consume(&blob, &nonce2, 1_700_000_100, 1_700_000_000));
    }

    #[test]
    fn expired_entries_pruned_allow_reuse_after_expiry_window() {
        let cache = BlobDownloadNonceCache::new();
        let blob = [1u8; 32];
        let nonce = [2u8; 32];
        assert!(cache.try_consume(&blob, &nonce, 100, 50));
        // After expiry, prune drops the entry so a *new* proof with the same nonce could be
        // accepted — callers must still enforce signature+expiry; expired proofs fail verify first.
        assert!(cache.try_consume(&blob, &nonce, 200, 150));
    }
}
