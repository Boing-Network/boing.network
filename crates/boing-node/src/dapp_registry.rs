//! In-memory dApp registration for success-based incentive tracking.
//! Stub implementation; production would persist and sync across nodes.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use boing_primitives::AccountId;
use boing_tokenomics::dapp_incentives::DappRegistration;

/// In-memory store of dApp registrations (contract -> owner).
#[derive(Clone, Debug, Default)]
pub struct DappRegistry {
    inner: Arc<RwLock<DappRegistryInner>>,
}

#[derive(Debug, Default)]
struct DappRegistryInner {
    registrations: Vec<DappRegistration>,
    contracts: HashSet<AccountId>,
}

impl DappRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(DappRegistryInner::default())),
        }
    }

    /// Soft cap on in-memory registrations (audit #61 item 20: unbounded RPC map).
    pub const MAX_REGISTRATIONS: usize = 10_000;

    /// Register a contract for dApp incentive tracking. Idempotent (re-register updates owner).
    /// Returns `false` when the registry is at capacity and `contract` is new.
    pub fn register(&self, contract: AccountId, owner: AccountId) -> bool {
        let mut inner = self.inner.write().unwrap();
        let is_new = !inner.contracts.contains(&contract);
        if is_new && inner.registrations.len() >= Self::MAX_REGISTRATIONS {
            return false;
        }
        inner.contracts.insert(contract);
        // Replace existing registration for same contract
        inner.registrations.retain(|r| r.contract != contract);
        inner
            .registrations
            .push(DappRegistration { contract, owner });
        true
    }

    pub fn len(&self) -> usize {
        self.inner.read().unwrap().registrations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// List all registered dApps.
    pub fn list(&self) -> Vec<DappRegistration> {
        self.inner.read().unwrap().registrations.clone()
    }

    /// Check if a contract is registered.
    pub fn is_registered(&self, contract: &AccountId) -> bool {
        self.inner.read().unwrap().contracts.contains(contract)
    }
}
