//! P2P node — libp2p swarm with gossipsub, mdns, and block request/response.
//!
//! Propagates blocks and **signed** transactions; discovers peers via mDNS; fetches blocks on demand.
//! Optional **max connections per IP** (`max_connections_per_ip`, 0 = disabled) limits Sybil-style fan-in.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use libp2p::futures::StreamExt;
use libp2p::multiaddr::Protocol;
use libp2p::gossipsub::{IdentTopic, MessageAuthenticity, ValidationMode};
#[cfg(feature = "mdns")]
use libp2p::mdns::tokio::Behaviour as Mdns;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::StreamProtocol;
use libp2p::gossipsub::PublishError;
use libp2p::{gossipsub, SwarmBuilder};
use tokio::sync::{mpsc, oneshot};
use tracing::info;

use crate::block_sync::{BlockRequest, BlockResponse};
use boing_primitives::{Block, ConsensusVote, EquivocationEvidence, Hash, SignedTransaction, VrfProofGossip};

const BLOCKS_TOPIC: &str = "boing/blocks";
const TRANSACTIONS_TOPIC: &str = "boing/transactions";
const VOTES_TOPIC: &str = "boing/votes";
const EQUIVOCATION_TOPIC: &str = "boing/equivocation";
const VRF_PROOFS_TOPIC: &str = "boing/vrf-proofs";

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
    /// Response from request_block (by hash or height).
    BlockFetched(Block),
}

enum BroadcastMsg {
    Block(Block),
    SignedTransaction(SignedTransaction),
    Vote(ConsensusVote),
    Equivocation(EquivocationEvidence),
    VrfProof(VrfProofGossip),
}

enum Command {
    RequestBlock(libp2p::PeerId, BlockRequest),
    GetPeers(oneshot::Sender<Vec<libp2p::PeerId>>),
    Dial(String),
}

/// Provides blocks for the request/response protocol.
pub trait BlockProvider: Send + Sync {
    fn get_block_by_hash(&self, hash: &Hash) -> Option<Block>;
    fn get_block_by_height(&self, height: u64) -> Option<Block>;
}

type BlockSyncBehaviour = request_response::cbor::Behaviour<BlockRequest, BlockResponse>;

fn ip_from_multiaddr(ma: &libp2p::Multiaddr) -> Option<IpAddr> {
    for p in ma.iter() {
        match p {
            Protocol::Ip4(ip) => return Some(IpAddr::V4(ip)),
            Protocol::Ip6(ip) => return Some(IpAddr::V6(ip)),
            _ => {}
        }
    }
    None
}

fn remote_multiaddr(endpoint: &libp2p_core::connection::ConnectedPoint) -> &libp2p::Multiaddr {
    match endpoint {
        libp2p_core::connection::ConnectedPoint::Dialer { address, .. } => address,
        libp2p_core::connection::ConnectedPoint::Listener { send_back_addr, .. } => {
            send_back_addr
        }
    }
}

#[cfg(feature = "mdns")]
#[derive(NetworkBehaviour)]
#[behaviour(prelude = "libp2p_swarm::derive_prelude")]
struct BoingBehaviour {
    mdns: Mdns,
    gossipsub: gossipsub::Behaviour,
    block_sync: BlockSyncBehaviour,
}

