//! Bounded retention of in-memory execution receipts (`BoingNode::receipts`).
//!
//! Receipts used to be kept forever, so RAM grew with every transaction ever executed. Receipts are
//! also persisted per height, so only a recent window needs to be resident. Lookups inside the window
//! (`boing_getTransactionReceipt`, `includeReceipts`, `boing_getLogs`, the account tx index) are
//! unchanged; for a tx older than the window `boing_getTransactionReceipt` returns `null`.

use std::collections::HashMap;
use std::sync::OnceLock;

use boing_primitives::{ExecutionReceipt, Hash};

/// Default window: roughly the last million blocks (`0` = keep everything, the previous behaviour).
pub const DEFAULT_RECEIPT_RETENTION_BLOCKS: u64 = 1_000_000;
/// Environment variable overriding the window (blocks; `0` = unlimited).
pub const ENV_RECEIPT_RETENTION_BLOCKS: &str = "BOING_RECEIPT_RETENTION_BLOCKS";
/// Pruning runs when `height % PRUNE_INTERVAL == 0`, so the O(n) scan is amortised over many blocks.
pub const PRUNE_INTERVAL: u64 = 256;

/// Parse a retention value; blank / unparseable falls back to the default.
pub fn parse_retention(raw: Option<&str>) -> u64 {
    raw.and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_RECEIPT_RETENTION_BLOCKS)
}

/// Configured window (read once from `BOING_RECEIPT_RETENTION_BLOCKS`).
pub fn retention_blocks() -> u64 {
    static CFG: OnceLock<u64> = OnceLock::new();
    *CFG.get_or_init(|| {
        parse_retention(std::env::var(ENV_RECEIPT_RETENTION_BLOCKS).ok().as_deref())
    })
}

/// Window as published over RPC (`receipt_retention_blocks`): `None` (JSON `null`) when unbounded.
pub fn window_for_rpc(window: u64) -> Option<u64> {
    if window == 0 {
        None
    } else {
        Some(window)
    }
}

/// First height whose receipts are kept when the tip is `tip` (0 when unbounded or the chain is shorter).
pub fn first_retained_height(tip: u64, window: u64) -> u64 {
    if window == 0 {
        0
    } else {
        tip.saturating_sub(window.saturating_sub(1))
    }
}

/// Drop receipts below the window. Returns how many were removed.
pub fn prune(receipts: &mut HashMap<Hash, ExecutionReceipt>, tip: u64, window: u64) -> usize {
    if window == 0 {
        return 0;
    }
    let first = first_retained_height(tip, window);
    let before = receipts.len();
    receipts.retain(|_, r| r.block_height >= first);
    before - receipts.len()
}

/// True when pruning should run after committing `height`.
pub fn prune_due(height: u64) -> bool {
    height > 0 && height % PRUNE_INTERVAL == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(height: u64, n: u8) -> ExecutionReceipt {
        ExecutionReceipt {
            tx_id: Hash([n; 32]),
            block_height: height,
            tx_index: 0,
            success: true,
            gas_used: 1,
            return_data: vec![],
            logs: vec![],
            error: None,
        }
    }

    #[test]
    fn window_for_rpc_maps_zero_to_null() {
        assert_eq!(window_for_rpc(0), None);
        assert_eq!(window_for_rpc(1), Some(1));
        assert_eq!(
            window_for_rpc(DEFAULT_RECEIPT_RETENTION_BLOCKS),
            Some(1_000_000)
        );
    }

    #[test]
    fn parse_defaults_and_overrides() {
        assert_eq!(parse_retention(None), DEFAULT_RECEIPT_RETENTION_BLOCKS);
        assert_eq!(
            parse_retention(Some("abc")),
            DEFAULT_RECEIPT_RETENTION_BLOCKS
        );
        assert_eq!(parse_retention(Some(" 500 ")), 500);
        assert_eq!(parse_retention(Some("0")), 0);
    }

    #[test]
    fn window_boundaries() {
        assert_eq!(first_retained_height(1000, 100), 901);
        assert_eq!(first_retained_height(50, 100), 0);
        assert_eq!(first_retained_height(1000, 0), 0);
        assert_eq!(first_retained_height(1000, 1), 1000);
    }

    #[test]
    fn prune_keeps_exactly_the_window() {
        let mut m: HashMap<Hash, ExecutionReceipt> = HashMap::new();
        for h in 1..=300u64 {
            let r = receipt(h, 0);
            // distinct ids: mix height into the id
            let mut id = [0u8; 32];
            id[..8].copy_from_slice(&h.to_le_bytes());
            m.insert(
                Hash(id),
                ExecutionReceipt {
                    tx_id: Hash(id),
                    ..r
                },
            );
        }
        let removed = prune(&mut m, 300, 100);
        assert_eq!(removed, 200);
        assert_eq!(m.len(), 100);
        assert!(m
            .values()
            .all(|r| r.block_height >= 201 && r.block_height <= 300));
        // unlimited never prunes
        assert_eq!(prune(&mut m, 10_000, 0), 0);
        assert_eq!(m.len(), 100);
    }

    #[test]
    fn prune_schedule() {
        assert!(!prune_due(0));
        assert!(!prune_due(255));
        assert!(prune_due(256));
        assert!(prune_due(512));
    }
}
