//! P2P node — libp2p swarm with gossipsub, mdns, and block request/response.
//!
//! Propagates blocks and **signed** transactions; discovers peers via mDNS; fetches blocks on demand.
//! Optional **max connections per IP** (`max_connections_per_ip`, 0 = disabled) limits Sybil-style fan-in.
//!
//! Connection upkeep: configured bootnodes ([`P2pNode::set_bootnodes`]) are redialed with exponential
//! backoff whenever they are not connected (see [`crate::redial`]); `/dns*` bootnodes are re-resolved on
//! every dial; a libp2p `ping` every [`PING_INTERVAL`] detects dead links so they are closed (and redialed)
//! instead of lingering as half-open connections.

use std::collections::HashMap;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use libp2p::futures::StreamExt;
use libp2p::gossipsub::PublishError;
use libp2p::gossipsub::{IdentTopic, MessageAcceptance, MessageAuthenticity, ValidationMode};
#[cfg(feature = "mdns")]
use libp2p::mdns::tokio::Behaviour as Mdns;
use libp2p::multiaddr::Protocol;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::StreamProtocol;
use libp2p::{gossipsub, SwarmBuilder};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

use crate::block_sync::{BlockRequest, BlockResponse};
use crate::gossip_limits::{GossipConfig, PeerRateLimiter};
use crate::redial::BootnodeRedial;
use boing_primitives::{
    Block, ConsensusVote, EquivocationEvidence, Hash, MediaBlobGossip, SignedTransaction,
    VrfProofGossip, MEDIA_BLOBS_GOSSIP_TOPIC,
};

const BLOCKS_TOPIC: &str = "boing/blocks";
const TRANSACTIONS_TOPIC: &str = "boing/transactions";
const VOTES_TOPIC: &str = "boing/votes";
const EQUIVOCATION_TOPIC: &str = "boing/equivocation";
const VRF_PROOFS_TOPIC: &str = "boing/vrf-proofs";
const MEDIA_BLOBS_TOPIC: &str = MEDIA_BLOBS_GOSSIP_TOPIC;

/// Keepalive / liveness probe interval on every connection.
pub const PING_INTERVAL: Duration = Duration::from_secs(15);
/// A ping unanswered for this long marks the connection dead (it is closed, then redialed if a bootnode).
pub const PING_TIMEOUT: Duration = Duration::from_secs(20);
/// How often the swarm task checks for bootnodes due for a (re)dial.
const REDIAL_TICK: Duration = Duration::from_millis(500);

/// Read-only connectivity snapshot (for health / readiness reporting).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct P2pStatus {
    /// Distinct peers with at least one established connection.
    pub connected_peers: usize,
    /// Configured bootnode addresses (after de-duplication).
    pub bootnodes: usize,
    /// Bootnode addresses that currently have a live connection.
    pub bootnodes_connected: usize,
}

/// P2P events (incoming blocks/transactions/votes and block fetch responses).
#[derive(Debug)]
pub enum P2pEvent {
    BlockReceived(Block),
    /// Gossiped signed transaction from a peer (verify before mempool insert).
    TransactionReceived(SignedTransaction),
    /// Gossiped consensus vote (signature verified at the P2P edge).
    VoteReceived(ConsensusVote),
    /// Gossiped equivocation evidence (both votes verified at the P2P edge).
    EquivocationReceived(EquivocationEvidence),
    /// Gossiped per-validator ECVRF proof (verified at the P2P edge).
    VrfProofReceived(VrfProofGossip),
    /// Gossiped media blob bytes (Phase 2 data plane; id verified = BLAKE3(data)).
    MediaBlobReceived(MediaBlobGossip),
    /// Response from request_block (by hash or height).
    BlockFetched(Block),
}

enum BroadcastMsg {
    Block(Block),
    SignedTransaction(SignedTransaction),
    Vote(ConsensusVote),
    Equivocation(EquivocationEvidence),
    VrfProof(VrfProofGossip),
    MediaBlob(MediaBlobGossip),
}

enum Command {
    RequestBlock(libp2p::PeerId, BlockRequest),
    GetPeers(oneshot::Sender<Vec<libp2p::PeerId>>),
    Dial(String),
    SetBootnodes(Vec<libp2p::Multiaddr>),
    Status(oneshot::Sender<P2pStatus>),
}

/// Provides blocks for the request/response protocol.
pub trait BlockProvider: Send + Sync {
    fn get_block_by_hash(&self, hash: &Hash) -> Option<Block>;
    fn get_block_by_height(&self, height: u64) -> Option<Block>;
}

type BlockSyncBehaviour = request_response::cbor::Behaviour<BlockRequest, BlockResponse>;

/// Extract a remote IP for the per-IP connection cap.
///
/// Literal `/ip4` / `/ip6` components win. `/dns`, `/dns4`, and `/dns6` are resolved best-effort so
/// DNS-dialed peers (including bootnodes) share the same cap as numeric multiaddrs — otherwise a
/// peer could bypass the limit by advertising only a hostname.
fn ip_from_multiaddr(ma: &libp2p::Multiaddr) -> Option<IpAddr> {
    let mut dns_name: Option<String> = None;
    for p in ma.iter() {
        match p {
            Protocol::Ip4(ip) => return Some(IpAddr::V4(ip)),
            Protocol::Ip6(ip) => return Some(IpAddr::V6(ip)),
            Protocol::Dns(name) | Protocol::Dns4(name) | Protocol::Dns6(name) => {
                dns_name = Some(name.to_string());
            }
            _ => {}
        }
    }
    let name = dns_name?;
    // Port is irrelevant for the A/AAAA lookup; `ToSocketAddrs` still requires one.
    (name.as_str(), 0u16)
        .to_socket_addrs()
        .ok()?
        .next()
        .map(|sa| sa.ip())
}

