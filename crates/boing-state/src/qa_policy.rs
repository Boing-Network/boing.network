//! Committed QA policy (testnet gate `qa_canonical_from`, v0.1.23).
//!
//! From the activation height, block execution uses the QA rule registry and pool config held
//! here. They are set only by consensus: on-chain governance proposals for `qa_registry` /
//! `qa_pool_config` whose cooling has ended are executed at the start of block processing.
//! Empty = protocol defaults (`RuleRegistry::new()` +
//! `QaPoolGovernanceConfig::development_default()`). A node's local `--qa-registry` /
//! `qa_pool_config.json` then only affects its own mempool admission.
//!
//! Values are stored as governance JSON bytes so this crate does not depend on `boing-qa`.
//! Committed into the account SMT under [`CommittedQaPolicy::smt_key`] only once non-empty, so
//! states that never set a policy keep exactly the same root.

use boing_primitives::hasher;
use serde::{Deserialize, Serialize};

/// Domain tag for the SMT leaf of the committed QA policy.
pub const QA_POLICY_REGISTRY_DOMAIN: &[u8] = b"BOING_QA_POLICY_V1";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommittedQaPolicy {
    /// `qa_registry` governance JSON (`None` = protocol default registry).
    pub registry_json: Option<Vec<u8>>,
    /// `qa_pool_config` governance JSON (`None` = protocol default pool config).
    pub pool_config_json: Option<Vec<u8>>,
}

impl CommittedQaPolicy {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registry_json.is_none() && self.pool_config_json.is_none()
    }

    #[must_use]
    pub fn smt_key() -> [u8; 32] {
        let mut h = hasher();
        h.update(QA_POLICY_REGISTRY_DOMAIN);
        *h.finalize().as_bytes()
    }

    #[must_use]
    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher();
        h.update(QA_POLICY_REGISTRY_DOMAIN);
        h.update(&bincode::serialize(self).unwrap_or_default());
        *h.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_by_default_and_commitment_changes() {
        let a = CommittedQaPolicy::default();
        assert!(a.is_empty());
        let b = CommittedQaPolicy {
            registry_json: Some(b"{}".to_vec()),
            pool_config_json: None,
        };
        assert!(!b.is_empty());
        assert_ne!(a.commitment(), b.commitment());
        assert_ne!(
            CommittedQaPolicy::smt_key(),
            crate::GovernanceRegistry::smt_key()
        );
    }

    #[test]
    fn empty_policy_keeps_state_root() {
        let mut s = crate::StateStore::new();
        let r0 = s.state_root();
        s.load_qa_policy(CommittedQaPolicy::default());
        assert_eq!(s.state_root(), r0);
        s.qa_policy_mut().registry_json = Some(b"{}".to_vec());
        assert_ne!(s.state_root(), r0);
    }
}
