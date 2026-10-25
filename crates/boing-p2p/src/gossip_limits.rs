//! Gossip hardening knobs: max message size, per-peer rate limiting, peer scoring.
//!
//! Defaults are safe for the live networks: the size limit only ever *raises* what this node accepts
//! (peers still on the old 64 KiB default drop larger frames on their side), rate limits are generous,
//! and scoring only penalises messages that fail to decode / verify.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use libp2p::PeerId;

/// Default max gossip frame: the 16 MiB media-blob cap plus 1 MiB of framing/metadata, which also
/// covers a full block (`max_txs_per_block` defaults to 1000) at typical transaction sizes.
/// libp2p's own default is 64 KiB, far too small for a full block or a media blob.
pub const DEFAULT_MAX_GOSSIP_MESSAGE_BYTES: usize = 17 * 1024 * 1024;
/// Hard floor (the libp2p default) so a typo cannot make the node drop every block.
pub const MIN_MAX_GOSSIP_MESSAGE_BYTES: usize = 64 * 1024;
/// Hard ceiling to keep a mis-set value from allowing multi-GiB allocations per frame.
pub const MAX_MAX_GOSSIP_MESSAGE_BYTES: usize = 256 * 1024 * 1024;
/// Default sustained gossip messages per second accepted from one peer (`0` = unlimited).
pub const DEFAULT_PEER_MSGS_PER_SEC: u32 = 500;
/// Default sustained gossip bytes per second accepted from one peer (`0` = unlimited).
pub const DEFAULT_PEER_BYTES_PER_SEC: u64 = 64 * 1024 * 1024;
/// Burst allowance, in seconds of the sustained rate.
pub const BURST_SECONDS: u32 = 2;

pub const ENV_MAX_MESSAGE_BYTES: &str = "BOING_P2P_MAX_GOSSIP_MESSAGE_BYTES";
pub const ENV_PEER_MSGS_PER_SEC: &str = "BOING_P2P_GOSSIP_MAX_MSGS_PER_SEC_PER_PEER";
pub const ENV_PEER_BYTES_PER_SEC: &str = "BOING_P2P_GOSSIP_MAX_BYTES_PER_SEC_PER_PEER";
pub const ENV_PEER_SCORING: &str = "BOING_P2P_PEER_SCORING";

/// Gossip tuning for [`crate::P2pNode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipConfig {
    /// Largest gossip frame accepted / published (bytes).
    pub max_message_bytes: usize,
    /// Per-peer sustained messages per second (0 = unlimited).
    pub peer_msgs_per_sec: u32,
    /// Per-peer sustained bytes per second (0 = unlimited).
    pub peer_bytes_per_sec: u64,
    /// Enable gossipsub peer scoring (and application-level message validation reporting).
    pub peer_scoring: bool,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            max_message_bytes: DEFAULT_MAX_GOSSIP_MESSAGE_BYTES,
            peer_msgs_per_sec: DEFAULT_PEER_MSGS_PER_SEC,
            peer_bytes_per_sec: DEFAULT_PEER_BYTES_PER_SEC,
            peer_scoring: true,
        }
    }
}

impl GossipConfig {
    /// Read the `BOING_P2P_*` environment variables (see `docs/ENVIRONMENT-VARIABLES.md`).
    pub fn from_env() -> Self {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// Same as [`from_env`](Self::from_env) with an injectable lookup (for tests). Unparseable values
    /// fall back to the default; the message-size limit is clamped to
    /// [`MIN_MAX_GOSSIP_MESSAGE_BYTES`]..=[`MAX_MAX_GOSSIP_MESSAGE_BYTES`].
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut c = Self::default();
        let num = |k: &str| get(k).and_then(|v| v.trim().parse::<u64>().ok());
        if let Some(v) = num(ENV_MAX_MESSAGE_BYTES) {
            c.max_message_bytes =
                (v as usize).clamp(MIN_MAX_GOSSIP_MESSAGE_BYTES, MAX_MAX_GOSSIP_MESSAGE_BYTES);
        }
        if let Some(v) = num(ENV_PEER_MSGS_PER_SEC) {
            c.peer_msgs_per_sec = v.min(u32::MAX as u64) as u32;
        }
        if let Some(v) = num(ENV_PEER_BYTES_PER_SEC) {
            c.peer_bytes_per_sec = v;
        }
        if let Some(v) = get(ENV_PEER_SCORING) {
            c.peer_scoring = !matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "false" | "off" | "no");
        }
        c
    }
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    msgs: f64,
    bytes: f64,
    last: Instant,
}