/// Parse a comma-separated listen multiaddr list. Empty / unparseable entries fail startup instead of
/// panicking inside the swarm task after [`P2pNode::new`] has already returned `Ok`.
fn parse_listen_addrs(listen_addr: &str) -> Result<Vec<libp2p::Multiaddr>, P2pError> {
    let mut out = Vec::new();
    for one in listen_addr
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let ma = one
            .parse::<libp2p::Multiaddr>()
            .map_err(|e| P2pError::Network(format!("invalid P2P listen address {one:?}: {e}")))?;
        out.push(ma);
    }
    if out.is_empty() {
        return Err(P2pError::Network(
            "P2P listen address is empty (expected one multiaddr or a comma-separated list)".into(),
        ));
    }
    Ok(out)
}

fn direction(endpoint: &libp2p_core::connection::ConnectedPoint) -> &'static str {
    if endpoint.is_dialer() {
        "outbound"
    } else {
        "inbound"
    }
}

fn remote_multiaddr(endpoint: &libp2p_core::connection::ConnectedPoint) -> &libp2p::Multiaddr {
    match endpoint {
        libp2p_core::connection::ConnectedPoint::Dialer { address, .. } => address,
        libp2p_core::connection::ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr,
    }
}

#[cfg(feature = "mdns")]
#[derive(NetworkBehaviour)]
#[behaviour(prelude = "libp2p_swarm::derive_prelude")]
struct BoingBehaviour {
    mdns: Mdns,
    gossipsub: gossipsub::Behaviour,
    block_sync: BlockSyncBehaviour,
    ping: libp2p::ping::Behaviour,
}

#[cfg(not(feature = "mdns"))]
#[derive(NetworkBehaviour)]
#[behaviour(prelude = "libp2p_swarm::derive_prelude")]
struct BoingBehaviour {
    gossipsub: gossipsub::Behaviour,
    block_sync: BlockSyncBehaviour,
    ping: libp2p::ping::Behaviour,
}

/// Gossipsub peer-score parameters.
///
/// Only *invalid* messages (undecodable / bad signature) are penalised: the mesh-delivery and
/// mesh-failure penalties are disabled so quiet topics on a small testnet do not score honest peers
/// down, and IP-colocation is disabled because operators legitimately run several nodes per host.
/// Thresholds are the libp2p defaults (gossip -10, publish -50, graylist -80), so a peer that sends a
/// few bad messages is graylisted and stops being relayed.
pub(crate) fn peer_score_params() -> (gossipsub::PeerScoreParams, gossipsub::PeerScoreThresholds) {
    let topic_params = gossipsub::TopicScoreParams {
        topic_weight: 1.0,
        mesh_message_deliveries_weight: 0.0,
        mesh_failure_penalty_weight: 0.0,
        invalid_message_deliveries_weight: -10.0,
        invalid_message_deliveries_decay: 0.3,
        ..Default::default()
    };
    let mut params = gossipsub::PeerScoreParams {
        ip_colocation_factor_weight: 0.0,
        ..Default::default()
    };
    for t in [
        BLOCKS_TOPIC,
        TRANSACTIONS_TOPIC,
        VOTES_TOPIC,
        EQUIVOCATION_TOPIC,
        VRF_PROOFS_TOPIC,
        MEDIA_BLOBS_TOPIC,
    ] {
        params
            .topics
            .insert(IdentTopic::new(t).hash(), topic_params.clone());
    }
    (params, gossipsub::PeerScoreThresholds::default())
}

/// P2P node handle. Broadcasts blocks/txs; emits P2pEvent for incoming data.
/// Use `inert()` for tests when no Tokio runtime is available.
#[derive(Clone)]
pub struct P2pNode {
    broadcast_tx: Option<mpsc::Sender<BroadcastMsg>>,
    cmd_tx: Option<mpsc::Sender<Command>>,
}

impl P2pNode {
    /// Create a P2P node and spawn the swarm task.
    /// `listen_addr` is one multiaddr or a comma-separated list (one listener each).
    /// Returns the node handle and a receiver for incoming P2pEvent.
    /// When `block_provider` is provided, enables block request/response protocol.
    ///
    /// `max_connections_per_ip`: cap simultaneous connections sharing the same remote IP (IPv4/IPv6).
    /// **0** disables the limit. Applies to established connections (best-effort Sybil / fan-in control).
    pub fn new(
        listen_addr: &str,
        block_provider: Option<Arc<dyn BlockProvider>>,
        max_connections_per_ip: u32,
    ) -> Result<(Self, mpsc::UnboundedReceiver<P2pEvent>), P2pError> {
        Self::new_with_gossip(
            listen_addr,
            block_provider,
            max_connections_per_ip,
            GossipConfig::from_env(),
        )
    }

