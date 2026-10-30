//! Block sync protocol — request/response for fetching blocks by hash or height.

use serde::{Deserialize, Serialize};

use boing_primitives::{Block, Hash};

/// Request for a block (by hash or height).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockRequest {
    ByHash([u8; 32]),
    ByHeight(u64),
}

/// Response: block if found, or unit meaning not found.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockResponse(pub Option<Block>);

impl BlockRequest {
    pub fn by_hash(h: &Hash) -> Self {
        BlockRequest::ByHash(h.0)
    }
}

/// libp2p's request-response CBOR codec rejects responses above 10 MiB, which is smaller than some
/// recorded testnet blocks (heights 1313..=1316 carry up to ~80 MiB of `PutBlob` data, ~160 MiB as
/// CBOR). A fresh node would silently stall at the first such block. Default: 512 MiB, which covers a
/// block holding one 256 MiB (Phase 7 cap) blob encoded as a CBOR integer sequence.
pub const DEFAULT_BLOCK_SYNC_MAX_RESPONSE_BYTES: u64 = 512 * 1024 * 1024;
/// Floor: libp2p's own default, so a typo cannot make ordinary blocks undeliverable.
pub const MIN_BLOCK_SYNC_MAX_RESPONSE_BYTES: u64 = 10 * 1024 * 1024;
/// Ceiling: keeps a mis-set value from allowing unbounded buffering of one response.
pub const MAX_BLOCK_SYNC_MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Block requests are a tiny enum (hash or height). Keep the receive cap tight so a peer cannot
/// force a large allocation on the request path (libp2p's CBOR default is already 1 MiB).
pub const DEFAULT_BLOCK_SYNC_MAX_REQUEST_BYTES: u64 = 64 * 1024;
pub const MIN_BLOCK_SYNC_MAX_REQUEST_BYTES: u64 = 1024;
pub const MAX_BLOCK_SYNC_MAX_REQUEST_BYTES: u64 = 1024 * 1024;
/// libp2p's default request timeout is 10 s, too short to move a ~160 MiB block on a home link.
pub const DEFAULT_BLOCK_SYNC_TIMEOUT_SECS: u64 = 120;
pub const MIN_BLOCK_SYNC_TIMEOUT_SECS: u64 = 10;
pub const MAX_BLOCK_SYNC_TIMEOUT_SECS: u64 = 3600;

pub const ENV_BLOCK_SYNC_MAX_RESPONSE_BYTES: &str = "BOING_P2P_BLOCK_SYNC_MAX_RESPONSE_BYTES";
pub const ENV_BLOCK_SYNC_MAX_REQUEST_BYTES: &str = "BOING_P2P_BLOCK_SYNC_MAX_REQUEST_BYTES";
pub const ENV_BLOCK_SYNC_TIMEOUT_SECS: &str = "BOING_P2P_BLOCK_SYNC_TIMEOUT_SECS";

/// Receive-side limits of the block-sync protocol (not consensus: they only decide what this node is
/// willing to wait for and buffer when it requests a block).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockSyncLimits {
    /// Largest accepted response (CBOR bytes).
    pub max_response_bytes: u64,
    /// Largest accepted request (CBOR bytes).
    pub max_request_bytes: u64,
    /// How long one block request may take before it fails.
    pub request_timeout_secs: u64,
}

impl Default for BlockSyncLimits {
    fn default() -> Self {
        Self {
            max_response_bytes: DEFAULT_BLOCK_SYNC_MAX_RESPONSE_BYTES,
            max_request_bytes: DEFAULT_BLOCK_SYNC_MAX_REQUEST_BYTES,
            request_timeout_secs: DEFAULT_BLOCK_SYNC_TIMEOUT_SECS,
        }
    }
}

impl BlockSyncLimits {
    /// Read `BOING_P2P_BLOCK_SYNC_*` size/timeout env vars.
    pub fn from_env() -> Self {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// [`from_env`](Self::from_env) with an injectable lookup. Unparseable values fall back to the
    /// default; values are clamped to their floor and ceiling.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut c = Self::default();
        let num = |k: &str| get(k).and_then(|v| v.trim().parse::<u64>().ok());
        if let Some(v) = num(ENV_BLOCK_SYNC_MAX_RESPONSE_BYTES) {
            c.max_response_bytes = v.clamp(
                MIN_BLOCK_SYNC_MAX_RESPONSE_BYTES,
                MAX_BLOCK_SYNC_MAX_RESPONSE_BYTES,
            );
        }
        if let Some(v) = num(ENV_BLOCK_SYNC_MAX_REQUEST_BYTES) {
            c.max_request_bytes = v.clamp(
                MIN_BLOCK_SYNC_MAX_REQUEST_BYTES,
                MAX_BLOCK_SYNC_MAX_REQUEST_BYTES,
            );
        }
        if let Some(v) = num(ENV_BLOCK_SYNC_TIMEOUT_SECS) {
            c.request_timeout_secs =
                v.clamp(MIN_BLOCK_SYNC_TIMEOUT_SECS, MAX_BLOCK_SYNC_TIMEOUT_SECS);
        }
        c
    }
}

#[cfg(test)]
mod limits_tests {
    use super::*;

    fn env(pairs: Vec<(&'static str, &'static str)>) -> BlockSyncLimits {
        BlockSyncLimits::from_lookup(|k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn default_covers_the_largest_recorded_testnet_block() {
        let d = BlockSyncLimits::default();
        // Block 1316: ~300 MB of JSON, ~160 MiB as CBOR.
        assert!(d.max_response_bytes >= 256 * 1024 * 1024);
        assert!(d.max_request_bytes <= 1024 * 1024);
        assert!(d.request_timeout_secs > 10);
    }

    #[test]
    fn env_overrides_are_clamped_and_garbage_is_ignored() {
        assert_eq!(
            env(vec![(ENV_BLOCK_SYNC_MAX_RESPONSE_BYTES, "1")]).max_response_bytes,
            MIN_BLOCK_SYNC_MAX_RESPONSE_BYTES
        );
        assert_eq!(
            env(vec![(ENV_BLOCK_SYNC_MAX_RESPONSE_BYTES, "99999999999999")]).max_response_bytes,
            MAX_BLOCK_SYNC_MAX_RESPONSE_BYTES
        );
        assert_eq!(
            env(vec![(ENV_BLOCK_SYNC_MAX_RESPONSE_BYTES, "20971520")]).max_response_bytes,
            20 * 1024 * 1024
        );
        assert_eq!(
            env(vec![(ENV_BLOCK_SYNC_MAX_REQUEST_BYTES, "1")]).max_request_bytes,
            MIN_BLOCK_SYNC_MAX_REQUEST_BYTES
        );
        assert_eq!(
            env(vec![(ENV_BLOCK_SYNC_TIMEOUT_SECS, "1")]).request_timeout_secs,
            10
        );
        assert_eq!(
            env(vec![(ENV_BLOCK_SYNC_TIMEOUT_SECS, "x")]),
            BlockSyncLimits::default()
        );
    }
}
