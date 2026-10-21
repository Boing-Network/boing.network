//! Boing State — Verkle tree state management
//!
//! Enables stateless clients and compact proofs.

pub mod names;
pub mod open_validators;
pub mod sample_offers;
pub mod sample_serve;
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
    EscrowedSampleOffer, SampleOfferRegistry, SampleOfferRegistryError,
};
pub use sample_serve::{SampleServeChallengeError, SampleServeChallengeRegistry};
pub use qa_pending::{unpaid_counted_voters, QaPendingRecord};
pub use sparse_merkle::{
    hash_account_leaf, hash_contract_code, MerkleProof, ProofStep, SparseMerkleTree,
};
pub use store::{ChainNativeAggregates, ContractStorageEntry, StateCheckpoint, StateStore};