/// Token-bucket limiter keyed by peer (messages and bytes per second).
#[derive(Debug)]
pub struct PeerRateLimiter {
    msgs_per_sec: u32,
    bytes_per_sec: u64,
    buckets: HashMap<PeerId, Bucket>,
}

impl PeerRateLimiter {
    pub fn new(cfg: &GossipConfig) -> Self {
        Self {
            msgs_per_sec: cfg.peer_msgs_per_sec,
            bytes_per_sec: cfg.peer_bytes_per_sec,
            buckets: HashMap::new(),
        }
    }

    fn enabled(&self) -> bool {
        self.msgs_per_sec > 0 || self.bytes_per_sec > 0
    }

    /// Account one message of `bytes` from `peer`; `false` = over the limit (drop it).
    pub fn allow(&mut self, peer: &PeerId, bytes: usize, now: Instant) -> bool {
        if !self.enabled() {
            return true;
        }
        let msg_cap = self.msgs_per_sec as f64 * BURST_SECONDS as f64;
        let byte_cap = self.bytes_per_sec as f64 * BURST_SECONDS as f64;
        let b = self.buckets.entry(*peer).or_insert(Bucket { msgs: msg_cap, bytes: byte_cap, last: now });
        let dt = now.saturating_duration_since(b.last).as_secs_f64();
        b.last = now;
        if self.msgs_per_sec > 0 {
            b.msgs = (b.msgs + dt * self.msgs_per_sec as f64).min(msg_cap);
        }
        if self.bytes_per_sec > 0 {
            b.bytes = (b.bytes + dt * self.bytes_per_sec as f64).min(byte_cap);
        }
        let need_msg = if self.msgs_per_sec > 0 { 1.0 } else { 0.0 };
        let need_bytes = if self.bytes_per_sec > 0 { bytes as f64 } else { 0.0 };
        // A single frame bigger than the whole byte bucket can never pass; the size cap handles that.
        if b.msgs < need_msg || b.bytes < need_bytes.min(byte_cap) {
            return false;
        }
        b.msgs -= need_msg;
        b.bytes -= need_bytes.min(byte_cap);
        true
    }

    /// Forget a peer (call on disconnect so the map stays bounded).
    pub fn forget(&mut self, peer: &PeerId) {
        self.buckets.remove(peer);
    }

    /// Number of tracked peers.
    pub fn tracked_peers(&self) -> usize {
        self.buckets.len()
    }

