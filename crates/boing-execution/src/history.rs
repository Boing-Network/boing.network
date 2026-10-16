//! Height-gated (versioned) execution rules.
//!
//! A live chain must replay from genesis with the current binary, so a rule change that alters state
//! roots is never applied retroactively: each historical rule set is kept as an *era* that starts at a
//! height, and blocks below the next era's start execute with it. New chains (and mainnet) start with
//! no history and run the current rules from genesis.

use boing_tokenomics::{FeeRemainder, NetworkFeePolicy};

/// Fee rule in force from `from_height` (inclusive) until the next era.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeEra {
    pub from_height: u64,
    pub policy: NetworkFeePolicy,
    pub remainder: FeeRemainder,
}

/// Ordered list of historical rule eras for one network.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionHistory {
    /// Network this history belongs to (chain id); a node refuses to apply it elsewhere.
    pub chain_id: u64,
    /// Sorted by `from_height`. Heights at/after `current_from` use the executor's configured policy.
    pub fee_eras: Vec<FeeEra>,
    /// First height executed with the current rules (no era applies at/after it).
    pub current_from: u64,
}

impl ExecutionHistory {
    /// Fee era covering `height`, or `None` when the current rules apply.
    pub fn fee_era_at(&self, height: u64) -> Option<&FeeEra> {
        if height >= self.current_from {
            return None;
        }
        self.fee_eras.iter().rev().find(|e| e.from_height <= height)
    }
}