#[cfg(not(feature = "mdns"))]
#[derive(NetworkBehaviour)]
#[behaviour(prelude = "libp2p_swarm::derive_prelude")]
struct BoingBehaviour {
    gossipsub: gossipsub::Behaviour,
    block_sync: BlockSyncBehaviour,
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
        let (broadcast_tx, mut broadcast_rx) = mpsc::channel(64);
        let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
        // Unbounded so the swarm task never awaits on backpressure when forwarding gossip / blocks
        // to the node (bounded channels can stall libp2p and break mesh + tx propagation).
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                Default::default(),
                (libp2p::tls::Config::new, libp2p::noise::Config::new),
                libp2p::yamux::Config::default,
            )
            .map_err(|e| P2pError::Network(e.to_string()))?
            .with_behaviour(|key| {
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(1))
                    .validation_mode(ValidationMode::Permissive)
                    .build()
                    .map_err(|e| Box::new(std::io::Error::other(e.to_string())))?;
                let gossipsub =
                    gossipsub::Behaviour::new(MessageAuthenticity::Signed(key.clone()), gossipsub_config)
                        .map_err(|e| Box::new(std::io::Error::other(e.to_string())))?;
                let block_sync = BlockSyncBehaviour::new(
                    [(StreamProtocol::new("/boing/block-sync/1"), ProtocolSupport::Full)],
                    request_response::Config::default(),
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
                    })
                }
                #[cfg(not(feature = "mdns"))]
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(BoingBehaviour {
                    gossipsub,
                    block_sync,
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
        let listen_addr = listen_addr.to_string();
        let block_provider = block_provider;

        tokio::spawn(async move {
            let mut swarm = swarm;
            let mut peer_ips: HashMap<libp2p::PeerId, IpAddr> = HashMap::new();
            let mut ip_connection_count: HashMap<IpAddr, u32> = HashMap::new();
            let _ = swarm.listen_on(
                listen_addr
                    .parse()
                    .expect("valid listen address"),
            );
            swarm.behaviour_mut().gossipsub.subscribe(&blocks_topic).expect("subscribe blocks");
            swarm.behaviour_mut().gossipsub.subscribe(&txs_topic).expect("subscribe txs");
            swarm.behaviour_mut().gossipsub.subscribe(&votes_topic).expect("subscribe votes");
            swarm
                .behaviour_mut()
                .gossipsub
                .subscribe(&equivocation_topic)
                .expect("subscribe equivocation");
            swarm
                .behaviour_mut()
                .gossipsub
                .subscribe(&vrf_proofs_topic)
                .expect("subscribe vrf-proofs");

            info!("P2P: listening on {} peer_id={:?}", listen_addr, swarm.local_peer_id());

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
                            None => break,
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
                            None => break,
                        }
                    }
                    ev = swarm.select_next_some() => {
                        if let SwarmEvent::Behaviour(BoingBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                                message, ..
                            })) = ev {
                            let topic = message.topic.as_str();
                            if topic == BLOCKS_TOPIC {
                                if let Ok(block) = bincode::deserialize(&message.data) {
                                    let _ = event_tx.send(P2pEvent::BlockReceived(block));
                                }
                            } else if topic == TRANSACTIONS_TOPIC {
                                if let Ok(signed) = bincode::deserialize::<SignedTransaction>(&message.data)
                                {
                                    let _ = event_tx.send(P2pEvent::TransactionReceived(signed));
                                }
                            } else if topic == VOTES_TOPIC {
                                if let Ok(vote) = bincode::deserialize::<ConsensusVote>(&message.data) {
                                    if vote.verify().is_ok() {
                                        let _ = event_tx.send(P2pEvent::VoteReceived(vote));
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
                                if let Ok(ev) =
                                    bincode::deserialize::<EquivocationEvidence>(&message.data)
                                {
                                    if ev.verify().is_ok() {
                                        let _ = event_tx.send(P2pEvent::EquivocationReceived(ev));
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
                                if let Ok(proof) =
                                    bincode::deserialize::<VrfProofGossip>(&message.data)
                                {
                                    if proof.verify() {
                                        let _ = event_tx.send(P2pEvent::VrfProofReceived(proof));
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
                            }
                        } else if let SwarmEvent::ConnectionEstablished {
                            peer_id,
                            endpoint,
                            ..
                        } = &ev
                        {
                            if max_connections_per_ip > 0 {
                                let ma = remote_multiaddr(endpoint);
                                if let Some(ip) = ip_from_multiaddr(ma) {
                                    if !peer_ips.contains_key(peer_id) {
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
                                        } else {
                                            *ip_connection_count.entry(ip).or_insert(0) += 1;
                                            peer_ips.insert(*peer_id, ip);
                                        }
                                    }
                                }
                            }
                        } else if let SwarmEvent::ConnectionClosed { peer_id, .. } = &ev {
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

    /// Dial a peer by multiaddress (e.g. "/ip4/127.0.0.1/tcp/4001").
    pub fn dial(&self, addr: &str) -> Result<(), P2pError> {
        if let Some(ref ch) = self.cmd_tx {
            ch.try_send(Command::Dial(addr.to_string()))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
    }

    /// Request a block from a peer. Response arrives via P2pEvent::BlockFetched.
    pub fn request_block(&self, peer: libp2p::PeerId, request: BlockRequest) -> Result<(), P2pError> {
        if let Some(ref ch) = self.cmd_tx {
            ch.try_send(Command::RequestBlock(peer, request))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
    }

    pub fn broadcast_block(&self, block: &Block) -> Result<(), P2pError> {
        if let Some(ref ch) = self.broadcast_tx {
            ch.try_send(BroadcastMsg::Block(block.clone()))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
    }

    /// Gossip a verified signed transaction to subscribed peers.
    pub fn broadcast_signed_transaction(&self, signed: &SignedTransaction) -> Result<(), P2pError> {
        if let Some(ref ch) = self.broadcast_tx {
            ch.try_send(BroadcastMsg::SignedTransaction(signed.clone()))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
    }

    /// Gossip a signed consensus vote to subscribed peers.
    pub fn broadcast_vote(&self, vote: &ConsensusVote) -> Result<(), P2pError> {
        if let Some(ref ch) = self.broadcast_tx {
            ch.try_send(BroadcastMsg::Vote(vote.clone()))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
    }

    /// Gossip verified equivocation evidence to subscribed peers.
    pub fn broadcast_equivocation(&self, evidence: &EquivocationEvidence) -> Result<(), P2pError> {
        if let Some(ref ch) = self.broadcast_tx {
            ch.try_send(BroadcastMsg::Equivocation(evidence.clone()))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
    }

    /// Gossip a verified per-validator ECVRF proof to subscribed peers.
    pub fn broadcast_vrf_proof(&self, proof: &VrfProofGossip) -> Result<(), P2pError> {
        if let Some(ref ch) = self.broadcast_tx {
            ch.try_send(BroadcastMsg::VrfProof(proof.clone()))
                .map_err(|e| P2pError::Network(e.to_string()))?;
        }
        Ok(())
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
