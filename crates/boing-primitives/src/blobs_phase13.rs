//! Phase 13: FailedServe appeal → native balance unburn helpers, settle-receipt TTL,
//! activation notes (Fly-safe 16 MiB default; admission height unchanged).
//!
//! Honest scope:
//! - Approved BlobFailedServe appeals restore provider **balance** from the fee burn sink.
//! - Settle receipts TTL-prune and are consumed when a challenge closes.
//! - ReservedRealV1 verify still rejects; do not flip `is_implemented`.
//! - Cap env unset → 16 MiB; do not change `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.

use crate::blobs_phase12::{
    phase12_cap_activation_checklist, phase12_das_succinct_proof_plan,
    PHASE12_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE12_CAP_ACTIVATION_ENV,
    PHASE12_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE12_MAX_SETTLE_RECEIPTS,
    PHASE12_SERVE_CHALLENGE_WINDOW_BLOCKS,
};

/// Env name for gated 256 MiB raise (unchanged).
pub const PHASE13_CAP_ACTIVATION_ENV: &str = PHASE12_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height.
pub const PHASE13_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE12_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Appeal-window hint for FailedServe SlashRegistry records.
pub const PHASE13_FAILED_SERVE_APPEAL_WINDOW_BLOCKS: u64 =
    PHASE12_FAILED_SERVE_APPEAL_WINDOW_BLOCKS;

/// Max retained settle receipts (alias).
pub const PHASE13_MAX_SETTLE_RECEIPTS: usize = PHASE12_MAX_SETTLE_RECEIPTS;

/// Serve-challenge window alias.
pub const PHASE13_SERVE_CHALLENGE_WINDOW_BLOCKS: u64 = PHASE12_SERVE_CHALLENGE_WINDOW_BLOCKS;

/// Drop settle receipts older than this many blocks past `settled_at_height`.
pub const PHASE13_SETTLE_RECEIPT_TTL_BLOCKS: u64 = 4096;

/// Operator-facing plan strings (Phase 13 unburn + prune; ReservedRealV1 still reject-until-wired).
pub fn phase13_das_succinct_proof_plan() -> Vec<&'static str> {
    let mut v = phase12_das_succinct_proof_plan();
    v.extend([
        "Phase 13: approved BlobFailedServe appeal restores provider native balance from FEE_BURN_SINK (not stake).",
        "Phase 13: settle receipts TTL-prune and are consumed when a serve challenge closes (Respond / FailedServe).",
    ]);
    v
}

/// Extended activation checklist (Phase 12 + Phase 13 ops). Do not rewrite live admission height.
pub fn phase13_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase12_cap_activation_checklist(activation_height);
    v.extend([
        "FailedServe approved appeal (operator RPC) restores provider balance from the burn sink; multi-node ops must resolve consistently until a consensus appeal tx exists.",
        "Do not change BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT once set on Fly (currently coordinated at 1200) unless operators re-agree.",
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset (Fly-safe 16 MiB) until coordinated disk/RPC preflight.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_and_aliases() {
        assert_eq!(PHASE13_SETTLE_RECEIPT_TTL_BLOCKS, 4096);
        assert_eq!(
            PHASE13_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
            PHASE12_FAILED_SERVE_APPEAL_WINDOW_BLOCKS
        );
    }

    #[test]
    fn checklist_and_plan_nonempty() {
        assert!(phase13_cap_activation_checklist(None).len() >= 14);
        assert!(phase13_das_succinct_proof_plan().len() >= 8);
    }
}
