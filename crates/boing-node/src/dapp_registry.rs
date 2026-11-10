//! In-memory dApp registration for success-based incentive tracking.
//! Stub implementation; production would persist and sync across nodes.

use std::collections::HashSet;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

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

    /// Read lock that survives a poisoned mutex (a panic must not brick dApp registration RPCs).
    fn read_inner(&self) -> RwLockReadGuard<'_, DappRegistryInner> {
        self.inner.read().unwrap_or_else(|poisoned| {
            tracing::warn!(
                "dApp registry lock poisoned by an earlier panic; recovered, poison cleared"
            );
            self.inner.clear_poison();
            poisoned.into_inner()
        })
    }

    /// Write lock that survives a poisoned mutex.
    fn write_inner(&self) -> RwLockWriteGuard<'_, DappRegistryInner> {
        self.inner.write().unwrap_or_else(|poisoned| {
            tracing::warn!(
                "dApp registry lock poisoned by an earlier panic; recovered, poison cleared"
            );
            self.inner.clear_poison();
            poisoned.into_inner()
        })
    }

    /// Register a contract for dApp incentive tracking. Idempotent (re-register updates owner).
    /// Returns `false` when the registry is at capacity and `contract` is new.
    pub fn register(&self, contract: AccountId, owner: AccountId) -> bool {
        let mut inner = self.write_inner();
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
        self.read_inner().registrations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// List all registered dApps.
    pub fn list(&self) -> Vec<DappRegistration> {
        self.read_inner().registrations.clone()
    }

    /// Check if a contract is registered.
    pub fn is_registered(&self, contract: &AccountId) -> bool {
        self.read_inner().contracts.contains(contract)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_lock_does_not_brick_registry() {
        // Regression: `read()/write().unwrap()` panicked forever once a panic poisoned the lock.
        let reg = DappRegistry::new();
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = reg.inner.write().unwrap();
            panic!("simulated panic while holding the dApp registry lock");
        }));
        std::panic::set_hook(prev);
        assert!(poisoned.is_err());
        assert!(reg.inner.is_poisoned(), "test setup must poison the lock");
        let a = AccountId([1u8; 32]);
        let b = AccountId([2u8; 32]);
        assert!(reg.register(a, b));
        assert_eq!(reg.len(), 1);
        assert!(reg.is_registered(&a));
        assert_eq!(reg.list().len(), 1);
    }
}
