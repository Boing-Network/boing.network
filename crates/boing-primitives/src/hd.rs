//! HD (Hierarchical Deterministic) wallet support.
//!
//! See SECURITY-STANDARDS.md (key management).

use serde::{Deserialize, Serialize};

/// BIP-44 style derivation path: m / purpose' / coin_type' / account' / change / index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HdPath {
    pub purpose: u32,
    pub coin_type: u32,
    pub account: u32,
    pub change: u32,
    pub index: u32,
}

impl HdPath {
    /// Boing mainnet coin type (placeholder; register with SLIP-44).
    pub const BOING_COIN_TYPE: u32 = 0x8000_0234; // example

    pub fn new(purpose: u32, coin_type: u32, account: u32, change: u32, index: u32) -> Self {
        Self {
            purpose,
            coin_type,
            account,
            change,
            index,
        }
    }

    /// Standard path for Boing: m/44'/BOING'/0'/0/index.
    pub fn boing(index: u32) -> Self {
        Self::new(44, Self::BOING_COIN_TYPE, 0, 0, index)
    }
}

impl std::fmt::Display for HdPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "m/{}'/{}'/{}'/{}/{}",
            self.purpose,
            self.coin_type & 0x7FFF_FFFF,
            self.account,
            self.change,
            self.index
        )
    }
}
