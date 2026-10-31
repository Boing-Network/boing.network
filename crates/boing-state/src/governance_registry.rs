//! State-root commitment for the phased governance engine (audit #61 item 41).
//!
//! Wraps [`boing_governance::Governance`] so propose / vote / cooling advance via consensus
//! `TransactionPayload` indices 37–39 land in the same SMT leaf on every validator. Operator
//! RPC (`governance.json`) remains a separate node-local book until product unifies them.

use boing_governance::Governance;
use boing_primitives::hasher;
use serde::{Deserialize, Serialize};

/// Domain tag for SMT commitment of the governance registry.
pub const GOVERNANCE_REGISTRY_DOMAIN: &[u8] = b"BOING_GOVERNANCE_REGISTRY_V1";

/// Committed governance book (serde/bincode of the engine).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GovernanceRegistry {
    engine: Governance,
}

impl GovernanceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: Governance::default(),
        }
    }

    #[must_use]
    pub fn from_engine(engine: Governance) -> Self {
        Self { engine }
    }

    #[must_use]
    pub fn engine(&self) -> &Governance {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut Governance {
        &mut self.engine
    }

    #[must_use]
    pub fn smt_key() -> [u8; 32] {
        let mut h = hasher();
        h.update(GOVERNANCE_REGISTRY_DOMAIN);
        *h.finalize().as_bytes()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.engine.is_empty()
    }

    #[must_use]
    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher();
        h.update(GOVERNANCE_REGISTRY_DOMAIN);
        h.update(&bincode::serialize(self).unwrap_or_default());
        *h.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn empty_registry_has_stable_key() {
        let a = GovernanceRegistry::smt_key();
        let b = GovernanceRegistry::smt_key();
        assert_eq!(a, b);
        assert!(GovernanceRegistry::new().is_empty());
    }

    #[test]
    fn propose_changes_commitment() {
        let mut reg = GovernanceRegistry::new();
        let c0 = reg.commitment();
        reg.engine_mut()
            .propose_at(
                "network_fee_config".into(),
                br#"{"ok":true}"#.to_vec(),
                UNIX_EPOCH + Duration::from_secs(1),
            )
            .unwrap();
        assert!(!reg.is_empty());
        assert_ne!(c0, reg.commitment());
    }
}
