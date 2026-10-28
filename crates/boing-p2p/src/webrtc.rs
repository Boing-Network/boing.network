//! Decentralized WebRTC signaling types.
//!
//! See WEBRTC-SIGNALING.md for full design.

use serde::{Deserialize, Serialize};

/// On-chain rate limit for signaling messages (spam prevention).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalingRateLimit {
    /// Max messages (offers + answers) per address per window.
    pub max_per_window: u32,
    /// Window duration in seconds.
    pub window_secs: u64,
}

impl Default for SignalingRateLimit {
    fn default() -> Self {
        Self {
            max_per_window: 5,
            window_secs: 60,
        }
    }
}

/// Deposit requirement for initiating an offer (spam prevention).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalingDepositConfig {
    /// Minimum BOING deposit required to post an offer.
    pub min_deposit: u128,
    /// Seconds after which unclaimed deposit can be reclaimed.
    pub expiration_secs: u64,
}

impl Default for SignalingDepositConfig {
    fn default() -> Self {
        Self {
            min_deposit: 100,
            expiration_secs: 300,
        }
    }
}

/// Signaling message kinds for offer/answer/ICE exchange.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalingMessageKind {
    /// WebRTC offer (SDP); encrypted with recipient's public key.
    Offer,
    /// WebRTC answer (SDP); encrypted with initiator's public key.
    Answer,
    /// ICE candidate for NAT traversal.
    IceCandidate,
}

/// Pointer to off-chain content (e.g. IPFS CID for large SDP).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentPointer {
    /// Content identifier (e.g. IPFS CID).
    pub cid: String,
    /// Optional hash for integrity check.
    pub hash: Option<[u8; 32]>,
}

/// WebRTC signaling message — posted to Boing contract or gossip.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalingMessage {
    pub kind: SignalingMessageKind,
    /// Sender's Boing account ID (public key).
    pub sender: [u8; 32],
    /// Recipient's Boing account ID.
    pub recipient: [u8; 32],
    /// Inline payload (for small SDP/ICE) or None if using content_pointer.
    pub payload: Option<Vec<u8>>,
    /// Pointer to off-chain content (IPFS/Filecoin) for large SDPs.
    pub content_pointer: Option<ContentPointer>,
}

impl SignalingMessage {
    pub fn offer(sender: [u8; 32], recipient: [u8; 32], payload: Vec<u8>) -> Self {
        Self {
            kind: SignalingMessageKind::Offer,
            sender,
            recipient,
            payload: Some(payload),
            content_pointer: None,
        }
    }

    pub fn answer(sender: [u8; 32], recipient: [u8; 32], payload: Vec<u8>) -> Self {
        Self {
            kind: SignalingMessageKind::Answer,
            sender,
            recipient,
            payload: Some(payload),
            content_pointer: None,
        }
    }

    pub fn ice_candidate(sender: [u8; 32], recipient: [u8; 32], payload: Vec<u8>) -> Self {
        Self {
            kind: SignalingMessageKind::IceCandidate,
            sender,
            recipient,
            payload: Some(payload),
            content_pointer: None,
        }
    }

    /// Create offer/answer with off-chain content pointer (large SDP).
    pub fn with_pointer(
        kind: SignalingMessageKind,
        sender: [u8; 32],
        recipient: [u8; 32],
        cid: String,
    ) -> Self {
        Self {
            kind,
            sender,
            recipient,
            payload: None,
            content_pointer: Some(ContentPointer { cid, hash: None }),
        }
    }
}

// --- STUN/TURN Registry & Reputation ---

/// Performance metrics for a STUN/TURN server (reputation inputs).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StunTurnMetrics {
    pub uptime_bps: u16,
    pub avg_latency_ms: u32,
    pub success_rate_bps: u16,
    pub throughput_mb: u64,
}

/// Reputation score for a STUN/TURN server (0–10000 = 0–100%).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StunTurnReputation {
    pub score: u16,
}

impl StunTurnReputation {
    pub const MAX: u16 = 10_000;

    pub fn new(score: u16) -> Self {
        Self {
            score: score.min(Self::MAX),
        }
    }

    pub fn from_metrics(metrics: &StunTurnMetrics) -> Self {
        let uptime = metrics.uptime_bps as u32;
        let success = metrics.success_rate_bps as u32;
        let latency_penalty = ((metrics.avg_latency_ms.saturating_sub(100)).min(500) / 10) as u16;
        let base = ((uptime + success) / 2) as u16;
        let raw = base.saturating_add(5000).saturating_sub(latency_penalty);
        Self::new(raw.min(Self::MAX))
    }
}

/// Registry entry for a registered STUN/TURN server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StunTurnRegistryEntry {
    pub server_id: [u8; 32],
    pub multiaddr: String,
    pub stake: u128,
    pub reputation: StunTurnReputation,
    pub registered_at_secs: u64,
}

// --- Signaling Contract Interface (stub) ---

/// Result of posting a signaling message to the contract.
#[derive(Debug)]
pub struct SignalingPostResult {
    pub ok: bool,
    pub message_id: Option<[u8; 32]>,
}

/// Stub interface for the on-chain WebRTC signaling contract.
/// See WEBRTC-SIGNALING.md for full design. Implementations would
/// interact with the actual Boing smart contract.
#[allow(unused_variables)]
pub trait SignalingContract {
    /// Post an offer. Returns result with message_id if accepted.
    fn post_offer(
        &self,
        sender: [u8; 32],
        recipient: [u8; 32],
        payload: Option<Vec<u8>>,
        content_pointer: Option<ContentPointer>,
        deposit: u128,
    ) -> SignalingPostResult {
        SignalingPostResult {
            ok: false,
            message_id: None,
        }
    }

    /// Post an answer in response to an offer.
    fn post_answer(
        &self,
        sender: [u8; 32],
        recipient: [u8; 32],
        payload: Option<Vec<u8>>,
        content_pointer: Option<ContentPointer>,
    ) -> SignalingPostResult {
        SignalingPostResult {
            ok: false,
            message_id: None,
        }
    }

    /// Post an ICE candidate.
    fn post_ice_candidate(
        &self,
        sender: [u8; 32],
        recipient: [u8; 32],
        payload: Vec<u8>,
    ) -> SignalingPostResult {
        SignalingPostResult {
            ok: false,
            message_id: None,
        }
    }

    /// Fetch pending offers for a recipient (stub).
    fn get_pending_offers(&self, recipient: [u8; 32]) -> Vec<SignalingMessage> {
        vec![]
    }
}
