//! Phase 9: SMT-resident sample-offer book + ask_fee escrow helpers, leader-VRF DAS LC path,
//! and extended public-testnet 256 MiB activation ops notes.
//!
//! Honest scope:
//! - Escrow account id + domain tags for the [`boing_state`] sample-offer registry.
//! - Leader-VRF output → DAS LC seed helpers (engine/RPC wire the round output).
//! - Cap activation checklist extended (default still unset / Fly-safe 16 MiB).
//! - Succinct DAS remains StubBlake3V0; ReservedRealV1 still rejects.

use crate::blobs::BlobId;
use crate::blobs_phase7::das_light_client_seed_from_vrf;
use crate::blobs_phase8::{phase8_cap_activation_checklist, PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES};
use crate::hasher;
use crate::AccountId;

/// Domain for the system ask-fee escrow account id.
pub const PHASE9_ASK_ESCROW_DOMAIN: &[u8] = b"BOING_BLOB_SAMPLE_ASK_ESCROW_V1";

/// Domain for the SMT sample-offer registry leaf (must match `boing_state::SampleOfferRegistry`).
pub const PHASE9_SAMPLE_OFFER_REGISTRY_DOMAIN: &[u8] = b"BOING_BLOB_SAMPLE_MARKET_V1";

/// Env name for gated 256 MiB raise (unchanged from Phase 7/8).
pub const PHASE9_CAP_ACTIVATION_ENV: &str = "BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT";

/// Activated max after coordinated height.
pub const PHASE9_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Fixed system account that holds escrowed `ask_fee` until settle/cancel/expiry refund.
#[must_use]
pub fn blob_sample_ask_escrow_account() -> AccountId {
    let mut h = hasher();
    h.update(PHASE9_ASK_ESCROW_DOMAIN);
    AccountId(*h.finalize().as_bytes())
}

/// Market key shared with Phase 6/8 books: `blob_hex:chunk:provider_hex`.
#[must_use]
pub fn sample_offer_market_key(blob_id: &BlobId, chunk_index: u32, provider: &AccountId) -> String {
    format!(
        "{}:{}:{}",
        hex::encode(blob_id),
        chunk_index,
        hex::encode(provider.0)
    )
}

/// DAS LC seed from a consensus leader-election VRF **output** (32 bytes).
///
/// When leader election falls back to [`crate::dummy_vrf_output`], pass that stub's `.output` so
/// LC clients match the engine. Callers with real ECVRF must verify before trusting the bytes.
#[must_use]
pub fn das_light_client_seed_from_leader_vrf(
    blob_id: &BlobId,
    leader_vrf_output: &[u8; 32],
) -> u64 {
    das_light_client_seed_from_vrf(blob_id, leader_vrf_output)
}

/// Extended coordinated activation checklist (Phase 8 lines + Phase 9 ops).
pub fn phase9_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let mut v = phase8_cap_activation_checklist(activation_height);
    v.extend([
        "Pre-flight: confirm Fly / host disk quotas, RPC gateway body limits, and explorer/indexer timeouts tolerate 256 MiB Put/Announce.",
        "Roll order: deploy identical binary everywhere → confirm all validators on that build → set identical H → restart all → verify params → only then load-test large Puts.",
        "Rollback: never partially unset the env after tip >= H on some nodes; recovery requires a coordinated freeze / emergency procedure — do not partial-roll.",
        "Monitor admission rejects / logs for size > 16 MiB before H; after H, watch OOM and RPC timeouts on large blobs.",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escrow_account_stable() {
        let a = blob_sample_ask_escrow_account();
        let b = blob_sample_ask_escrow_account();
        assert_eq!(a, b);
        assert_ne!(a, AccountId([0u8; 32]));
    }

    #[test]
    fn checklist_extends_phase8() {
        let c8 = phase8_cap_activation_checklist(None);
        let c9 = phase9_cap_activation_checklist(None);
        assert!(c9.len() > c8.len());
        assert_eq!(PHASE9_ACTIVATED_MAX_MEDIA_BLOB_BYTES, 256 * 1024 * 1024);
    }

    #[test]
    fn market_key_format() {
        let k = sample_offer_market_key(&[1u8; 32], 3, &AccountId([2u8; 32]));
        assert!(k.contains(":3:"));
    }
}
