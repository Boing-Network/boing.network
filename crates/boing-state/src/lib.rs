//! Boing State — Verkle tree state management
//!
//! Enables stateless clients and compact proofs.

pub mod names;
pub mod open_validators;
pub mod sample_offers;
pub mod sample_serve;
pub mod slash_appeals;
mod qa_pending;
mod sparse_merkle;
mod store;

pub use boing_primitives::{Account, AccountId, AccountState, Hash};
pub use names::{
    CommitRecord, NameRecord, NameRegistry, NameRegistryError, NameRegistryParams, NameStatus,
};
pub use open_validators::{
    LivenessEntry, LivenessStats, OpenValidatorError, OpenValidatorParams, OpenValidatorRegistry,
    ValidatorRecord, ValidatorStatus,
};
pub use sample_offers::{
    EscrowedSampleOffer, SampleOfferRegistry, SampleOfferRegistryError, SettledSampleMatch,
};
pub use sample_serve::{SampleServeChallengeError, SampleServeChallengeRegistry};
pub use slash_appeals::{
    CastResolveOutcome, ConsensusAppealStatus, ConsensusSlashAppeal, ConsensusSlashReason,
    ConsensusSlashRecord, ResolveVote, SlashAppealError, SlashAppealRegistry,
    SLASH_APPEAL_REGISTRY_DOMAIN,
};
pub use qa_pending::{unpaid_counted_voters, QaPendingRecord};
pub use sparse_merkle::{
    hash_account_leaf, hash_contract_code, MerkleProof, ProofStep, SparseMerkleTree,
    DEEP_MAX_DEPTH, LEGACY_MAX_DEPTH,
};
pub use store::{ChainNativeAggregates, ContractStorageEntry, StateCheckpoint, StateStore};
