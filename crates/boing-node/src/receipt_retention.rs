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

/// Error prefix of [`boing_execution`]'s `VmError::InvalidNonce` as stored in receipts.
pub const NONCE_REJECT_ERROR_PREFIX: &str = "Invalid nonce:";

/// How authoritative a receipt is for its tx id when the same tx id appears in more than one block:
/// success > failed execution (nonce consumed) > `Invalid nonce` rejection (a no-op duplicate or a
/// premature inclusion; never the inclusion that executed the tx).
fn receipt_rank(r: &ExecutionReceipt) -> u8 {
    if r.success {
        2
    } else if r
        .error
        .as_deref()
        .is_some_and(|e| e.starts_with(NONCE_REJECT_ERROR_PREFIX))
    {
        0
    } else {
        1
    }
}

/// Insert `r` into the tx-id-keyed receipt index without letting a duplicate inclusion hide the
/// real outcome.
///
/// The index is keyed by tx id, but a tx id can legitimately appear in more than one block (a
/// validator re-including an already-executed tx; the later copy fails with `Invalid nonce` and
/// does nothing). Plain last-write-wins made `boing_getTransactionReceipt` report that no-op failure
/// instead of the inclusion that executed. Rule: a receipt from the **same** block height replaces
/// (re-commit / reload of the same block); otherwise the higher-ranked receipt wins (see
/// [`receipt_rank`]) and, on a tie, the earlier block. Per-height receipt files on disk are never
/// affected. Returns `true` when `r` was stored.
pub fn index_receipt(receipts: &mut HashMap<Hash, ExecutionReceipt>, r: ExecutionReceipt) -> bool {
    if let Some(existing) = receipts.get(&r.tx_id) {
        if existing.block_height != r.block_height {
            let (new_rank, old_rank) = (receipt_rank(&r), receipt_rank(existing));
            let replace = new_rank > old_rank
                || (new_rank == old_rank && r.block_height < existing.block_height);
            if !replace {
                return false;
            }
        }
    }
    receipts.insert(r.tx_id, r);
    true
}

/// True when pruning should run after committing `height`.
pub fn prune_due(height: u64) -> bool {
    height > 0 && height.is_multiple_of(PRUNE_INTERVAL)
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

    fn nonce_dup(height: u64, n: u8) -> ExecutionReceipt {
        ExecutionReceipt {
            success: false,
            gas_used: 0,
            tx_index: 1,
            error: Some("Invalid nonce: expected 45, got 44".into()),
            ..receipt(height, n)
        }
    }

    #[test]
    fn duplicate_nonce_failure_does_not_overwrite_success() {
        // Live testnet shape: success at N, the same tx id re-included at N+1 and rejected.
        let mut m: HashMap<Hash, ExecutionReceipt> = HashMap::new();
        assert!(index_receipt(&mut m, receipt(66276, 7)));
        assert!(!index_receipt(&mut m, nonce_dup(66277, 7)));
        let r = &m[&Hash([7; 32])];
        assert!(r.success);
        assert_eq!(r.block_height, 66276);
        // Reverse arrival order (any reindex order) converges on the same answer.
        let mut m2: HashMap<Hash, ExecutionReceipt> = HashMap::new();
        assert!(index_receipt(&mut m2, nonce_dup(66277, 7)));
        assert!(index_receipt(&mut m2, receipt(66276, 7)));
        assert_eq!(m2[&Hash([7; 32])], m[&Hash([7; 32])]);
    }

    #[test]
    fn real_failure_beats_nonce_duplicate_and_earliest_wins_ties() {
        let mut m: HashMap<Hash, ExecutionReceipt> = HashMap::new();
        let reverted = ExecutionReceipt {
            success: false,
            error: Some("Execution reverted".into()),
            ..receipt(10, 3)
        };
        assert!(index_receipt(&mut m, reverted.clone()));
        assert!(!index_receipt(&mut m, nonce_dup(11, 3)));
        assert_eq!(m[&Hash([3; 32])], reverted);
        // A premature (future-nonce) rejection followed by the real execution: execution wins.
        let mut m2: HashMap<Hash, ExecutionReceipt> = HashMap::new();
        assert!(index_receipt(&mut m2, nonce_dup(20, 4)));
        assert!(index_receipt(&mut m2, receipt(22, 4)));
        assert!(m2[&Hash([4; 32])].success);
        // Same height always replaces (re-commit / reload of the same block).
        let mut m3: HashMap<Hash, ExecutionReceipt> = HashMap::new();
        assert!(index_receipt(&mut m3, receipt(30, 5)));
        assert!(index_receipt(&mut m3, nonce_dup(30, 5)));
        assert!(!m3[&Hash([5; 32])].success);
    }

    #[test]
    fn prune_schedule() {
        assert!(!prune_due(0));
        assert!(!prune_due(255));
        assert!(prune_due(256));
        assert!(prune_due(512));
    }
}
