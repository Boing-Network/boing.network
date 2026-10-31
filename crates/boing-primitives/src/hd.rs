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
    /// Boing coin type for BIP-44 paths.
    ///
    /// **Not** a registered SLIP-44 constant yet — provisional `0x8000_0234` until Boing has an
    /// official assignment (audit #61 item 38). Do not treat this as final for hardware wallets.
    pub const BOING_COIN_TYPE: u32 = 0x8000_0234;

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