    /// Drop peers idle for longer than `idle` (defence in depth).
    pub fn prune_idle(&mut self, now: Instant, idle: Duration) {
        self.buckets.retain(|_, b| now.saturating_duration_since(b.last) < idle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(msgs: u32, bytes: u64) -> GossipConfig {
        GossipConfig { peer_msgs_per_sec: msgs, peer_bytes_per_sec: bytes, ..GossipConfig::default() }
    }

    #[test]
    fn defaults_hold_a_full_block_and_a_max_blob() {
        let c = GossipConfig::default();
        assert!(c.max_message_bytes >= 16 * 1024 * 1024 + 1024 * 1024);
        assert!(c.max_message_bytes > MIN_MAX_GOSSIP_MESSAGE_BYTES);
        assert!(c.peer_scoring);
    }

    #[test]
    fn env_parsing_clamps_and_falls_back() {
        let env = |pairs: Vec<(&'static str, &'static str)>| {
            GossipConfig::from_lookup(move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string()))
        };
        assert_eq!(env(vec![]), GossipConfig::default());
        assert_eq!(env(vec![(ENV_MAX_MESSAGE_BYTES, "1")]).max_message_bytes, MIN_MAX_GOSSIP_MESSAGE_BYTES);
        assert_eq!(
            env(vec![(ENV_MAX_MESSAGE_BYTES, "99999999999")]).max_message_bytes,
            MAX_MAX_GOSSIP_MESSAGE_BYTES
        );
        assert_eq!(env(vec![(ENV_MAX_MESSAGE_BYTES, "2097152")]).max_message_bytes, 2 * 1024 * 1024);
        assert_eq!(env(vec![(ENV_MAX_MESSAGE_BYTES, "garbage")]).max_message_bytes, DEFAULT_MAX_GOSSIP_MESSAGE_BYTES);
        assert_eq!(env(vec![(ENV_PEER_MSGS_PER_SEC, "0")]).peer_msgs_per_sec, 0);
        assert_eq!(env(vec![(ENV_PEER_MSGS_PER_SEC, "7")]).peer_msgs_per_sec, 7);
        assert!(!env(vec![(ENV_PEER_SCORING, "off")]).peer_scoring);
        assert!(!env(vec![(ENV_PEER_SCORING, "0")]).peer_scoring);
        assert!(env(vec![(ENV_PEER_SCORING, "1")]).peer_scoring);
    }

    #[test]
    fn message_rate_limit_bursts_then_refills() {
        let mut l = PeerRateLimiter::new(&cfg(10, 0));
        let p = PeerId::random();
        let t0 = Instant::now();
        // burst = 2 seconds worth = 20 messages
        let passed = (0..40).filter(|_| l.allow(&p, 100, t0)).count();
        assert_eq!(passed, 20);
        // half a second later: 5 tokens
        let t1 = t0 + Duration::from_millis(500);
        let passed = (0..40).filter(|_| l.allow(&p, 100, t1)).count();
        assert_eq!(passed, 5);
        // other peers are independent
        assert!(l.allow(&PeerId::random(), 100, t1));
    }

    #[test]
    fn byte_rate_limit_and_unlimited_and_forget() {
        let mut l = PeerRateLimiter::new(&cfg(0, 1000));
        let p = PeerId::random();
        let t0 = Instant::now();
        assert!(l.allow(&p, 1500, t0));
        assert!(l.allow(&p, 500, t0)); // 2000 burst total
        assert!(!l.allow(&p, 100, t0));
        assert!(l.allow(&p, 100, t0 + Duration::from_millis(200)));
        assert_eq!(l.tracked_peers(), 1);
        l.forget(&p);
        assert_eq!(l.tracked_peers(), 0);

        let mut open = PeerRateLimiter::new(&cfg(0, 0));
        assert!((0..10_000).all(|_| open.allow(&p, 1 << 20, t0)));
        assert_eq!(open.tracked_peers(), 0);
    }

    #[test]
    fn oversize_single_frame_does_not_wedge_the_bucket() {
        // frame larger than the whole byte bucket is charged at most the bucket (size cap rejects it earlier)
        let mut l = PeerRateLimiter::new(&cfg(0, 100));
        let p = PeerId::random();
        let t0 = Instant::now();
        assert!(l.allow(&p, 10_000, t0));
        assert!(!l.allow(&p, 10, t0));
        assert!(l.allow(&p, 10, t0 + Duration::from_secs(3)));
    }

    #[test]
    fn prune_idle_bounds_the_map() {
        let mut l = PeerRateLimiter::new(&cfg(10, 0));
        let t0 = Instant::now();
        for _ in 0..50 {
            l.allow(&PeerId::random(), 1, t0);
        }
        l.prune_idle(t0 + Duration::from_secs(120), Duration::from_secs(60));
        assert_eq!(l.tracked_peers(), 0);
    }
}
