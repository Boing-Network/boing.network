//! Boing P2P — libp2p-based networking
//!
//! Permissionless peer discovery, transaction propagation, sync.

mod block_sync;
mod discovery;
mod node;
mod webrtc;

pub use block_sync::{BlockRequest, BlockResponse};
pub use discovery::{BootnodeEntry, PeerDiscoveryConfig, PeerScore};
pub use webrtc::{
    ContentPointer, SignalingContract, SignalingDepositConfig, SignalingMessage,
    SignalingMessageKind, SignalingPostResult, SignalingRateLimit, StunTurnMetrics,
    StunTurnRegistryEntry, StunTurnReputation,
};
pub use node::{BlockProvider, P2pError, P2pEvent, P2pNode};
pub use boing_primitives::{Block, ConsensusVote, EquivocationEvidence, SignedTransaction, Transaction};
