//! Testnet faucet: deterministic key and account for boing_faucetRequest RPC.
//! **Use only on testnet.** Do not enable on mainnet.

use boing_primitives::{hash::hasher, AccountId};
use ed25519_dalek::SigningKey;

/// Deterministic testnet faucet key (from seed). Same account everywhere for testnet.
pub fn testnet_faucet_signing_key() -> SigningKey {
    let mut h = hasher();
    h.update(b"boing-testnet-faucet-v1");
    let out = h.finalize();
    let bytes: [u8; 32] = *out.as_bytes();
    SigningKey::from_bytes(&bytes)
}

/// Faucet account ID (public key of the testnet faucet key).
pub fn testnet_faucet_account_id() -> AccountId {
    let key = testnet_faucet_signing_key();
    AccountId(key.verifying_key().to_bytes())
}

/// Initial balance for the faucet account when created (testnet only).
pub const FAUCET_INITIAL_BALANCE: u128 = 10_000_000;

/// Amount sent per faucet request.
///
/// Sized so a single request can fund many native txs (a transfer is 1 BOING; a
/// reference fungible deploy is typically ~10–20 BOING after `fee_for_gas`) **and**
/// min validator stake ([`boing_tokenomics::MIN_VALIDATOR_STAKE`] = 10_000).
pub const FAUCET_DISPENSE_AMOUNT: u128 = 1_000_000;
