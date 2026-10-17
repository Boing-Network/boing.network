//! Bootnode redial: keeps configured bootnodes connected.
//!
//! Bootnodes are dialed right away, and again with exponential backoff while they are not connected
//! (failed dial, remote restart, dropped TCP path). A successful connection resets the backoff; a
//! drop schedules a fast redial. Each attempt dials the configured multiaddr again, so a `/dns4`,
//! `/dns6` or `/dns` bootnode (e.g. `/dns6/<app>.internal/tcp/4001` on Fly) is re-resolved every time.
//!
//! Before this existed, bootnodes were dialed once at startup: after any drop the node stayed at
//! zero peers (and stopped syncing) until it was restarted.

use std::time::{Duration, Instant};

use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};

/// First retry delay after a drop or a failed dial.
pub const REDIAL_BACKOFF_BASE: Duration = Duration::from_secs(2);
/// Upper bound for the retry delay while a bootnode stays unreachable.
pub const REDIAL_BACKOFF_MAX: Duration = Duration::from_secs(60);

#[derive(Debug)]
struct Entry {
    addr: Multiaddr,
    /// Peer last reached through this address (bootnode identities can change on restart).
    peer: Option<PeerId>,
    connected: bool,
    backoff: Duration,
    next_attempt: Instant,
}

/// Bootnode connection state and redial schedule (pure; the swarm task drives it).
#[derive(Debug)]
pub struct BootnodeRedial {
    entries: Vec<Entry>,
    base: Duration,
    max: Duration,
}

/// `addr` without a trailing `/p2p/<peer id>`, for comparing dialed and configured addresses.
fn strip_p2p(addr: &Multiaddr) -> Multiaddr {
    addr.iter()
        .filter(|p| !matches!(p, Protocol::P2p(_)))
        .collect()
}

impl BootnodeRedial {
    /// All addresses are due immediately.
    pub fn new(addrs: Vec<Multiaddr>, now: Instant) -> Self {
        Self::with_backoff(addrs, now, REDIAL_BACKOFF_BASE, REDIAL_BACKOFF_MAX)
    }

