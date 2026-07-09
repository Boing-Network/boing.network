//! Adaptive gas model — configurable costs with predictable upper bounds.
//!
//! Gas costs can scale with network congestion while respecting caps.

/// Base gas costs per transaction type (before multiplier).
pub mod base {
    pub const TRANSFER: u64 = 21_000;
    pub const CONTRACT_CALL: u64 = 100_000;
    pub const CONTRACT_DEPLOY: u64 = 200_000;
    pub const BOND: u64 = 21_000;
    pub const UNBOND: u64 = 21_000;
    pub const CLAIM_UNBOND: u64 = 21_000;
}

/// Maximum gas multiplier (e.g. 2x under heavy load).
const MAX_MULTIPLIER: u64 = 2;

/// Gas configuration — base costs and optional congestion multiplier.
#[derive(Clone, Debug)]
pub struct GasConfig {
    /// 1.0 = base cost. 1.5 = 50% increase. Max 2.0.
    pub multiplier_e4: u64, // e.g. 10000 = 1.0, 15000 = 1.5
}

impl Default for GasConfig {
    fn default() -> Self {
        Self { multiplier_e4: 10000 }
    }
}

impl GasConfig {
    /// Create with multiplier (1.0 = base). Clamped to [1.0, 2.0].
    pub fn with_multiplier(multiplier: f64) -> Self {
        let e4 = (multiplier.clamp(1.0, MAX_MULTIPLIER as f64) * 10000.0) as u64;
        Self { multiplier_e4: e4 }
    }

    /// Compute gas for a base cost.
    pub fn gas(&self, base: u64) -> u64 {
        (base * self.multiplier_e4) / 10000
    }

    /// Transfer gas.
    pub fn transfer(&self) -> u64 {
        self.gas(base::TRANSFER)
    }

    /// Contract call gas.
    pub fn contract_call(&self) -> u64 {
        self.gas(base::CONTRACT_CALL)
    }

    /// Contract deploy gas.
    pub fn contract_deploy(&self) -> u64 {
        self.gas(base::CONTRACT_DEPLOY)
    }

    /// Upper bound for any single tx (predictable cap).
    pub fn max_tx_gas(&self) -> u64 {
        self.gas(base::CONTRACT_DEPLOY * 2) // worst-case reasonable tx
    }
}
