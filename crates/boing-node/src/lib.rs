//! Boing Node — blockchain node library.
//!
//! Provides BoingNode for running a validator or full node.

pub mod block_producer;
pub mod block_validation;
pub mod chain;
pub mod faucet;
pub mod persistence;
pub use node::ChainBlockProvider;
pub mod dapp_registry;
pub mod intent_pool;
pub mod logging;
pub mod mempool;
pub mod native_dex_discovery;
pub mod node;
pub mod rpc;
pub mod rpc_ws;
pub mod security;
pub mod validator_config;