    /// Like [`new`](Self::new) with explicit gossip limits (max frame size, per-peer rate limits, peer scoring).
    /// `new` reads them from the `BOING_P2P_*` environment ([`GossipConfig::from_env`]).
    pub fn new_with_gossip(
        listen_addr: &str,
        block_provider: Option<Arc<dyn BlockProvider>>,
        max_connections_per_ip: u32,
        gossip: GossipConfig,
    ) -> Result<(Self, mpsc::UnboundedReceiver<P2pEvent>), P2pError> {
        // Validate listen multiaddrs *before* spawning so a bad `--p2p-listen` returns Err to the
        // caller instead of panicking inside the swarm task after this method has returned Ok.
        let listen_addrs = parse_listen_addrs(listen_addr)?;
        let listen_addr_display = listen_addr.to_string();

        let gossip_for_behaviour = gossip.clone();
        // Larger than the historical 64 so vote bursts after a proposal are less likely to drop;
        // try_send failures are still logged (see broadcast_* helpers).
        let (broadcast_tx, mut broadcast_rx) = mpsc::channel(256);
        let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
        // Unbounded so the swarm task never awaits on backpressure when forwarding gossip / blocks
        // to the node (bounded channels can stall libp2p and break mesh + tx propagation).
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                Default::default(),
                (libp2p::tls::Config::new, libp2p::noise::Config::new),
                libp2p::yamux::Config::default,
            )
            .map_err(|e| P2pError::Network(e.to_string()))?
            // Resolve `/dns4`, `/dns6`, `/dns` multiaddrs at dial time (re-resolved on every redial).
            .with_dns()
            .map_err(|e| P2pError::Network(e.to_string()))?
            .with_behaviour(|key| {
                let mut cfg_builder = gossipsub::ConfigBuilder::default();
                cfg_builder
                    .heartbeat_interval(Duration::from_secs(1))
                    // Strict matches MessageAuthenticity::Signed: reject unsigned / malformed
                    // author fields at the protocol layer (Permissive would accept them).
                    .validation_mode(ValidationMode::Strict)
                    .max_transmit_size(gossip_for_behaviour.max_message_bytes);
                if gossip_for_behaviour.peer_scoring {
                    // Hold forwarding until the application reports Accept/Reject/Ignore so invalid
                    // messages are never relayed and feed the sender's score.
                    cfg_builder.validate_messages();
                }
                let gossipsub_config = cfg_builder
                    .build()
                    .map_err(|e| Box::new(std::io::Error::other(e.to_string())))?;
                let mut gossipsub = gossipsub::Behaviour::new(
                    MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .map_err(|e| Box::new(std::io::Error::other(e.to_string())))?;
                if gossip_for_behaviour.peer_scoring {
                    let (params, thresholds) = peer_score_params();
                    gossipsub
                        .with_peer_score(params, thresholds)
                        .map_err(|e| Box::new(std::io::Error::other(e)))?;
                }
                let sync_limits = crate::block_sync::BlockSyncLimits::from_env();
                // Requests are tiny (hash/height); responses carry full blocks — set both explicitly
                // rather than relying on libp2p's 1 MiB / 10 MiB defaults.
                let block_sync = BlockSyncBehaviour::with_codec(
                    request_response::cbor::codec::Codec::default()
                        .set_request_size_maximum(sync_limits.max_request_bytes)
                        .set_response_size_maximum(sync_limits.max_response_bytes),
                    [(
                        StreamProtocol::new("/boing/block-sync/1"),
                        ProtocolSupport::Full,
                    )],
                    request_response::Config::default().with_request_timeout(Duration::from_secs(
                        sync_limits.request_timeout_secs,
                    )),
                );
                let ping = libp2p::ping::Behaviour::new(
                    libp2p::ping::Config::new()
                        .with_interval(PING_INTERVAL)
                        .with_timeout(PING_TIMEOUT),
                );
                #[cfg(feature = "mdns")]
                {
                    let peer_id = key.public().to_peer_id();
                    let mdns = Mdns::new(libp2p::mdns::Config::default(), peer_id)
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>(BoingBehaviour {
                        mdns,
                        gossipsub,
                        block_sync,
                        ping,
                    })
                }
                #[cfg(not(feature = "mdns"))]
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(BoingBehaviour {
                    gossipsub,
                    block_sync,
                    ping,
                })
            })
            .map_err(|e| P2pError::Network(e.to_string()))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        let blocks_topic = IdentTopic::new(BLOCKS_TOPIC);
        let txs_topic = IdentTopic::new(TRANSACTIONS_TOPIC);
        let votes_topic = IdentTopic::new(VOTES_TOPIC);
        let equivocation_topic = IdentTopic::new(EQUIVOCATION_TOPIC);
        let vrf_proofs_topic = IdentTopic::new(VRF_PROOFS_TOPIC);
        let media_blobs_topic = IdentTopic::new(MEDIA_BLOBS_TOPIC);

        // Bind listeners and subscribe before spawn so startup failures surface to the caller.
        for ma in &listen_addrs {
            swarm
                .listen_on(ma.clone())
                .map_err(|e| P2pError::Network(format!("P2P listen_on {ma} failed: {e}")))?;
        }
        for (topic, label) in [
            (&blocks_topic, "blocks"),
            (&txs_topic, "txs"),
            (&votes_topic, "votes"),
            (&equivocation_topic, "equivocation"),
            (&vrf_proofs_topic, "vrf-proofs"),
            (&media_blobs_topic, "media-blobs"),
        ] {
            swarm
                .behaviour_mut()
                .gossipsub
                .subscribe(topic)
                .map_err(|e| P2pError::Network(format!("subscribe {label}: {e}")))?;
        }

        info!(
            "P2P: listening on {} peer_id={:?}",
            listen_addr_display,
            swarm.local_peer_id()
        );

        let block_provider = block_provider;

        tokio::spawn(async move {
            let mut swarm = swarm;
            let mut peer_ips: HashMap<libp2p::PeerId, IpAddr> = HashMap::new();
            let mut ip_connection_count: HashMap<IpAddr, u32> = HashMap::new();
            let mut redial = BootnodeRedial::new(Vec::new(), Instant::now());
            let mut gossip_limiter = PeerRateLimiter::new(&gossip);
            let report_validation = gossip.peer_scoring;
            let mut redial_tick = tokio::time::interval(REDIAL_TICK);
            redial_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(Command::RequestBlock(peer, req)) => {
                                swarm.behaviour_mut().block_sync.send_request(&peer, req);
                            }
                            Some(Command::GetPeers(tx)) => {
                                let peers: Vec<_> = swarm.connected_peers().cloned().collect();
                                let _ = tx.send(peers);
                            }
                            Some(Command::Dial(addr)) => {
                                if let Ok(ma) = addr.parse::<libp2p::Multiaddr>() {
                                    if let Err(e) = swarm.dial(ma) {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "dial_failed",
                                            format!("{addr}: {e:?}"),
                                        );
                                    }
                                }
                            }
                            Some(Command::SetBootnodes(addrs)) => {
                                redial = BootnodeRedial::new(addrs, Instant::now());
                                info!(
                                    "P2P: maintaining {} bootnode address(es) (redial with backoff {:?}..{:?})",
                                    redial.len(),
                                    crate::redial::REDIAL_BACKOFF_BASE,
                                    crate::redial::REDIAL_BACKOFF_MAX
                                );
                            }
                            Some(Command::Status(tx)) => {
                                let _ = tx.send(P2pStatus {
                                    connected_peers: swarm.connected_peers().count(),
                                    bootnodes: redial.len(),
                                    bootnodes_connected: redial.connected_count(),
                                });
                            }
                            None => break,
                        }
                    }
                    _ = redial_tick.tick(), if !redial.is_empty() => {
                        for addr in redial.due(Instant::now()) {
                            // Addresses come from operator config; log only that a dial happened.
                            match swarm.dial(addr) {
                                Ok(()) => debug!(
                                    "P2P: dialing bootnode ({} of {} connected, peers={})",
                                    redial.connected_count(),
                                    redial.len(),
                                    swarm.connected_peers().count()
                                ),
                                Err(e) => boing_telemetry::component_warn(
                                    "boing_p2p::swarm",
                                    "p2p",
                                    "bootnode_dial_failed",
                                    format!("{e}"),
                                ),
                            }
                        }
                    }
                    msg = broadcast_rx.recv() => {
                        match msg {
                            Some(BroadcastMsg::Block(block)) => {
                                match bincode::serialize(&block) {
                                    Ok(bytes) => {
                                        if let Err(e) = swarm
                                            .behaviour_mut()
                                            .gossipsub
                                            .publish(blocks_topic.clone(), bytes)
                                        {
                                            // Gossipsub returns NoPeersSubscribedToTopic when no remote peer has advertised
                                            // subscription to `boing/blocks` yet (common for a few heartbeats after
                                            // connect). Local consensus and RPC are unaffected.
                                            if matches!(e, PublishError::NoPeersSubscribedToTopic) {
                                                boing_telemetry::component_debug(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_block_no_subscribed_peers",
                                                    "block not gossip-published yet (no subscribed peers); peers may still catch up via block-sync",
                                                );
                                            } else {
                                                boing_telemetry::component_warn(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_block_publish_failed",
                                                    e,
                                                );
                                            }
                                        } else {
                                            info!("P2P: broadcast block height={}", block.header.height);
                                        }
                                    }
                                    Err(e) => {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "block_serialize_failed",
                                            e,
                                        );
                                    }
                                }
                            }
                            Some(BroadcastMsg::SignedTransaction(signed)) => {
                                match bincode::serialize(&signed) {
                                    Ok(bytes) => {
                                        if let Err(e) = swarm
                                            .behaviour_mut()
                                            .gossipsub
                                            .publish(txs_topic.clone(), bytes)
                                        {
                                            if matches!(e, PublishError::NoPeersSubscribedToTopic) {
                                                boing_telemetry::component_debug(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_tx_no_subscribed_peers",
                                                    "tx not gossip-published yet (no subscribed peers)",
                                                );
                                            } else {
                                                boing_telemetry::component_warn(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_tx_publish_failed",
                                                    e,
                                                );
                                            }
                                        } else {
                                            info!("P2P: broadcast signed tx from {:?}", signed.tx.sender);
                                        }
                                    }
                                    Err(e) => {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "signed_tx_serialize_failed",
                                            e,
                                        );
                                    }
                                }
                            }
                            Some(BroadcastMsg::Vote(vote)) => {
                                match bincode::serialize(&vote) {
                                    Ok(bytes) => {
                                        if let Err(e) = swarm
                                            .behaviour_mut()
                                            .gossipsub
                                            .publish(votes_topic.clone(), bytes)
                                        {
                                            if matches!(e, PublishError::NoPeersSubscribedToTopic) {
                                                boing_telemetry::component_debug(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_vote_no_subscribed_peers",
                                                    "vote not gossip-published yet (no subscribed peers)",
                                                );
                                            } else {
                                                boing_telemetry::component_warn(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_vote_publish_failed",
                                                    e,
                                                );
                                            }
                                        } else {
                                            info!(
                                                "P2P: broadcast vote round={} validator={:?}",
                                                vote.round, vote.validator
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "vote_serialize_failed",
                                            e,
                                        );
                                    }
                                }
                            }
                            Some(BroadcastMsg::Equivocation(ev)) => {
                                match bincode::serialize(&ev) {
                                    Ok(bytes) => {
                                        if let Err(e) = swarm
                                            .behaviour_mut()
                                            .gossipsub
                                            .publish(equivocation_topic.clone(), bytes)
                                        {
                                            if matches!(e, PublishError::NoPeersSubscribedToTopic) {
                                                boing_telemetry::component_debug(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_equivocation_no_subscribed_peers",
                                                    "equivocation not gossip-published yet (no subscribed peers)",
                                                );
                                            } else {
                                                boing_telemetry::component_warn(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_equivocation_publish_failed",
                                                    e,
                                                );
                                            }
                                        } else {
                                            info!(
                                                "P2P: broadcast equivocation round={} validator={:?}",
                                                ev.round(),
                                                ev.validator()
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "equivocation_serialize_failed",
                                            e,
                                        );
                                    }
                                }
                            }
                            Some(BroadcastMsg::VrfProof(proof)) => {
                                match bincode::serialize(&proof) {
                                    Ok(bytes) => {
                                        if let Err(e) = swarm
                                            .behaviour_mut()
                                            .gossipsub
                                            .publish(vrf_proofs_topic.clone(), bytes)
                                        {
                                            if matches!(e, PublishError::NoPeersSubscribedToTopic) {
                                                boing_telemetry::component_debug(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_vrf_proof_no_subscribed_peers",
                                                    "vrf proof not gossip-published yet (no subscribed peers)",
                                                );
                                            } else {
                                                boing_telemetry::component_warn(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_vrf_proof_publish_failed",
                                                    e,
                                                );
                                            }
                                        } else {
                                            info!(
                                                "P2P: broadcast vrf proof round={} validator={:?}",
                                                proof.round, proof.validator
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "vrf_proof_serialize_failed",
                                            e,
                                        );
                                    }
                                }
                            }
                            Some(BroadcastMsg::MediaBlob(blob)) => {
                                match bincode::serialize(&blob) {
                                    Ok(bytes) => {
                                        if let Err(e) = swarm
                                            .behaviour_mut()
                                            .gossipsub
                                            .publish(media_blobs_topic.clone(), bytes)
                                        {
                                            if matches!(e, PublishError::NoPeersSubscribedToTopic) {
                                                boing_telemetry::component_debug(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_media_blob_no_subscribed_peers",
                                                    "media blob not gossip-published yet (no subscribed peers)",
                                                );
                                            } else {
                                                boing_telemetry::component_warn(
                                                    "boing_p2p::swarm",
                                                    "p2p",
                                                    "gossip_media_blob_publish_failed",
                                                    e,
                                                );
                                            }
                                        } else {
                                            info!(
                                                "P2P: broadcast media blob size={}",
                                                blob.data.len()
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "media_blob_serialize_failed",
                                            e,
                                        );
                                    }
                                }
                            }
                            None => break,
                        }
                    }
                    ev = swarm.select_next_some() => {
                        // Connection bookkeeping (bootnode redial, liveness, operator-visible logs).
                        match &ev {
                            SwarmEvent::ConnectionEstablished {
                                peer_id,
                                endpoint,
                                num_established,
                                ..
                            } => {
                                // Enforce the per-IP cap *before* marking bootnodes connected, so a
                                // rejected peer is not counted as live and immediately redialed.
                                let mut rejected_ip_cap = false;
                                if max_connections_per_ip > 0 && !peer_ips.contains_key(peer_id) {
                                    let ma = remote_multiaddr(endpoint);
                                    if let Some(ip) = ip_from_multiaddr(ma) {
                                        let cnt = *ip_connection_count.get(&ip).unwrap_or(&0);
                                        if cnt >= max_connections_per_ip {
                                            boing_telemetry::component_warn(
                                                "boing_p2p::swarm",
                                                "p2p",
                                                "connection_rejected_ip_cap",
                                                format!(
                                                    "peer={peer_id:?} remote_ip={ip} established_from_ip={cnt} limit={max_connections_per_ip}"
                                                ),
                                            );
                                            let _ = swarm.disconnect_peer_id(*peer_id);
                                            rejected_ip_cap = true;
                                        } else {
                                            *ip_connection_count.entry(ip).or_insert(0) += 1;
                                            peer_ips.insert(*peer_id, ip);
                                        }
                                    }
                                }
                                if !rejected_ip_cap {
                                    if let libp2p_core::connection::ConnectedPoint::Dialer {
                                        address,
                                        ..
                                    } = endpoint
                                    {
                                        redial.on_dialed_connection(address, *peer_id);
                                    }
                                    redial.on_peer_connected(*peer_id);
                                    info!(
                                        "P2P: connection established peer={} direction={} connections_to_peer={} peers={} bootnodes_connected={}/{}",
                                        peer_id,
                                        direction(endpoint),
                                        num_established,
                                        swarm.connected_peers().count(),
                                        redial.connected_count(),
                                        redial.len()
                                    );
                                }
                            }
                            SwarmEvent::ConnectionClosed {
                                peer_id,
                                endpoint,
                                num_established,
                                cause,
                                ..
                            } => {
                                if *num_established == 0 {
                                    // Only drop the IP accounting when the peer has no remaining
                                    // connections (an earlier bug decremented on every close).
                                    gossip_limiter.forget(peer_id);
                                    if max_connections_per_ip > 0 {
                                        if let Some(ip) = peer_ips.remove(peer_id) {
                                            if let Some(c) = ip_connection_count.get_mut(&ip) {
                                                *c = c.saturating_sub(1);
                                                if *c == 0 {
                                                    ip_connection_count.remove(&ip);
                                                }
                                            }
                                        }
                                    }
                                    let bootnode_addrs =
                                        redial.on_peer_disconnected(*peer_id, Instant::now());
                                    let peers = swarm.connected_peers().count();
                                    let cause = cause
                                        .as_ref()
                                        .map(|c| c.to_string())
                                        .unwrap_or_else(|| "closed".to_string());
                                    if peers == 0 {
                                        warn!(
                                            "P2P: peer disconnected peer={} direction={} cause={} peers=0 bootnode_redial={}",
                                            peer_id,
                                            direction(endpoint),
                                            cause,
                                            bootnode_addrs > 0 || !redial.is_empty()
                                        );
                                    } else {
                                        info!(
                                            "P2P: peer disconnected peer={} direction={} cause={} peers={}",
                                            peer_id,
                                            direction(endpoint),
                                            cause,
                                            peers
                                        );
                                    }
                                } else {
                                    debug!(
                                        "P2P: extra connection closed peer={} remaining={} cause={:?}",
                                        peer_id, num_established, cause
                                    );
                                }
                            }
                            SwarmEvent::OutgoingConnectionError { error, .. } => {
                                debug!("P2P: outgoing connection failed: {}", error);
                            }
                            SwarmEvent::Behaviour(BoingBehaviourEvent::Ping(libp2p::ping::Event {
                                peer,
                                connection,
                                result,
                            })) => match result {
                                Ok(rtt) => debug!("P2P: ping peer={} rtt={:?}", peer, rtt),
                                // Older nodes without ping: keep the connection.
                                Err(libp2p::ping::Failure::Unsupported) => {
                                    debug!("P2P: peer={} does not support ping", peer)
                                }
                                Err(e) => {
                                    warn!(
                                        "P2P: ping failed peer={} ({}); closing the connection so it can be redialed",
                                        peer, e
                                    );
                                    swarm.close_connection(*connection);
                                }
                            },
                            _ => {}
                        }
                        if let SwarmEvent::Behaviour(BoingBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                                propagation_source, message_id, message,
                            })) = ev {
                            let topic = message.topic.as_str();
                            // Default Accept; decode / verify failures below flip it to Reject, and a
                            // peer over its rate budget is Ignore (dropped, not relayed, mild score effect).
                            let mut acceptance = MessageAcceptance::Accept;
                            if !gossip_limiter.allow(&propagation_source, message.data.len(), Instant::now()) {
                                acceptance = MessageAcceptance::Ignore;
                                boing_telemetry::component_warn(
                                    "boing_p2p::swarm",
                                    "p2p",
                                    "gossip_peer_rate_limited",
                                    format!("peer={propagation_source:?} topic={topic} bytes={}", message.data.len()),
                                );
                            } else if topic == BLOCKS_TOPIC {
                                if let Ok(block) = boing_primitives::Block::from_bincode(&message.data) {
                                    let _ = event_tx.send(P2pEvent::BlockReceived(block));
                                } else {
                                    acceptance = MessageAcceptance::Reject;
                                }
                            } else if topic == TRANSACTIONS_TOPIC {
                                if let Ok(signed) = SignedTransaction::from_bincode(&message.data)
                                {
                                    let _ = event_tx.send(P2pEvent::TransactionReceived(signed));
                                } else {
                                    acceptance = MessageAcceptance::Reject;
                                }
                            } else if topic == VOTES_TOPIC {
                            acceptance = MessageAcceptance::Reject;
                                if let Ok(vote) = bincode::deserialize::<ConsensusVote>(&message.data) {
                                    if vote.verify().is_ok() {
                                        let _ = event_tx.send(P2pEvent::VoteReceived(vote));
                                        acceptance = MessageAcceptance::Accept;
                                    } else {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "gossip_vote_bad_signature",
                                            format!("round={} validator={:?}", vote.round, vote.validator),
                                        );
                                    }
                                }
                            } else if topic == EQUIVOCATION_TOPIC {
                            acceptance = MessageAcceptance::Reject;
                                if let Ok(ev) =
                                    bincode::deserialize::<EquivocationEvidence>(&message.data)
                                {
                                    if ev.verify().is_ok() {
                                        let _ = event_tx.send(P2pEvent::EquivocationReceived(ev));
                                        acceptance = MessageAcceptance::Accept;
                                    } else {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "gossip_equivocation_invalid",
                                            format!(
                                                "round={} validator={:?}",
                                                ev.round(),
                                                ev.validator()
                                            ),
                                        );
                                    }
                                }
                            } else if topic == VRF_PROOFS_TOPIC {
                            acceptance = MessageAcceptance::Reject;
                                if let Ok(proof) =
                                    bincode::deserialize::<VrfProofGossip>(&message.data)
                                {
                                    if proof.verify() {
                                        let _ = event_tx.send(P2pEvent::VrfProofReceived(proof));
                                        acceptance = MessageAcceptance::Accept;
                                    } else {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "gossip_vrf_proof_invalid",
                                            format!(
                                                "round={} validator={:?}",
                                                proof.round, proof.validator
                                            ),
                                        );
                                    }
                                }
                            } else if topic == MEDIA_BLOBS_TOPIC {
                            acceptance = MessageAcceptance::Reject;
                                if let Ok(blob) =
                                    bincode::deserialize::<MediaBlobGossip>(&message.data)
                                {
                                    if blob.validate().is_ok() {
                                        let _ = event_tx.send(P2pEvent::MediaBlobReceived(blob));
                                        acceptance = MessageAcceptance::Accept;
                                    } else {
                                        boing_telemetry::component_warn(
                                            "boing_p2p::swarm",
                                            "p2p",
                                            "gossip_media_blob_invalid",
                                            "blob id / size / content_type failed validation",
                                        );
                                    }
                                }
                            } else {
                                acceptance = MessageAcceptance::Ignore;
                            }
                            if report_validation {
                                let _ = swarm.behaviour_mut().gossipsub.report_message_validation_result(
                                    &message_id,
                                    &propagation_source,
                                    acceptance,
                                );
                            }
                        } else if let SwarmEvent::Behaviour(BoingBehaviourEvent::BlockSync(
                            request_response::Event::Message {
                                message: request_response::Message::Response { response, .. },
                                ..
                            },
                        )) = ev
                        {
                            if let Some(block) = response.0 {
                                let _ = event_tx.send(P2pEvent::BlockFetched(block));
                            }
                        } else if let SwarmEvent::Behaviour(BoingBehaviourEvent::BlockSync(
                            request_response::Event::Message {
                                peer: _peer,
                                message: request_response::Message::Request { request, channel, .. },
                                ..
                            },
                        )) = ev
                        {
                            if let Some(ref provider) = block_provider {
                                let block = match &request {
                                    BlockRequest::ByHash(h) => {
                                        let hash = Hash(*h);
                                        provider.get_block_by_hash(&hash)
                                    }
                                    BlockRequest::ByHeight(h) => provider.get_block_by_height(*h),
                                };
                                let resp = BlockResponse(block);
                                if let Err(e) = swarm.behaviour_mut().block_sync.send_response(channel, resp) {
                                    boing_telemetry::component_warn(
                                        "boing_p2p::swarm",
                                        "p2p",
                                        "block_sync_response_send_failed",
                                        format!("{e:?}"),
                                    );
                                }
                            }
                        } else if let SwarmEvent::NewListenAddr { address, .. } = ev {
                            info!("P2P: listening on {}", address);
                        }
                    }
                }
            }
        });

        Ok((
            Self {
                broadcast_tx: Some(broadcast_tx),
                cmd_tx: Some(cmd_tx),
            },
            event_rx,
        ))
    }

    /// Create an inert P2P node (no network). Use in tests without a Tokio runtime.
    pub fn inert() -> Self {
        Self {
            broadcast_tx: None,
            cmd_tx: None,
        }
    }

    /// Returns connected peers. For inert nodes, returns empty vec.
    pub async fn connected_peers(&self) -> Vec<libp2p::PeerId> {
        if let Some(ref ch) = self.cmd_tx {
            let (tx, rx) = oneshot::channel();
            if ch.send(Command::GetPeers(tx)).await.is_ok() {
                if let Ok(peers) = rx.await {
                    return peers;
                }
            }
        }
        vec![]
    }

    /// Read-only connectivity snapshot. Inert nodes report all zeros.
    pub async fn status(&self) -> P2pStatus {
        if let Some(ref ch) = self.cmd_tx {
            let (tx, rx) = oneshot::channel();
            if ch.send(Command::Status(tx)).await.is_ok() {
                if let Ok(status) = rx.await {
                    return status;
                }
            }
        }
        P2pStatus::default()
    }

    /// Keep these bootnodes connected: dial each now, and redial with exponential backoff whenever it
    /// is not connected (failed dial, dropped connection, remote restart). Replaces any previous set.
    /// `/dns4`, `/dns6` and `/dns` addresses are re-resolved on every dial. Invalid entries are
    /// skipped with a warning; returns how many addresses were accepted.
    pub fn set_bootnodes<S: AsRef<str>>(&self, addrs: &[S]) -> Result<usize, P2pError> {
        let mut parsed = Vec::new();
        for (i, a) in addrs.iter().enumerate() {
            let a = a.as_ref().trim();
            if a.is_empty() {
                continue;
            }
            match a.parse::<libp2p::Multiaddr>() {
                Ok(ma) => parsed.push(ma),
                Err(e) => boing_telemetry::component_warn(
                    "boing_p2p::swarm",
                    "p2p",
                    "bootnode_invalid",
                    format!("entry #{i}: {e}"),
                ),
            }
        }
        let n = parsed.len();
        if let Some(ref ch) = self.cmd_tx {
            ch.try_send(Command::SetBootnodes(parsed))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(n)
    }

    /// Dial a peer by multiaddress once (e.g. "/ip4/127.0.0.1/tcp/4001"); no redial.
    /// Use [`P2pNode::set_bootnodes`] for peers that should stay connected.
    pub fn dial(&self, addr: &str) -> Result<(), P2pError> {
        if let Some(ref ch) = self.cmd_tx {
            ch.try_send(Command::Dial(addr.to_string()))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
    }

    /// Request a block from a peer. Response arrives via P2pEvent::BlockFetched.
    pub fn request_block(
        &self,
        peer: libp2p::PeerId,
        request: BlockRequest,
    ) -> Result<(), P2pError> {
        if let Some(ref ch) = self.cmd_tx {
            ch.try_send(Command::RequestBlock(peer, request))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
    }

    fn try_broadcast(&self, msg: BroadcastMsg, kind: &str) -> Result<(), P2pError> {
        let Some(ref ch) = self.broadcast_tx else {
            return Ok(());
        };
        match ch.try_send(msg) {
            Ok(()) => Ok(()),
            Err(e) => {
                // Surface channel-full / closed failures (callers often `let _ =`); otherwise a
                // panicked swarm task looks like silent vote/tx loss.
                boing_telemetry::component_warn(
                    "boing_p2p::broadcast",
                    "p2p",
                    "broadcast_channel_full_or_closed",
                    format!("{kind}: {e}"),
                );
                Err(P2pError::Network(format!("broadcast {kind}: {e}")))
            }
        }
    }

    pub fn broadcast_block(&self, block: &Block) -> Result<(), P2pError> {
        self.try_broadcast(BroadcastMsg::Block(block.clone()), "block")
    }

    /// Gossip a verified signed transaction to subscribed peers.
    pub fn broadcast_signed_transaction(&self, signed: &SignedTransaction) -> Result<(), P2pError> {
        self.try_broadcast(
            BroadcastMsg::SignedTransaction(signed.clone()),
            "transaction",
        )
    }

    /// Gossip a signed consensus vote to subscribed peers.
    pub fn broadcast_vote(&self, vote: &ConsensusVote) -> Result<(), P2pError> {
        self.try_broadcast(BroadcastMsg::Vote(vote.clone()), "vote")
    }

    /// Gossip verified equivocation evidence to subscribed peers.
    pub fn broadcast_equivocation(&self, evidence: &EquivocationEvidence) -> Result<(), P2pError> {
        self.try_broadcast(BroadcastMsg::Equivocation(evidence.clone()), "equivocation")
    }

    /// Gossip a verified per-validator ECVRF proof to subscribed peers.
    pub fn broadcast_vrf_proof(&self, proof: &VrfProofGossip) -> Result<(), P2pError> {
        self.try_broadcast(BroadcastMsg::VrfProof(proof.clone()), "vrf_proof")
    }

    /// Gossip media blob bytes on `boing/media-blobs` (Phase 2 data plane). Caller should
    /// [`MediaBlobGossip::validate`] first; peers re-verify on receive.
    pub fn broadcast_media_blob(&self, blob: &MediaBlobGossip) -> Result<(), P2pError> {
        self.try_broadcast(BroadcastMsg::MediaBlob(blob.clone()), "media_blob")
    }
}

impl Default for P2pNode {
    /// Default is inert (no network) for compatibility with tests.
    /// Use `P2pNode::new(addr)` for a live node.
    fn default() -> Self {
        Self::inert()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum P2pError {
    #[error("Network error: {0}")]
    Network(String),
}

#[cfg(test)]
mod startup_tests {
    use super::*;

    #[test]
    fn parse_listen_addrs_rejects_empty_and_garbage() {
        assert!(parse_listen_addrs("").is_err());
        assert!(parse_listen_addrs("   ,  , ").is_err());
        assert!(parse_listen_addrs("not-a-multiaddr").is_err());
        let ok = parse_listen_addrs("/ip4/127.0.0.1/tcp/4001").unwrap();
        assert_eq!(ok.len(), 1);
        let two = parse_listen_addrs("/ip4/0.0.0.0/tcp/4001,/ip6/::/tcp/4001").unwrap();
        assert_eq!(two.len(), 2);
    }

    #[test]
    fn ip_from_multiaddr_reads_literal_ip() {
        let ma: libp2p::Multiaddr = "/ip4/203.0.113.9/tcp/4001".parse().unwrap();
        assert_eq!(
            ip_from_multiaddr(&ma),
            Some(IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 9)))
        );
    }
}
