//! Security configuration and incident response types.
//!
//! See SECURITY-STANDARDS.md for full design.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Rate limit configuration for DDoS resistance.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Max requests per second **per peer IP** (0 = disabled).
    ///
    /// Also used as the burst size floor (2× rps, min 10) for the token bucket.
    pub requests_per_sec: u32,
    /// Max connections per IP (0 = disabled).
    pub connections_per_ip: u32,
    /// Max distinct pending nonces per sender in the mempool (applied at `boing-node` startup; HTTP RPC uses the same `RateLimitConfig` instance).
    pub pending_txs_per_sender: u32,
}

/// Per-IP token-bucket rate limiter for HTTP JSON-RPC (issue #61 item 16).
///
/// One noisy client can no longer exhaust a process-global quota shared by everyone else.
#[derive(Debug)]
pub struct IpRateLimiter {
    requests_per_sec: f64,
    burst: f64,
    inner: Mutex<HashMap<IpAddr, TokenBucket>>,
}

#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last: Instant,
}

impl IpRateLimiter {
    pub fn new(requests_per_sec: u32) -> Self {
        let rps = requests_per_sec.max(1) as f64;
        let burst = (requests_per_sec.saturating_mul(2).max(10)) as f64;
        Self {
            requests_per_sec: rps,
            burst,
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` when the request is allowed for `ip`.
    pub fn check(&self, ip: IpAddr) -> bool {
        let mut map = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let now = Instant::now();
        // Opportunistic prune so idle peers cannot grow the map forever.
        if map.len() > 10_000 {
            map.retain(|_, b| now.duration_since(b.last) < Duration::from_secs(120));
        }
        let bucket = map.entry(ip).or_insert(TokenBucket {
            tokens: self.burst,
            last: now,
        });
        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.last = now;
        bucket.tokens = (bucket.tokens + elapsed * self.requests_per_sec).min(self.burst);
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Global token bucket that caps expensive gossip-tx prechecks (issue #61 item 18).
#[derive(Debug)]
pub struct GossipPrecheckLimiter {
    per_sec: f64,
    burst: f64,
    inner: Mutex<TokenBucket>,
}

impl GossipPrecheckLimiter {
    pub fn new(per_sec: u32) -> Self {
        let per_sec = per_sec.max(1) as f64;
        let burst = (per_sec * 2.0).max(10.0);
        Self {
            per_sec,
            burst,
            inner: Mutex::new(TokenBucket {
                tokens: burst,
                last: Instant::now(),
            }),
        }
    }

    pub fn try_admit(&self) -> bool {
        let mut b = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let now = Instant::now();
        let elapsed = now.duration_since(b.last).as_secs_f64();
        b.last = now;
        b.tokens = (b.tokens + elapsed * self.per_sec).min(self.burst);
        if b.tokens >= 1.0 {
            b.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Default max expensive gossip prechecks per second (snapshot+VM dry-runs).
pub const DEFAULT_GOSSIP_EXPENSIVE_PRECHECK_PER_SEC: u32 = 50;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn ip_rate_limiter_isolates_peers() {
        let lim = IpRateLimiter::new(2);
        let a = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let b = IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8));
        assert!(lim.check(a));
        assert!(lim.check(a));
        // Burst is 2*rps=4, so a few more may pass; drain until a is blocked.
        let mut blocked_a = false;
        for _ in 0..20 {
            if !lim.check(a) {
                blocked_a = true;
                break;
            }
        }
        assert!(blocked_a, "peer A should eventually be rate-limited");
        assert!(lim.check(b), "peer B must keep its own bucket");
    }

    #[test]
    fn gossip_precheck_limiter_exhausts() {
        let lim = GossipPrecheckLimiter::new(1);
        let mut admitted = 0u32;
        for _ in 0..20 {
            if lim.try_admit() {
                admitted += 1;
            }
        }
        assert!(admitted >= 1);
        assert!(admitted < 20, "limiter must refuse after burst");
    }
}

impl RateLimitConfig {
    pub fn default_mainnet() -> Self {
        Self {
            requests_per_sec: 100,
            connections_per_ip: 50,
            pending_txs_per_sender: 16,
        }
    }

    pub fn default_devnet() -> Self {
        Self {
            requests_per_sec: 1000,
            connections_per_ip: 100,
            pending_txs_per_sender: 64,
        }
    }
}

/// Severity level for security incidents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Incident report (for internal tracking and response).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IncidentReport {
    pub severity: IncidentSeverity,
    pub summary: String,
    pub component: String,
    pub timestamp_secs: u64,
}
