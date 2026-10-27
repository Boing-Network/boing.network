//! Boing P2P — libp2p-based networking
//!
//! Permissionless peer discovery, transaction propagation, sync.

mod block_sync;
mod discovery;
pub mod gossip_limits;
mod node;
pub mod redial;
mod webrtc;

pub use block_sync::{BlockRequest, BlockResponse};
pub use boing_primitives::{
    Block, ConsensusVote, EquivocationEvidence, SignedTransaction, Transaction, VrfProofGossip,
};
pub use discovery::{BootnodeEntry, PeerDiscoveryConfig, PeerScore};
pub use gossip_limits::{GossipConfig, PeerRateLimiter};
pub use node::{
    BlockProvider, P2pError, P2pEvent, P2pNode, P2pStatus, PING_INTERVAL, PING_TIMEOUT,
};
pub use webrtc::{
    ContentPointer, SignalingContract, SignalingDepositConfig, SignalingMessage,
    SignalingMessageKind, SignalingPostResult, SignalingRateLimit, StunTurnMetrics,
    StunTurnRegistryEntry, StunTurnReputation,
};