    pub fn with_backoff(
        addrs: Vec<Multiaddr>,
        now: Instant,
        base: Duration,
        max: Duration,
    ) -> Self {
        let mut entries: Vec<Entry> = Vec::new();
        for addr in addrs {
            if entries
                .iter()
                .any(|e| strip_p2p(&e.addr) == strip_p2p(&addr))
            {
                continue;
            }
            entries.push(Entry {
                addr,
                peer: None,
                connected: false,
                backoff: base,
                next_attempt: now,
            });
        }
        Self {
            entries,
            base,
            max: max.max(base),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bootnode addresses that currently have a live connection.
    pub fn connected_count(&self) -> usize {
        self.entries.iter().filter(|e| e.connected).count()
    }

    /// Addresses to dial now. Each returned address is not retried before its backoff elapses, and
    /// its backoff doubles (capped) until a connection through it succeeds.
    pub fn due(&mut self, now: Instant) -> Vec<Multiaddr> {
        let mut out = Vec::new();
        for e in &mut self.entries {
            if e.connected || now < e.next_attempt {
                continue;
            }
            out.push(e.addr.clone());
            e.next_attempt = now + e.backoff;
            e.backoff = (e.backoff * 2).min(self.max);
        }
        out
    }

    /// An outbound dial to `dialed` connected to `peer`.
    pub fn on_dialed_connection(&mut self, dialed: &Multiaddr, peer: PeerId) {
        let dialed = strip_p2p(dialed);
        for e in &mut self.entries {
            if strip_p2p(&e.addr) == dialed {
                e.peer = Some(peer);
                e.connected = true;
                e.backoff = self.base;
            }
        }
    }

    /// Any connection (e.g. inbound) to `peer` is up: bootnodes known to be that peer are connected.
    pub fn on_peer_connected(&mut self, peer: PeerId) {
        for e in &mut self.entries {
            if e.peer == Some(peer) {
                e.connected = true;
                e.backoff = self.base;
            }
        }
    }

    /// The last connection to `peer` closed: redial its bootnode address(es) after the base delay.
    /// Returns how many bootnode addresses were affected.
    pub fn on_peer_disconnected(&mut self, peer: PeerId, now: Instant) -> usize {
        let mut n = 0;
        for e in &mut self.entries {
            if e.peer == Some(peer) && e.connected {
                e.connected = false;
                e.backoff = self.base;
                e.next_attempt = now + self.base;
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ma(s: &str) -> Multiaddr {
        s.parse().unwrap()
    }

    #[test]
    fn dials_immediately_then_backs_off_exponentially_up_to_cap() {
        let t0 = Instant::now();
        let base = Duration::from_secs(2);
        let max = Duration::from_secs(10);
        let mut r = BootnodeRedial::with_backoff(vec![ma("/ip4/10.0.0.1/tcp/4001")], t0, base, max);
        assert_eq!(r.due(t0).len(), 1);
        assert!(
            r.due(t0).is_empty(),
            "not retried before the backoff elapses"
        );
        assert!(r.due(t0 + Duration::from_millis(1999)).is_empty());
        // Attempts at +2, +2+4, +2+4+8 would exceed the cap: 2, 4, 8 -> 10, 10.
        let mut t = t0;
        let mut gaps = Vec::new();
        let mut last = t0;
        for _ in 0..5 {
            loop {
                t += Duration::from_millis(500);
                if !r.due(t).is_empty() {
                    gaps.push((t - last).as_secs());
                    last = t;
                    break;
                }
            }
        }
        assert_eq!(gaps, vec![2, 4, 8, 10, 10]);
    }

    #[test]
    fn connected_bootnode_is_not_redialed_until_it_drops() {
        let t0 = Instant::now();
        let addr = ma("/ip4/10.0.0.1/tcp/4001");
        let peer = PeerId::random();
        let mut r = BootnodeRedial::new(vec![addr.clone()], t0);
        assert_eq!(r.due(t0), vec![addr.clone()]);
        r.on_dialed_connection(&addr.clone().with(Protocol::P2p(peer)), peer);
        assert_eq!(r.connected_count(), 1);
        assert!(r.due(t0 + Duration::from_secs(3600)).is_empty());

        let t1 = t0 + Duration::from_secs(3600);
        assert_eq!(r.on_peer_disconnected(peer, t1), 1);
        assert_eq!(r.connected_count(), 0);
        assert!(
            r.due(t1).is_empty(),
            "short base delay before the first redial"
        );
        assert_eq!(r.due(t1 + REDIAL_BACKOFF_BASE), vec![addr]);
    }

    #[test]
    fn backoff_resets_after_success_and_new_identity_is_tracked() {
        let t0 = Instant::now();
        let addr = ma("/dns6/boing-testnet-1.internal/tcp/4001");
        let mut r = BootnodeRedial::new(vec![addr.clone()], t0);
        // Several failed attempts grow the backoff.
        let mut t = t0;
        for _ in 0..6 {
            r.due(t);
            t += REDIAL_BACKOFF_MAX;
        }
        let old = PeerId::random();
        r.on_dialed_connection(&addr, old);
        // Remote restarts with a new identity.
        r.on_peer_disconnected(old, t);
        assert_eq!(
            r.due(t + REDIAL_BACKOFF_BASE).len(),
            1,
            "backoff was reset to base"
        );
        let new = PeerId::random();
        r.on_dialed_connection(&addr, new);
        assert_eq!(r.connected_count(), 1);
        assert_eq!(
            r.on_peer_disconnected(old, t),
            0,
            "stale identity no longer maps to the bootnode"
        );
        assert_eq!(r.on_peer_disconnected(new, t), 1);
    }

    #[test]
    fn inbound_connection_from_known_bootnode_peer_counts_as_connected() {
        let t0 = Instant::now();
        let addr = ma("/ip4/10.0.0.1/tcp/4001");
        let peer = PeerId::random();
        let mut r = BootnodeRedial::new(vec![addr.clone()], t0);
        r.due(t0);
        r.on_dialed_connection(&addr, peer);
        r.on_peer_disconnected(peer, t0);
        r.on_peer_connected(peer);
        assert_eq!(r.connected_count(), 1);
        assert!(r.due(t0 + Duration::from_secs(600)).is_empty());
    }

    #[test]
    fn duplicate_addresses_are_collapsed() {
        let peer = PeerId::random();
        let r = BootnodeRedial::new(
            vec![
                ma("/ip4/10.0.0.1/tcp/4001"),
                ma("/ip4/10.0.0.1/tcp/4001").with(Protocol::P2p(peer)),
                ma("/ip4/10.0.0.2/tcp/4001"),
            ],
            Instant::now(),
        );
        assert_eq!(r.len(), 2);
    }
}
