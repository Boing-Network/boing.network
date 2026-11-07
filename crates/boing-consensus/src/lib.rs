//! Boing Consensus — PoS + HotStuff-style **single-round quorum commit**
//!
//! RPC advertises `model: "single_round_quorum_commit"`. This is a wall-clock pacemaker with
//! 2f+1 vote commit — not a certified HotStuff implementation with timeout certificates.
//!
//! Permissionless validator set, deterministic finality.

mod engine;

pub use boing_primitives::{Block, BlockHeader, Hash};
pub use engine::{ConsensusEngine, ConsensusError, LeaderElection};
