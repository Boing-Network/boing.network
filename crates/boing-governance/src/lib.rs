//! Phased governance — time-locked parameter changes.
//!
//! Proposal → Cooling → Execution phases allow community response.
//! Includes transparent slashing with appeal mechanism.
//!
//! **Wiring status (audit #61 item 41):** [`SlashRegistry`] is used by `boing-node` (local mirror +
//! persistence). The phased [`Governance`] engine is also held by `boing-node` and driven through
//! operator RPCs (`boing_governancePropose` / `AdvanceCooling` / `AdvanceExecution` /
//! `Execute`). Allowlisted keys apply fee / QA policy via the same setters as
//! `boing_operatorApply*`. **Still deferred:** on-chain stake-weighted quorum votes and consensus
//! transactions that replace the operator-attested cooling gate.

pub mod slashing_appeal;

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Phase of a governance proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalPhase {
    /// Voting / proposal open.
    Proposal,
    /// Cooling period — community can exit.
    Cooling,
    /// Executable.
    Execution,
}

/// A governance proposal with time locks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Proposal {
    pub id: u64,
    pub target_key: String,
    pub target_value: Vec<u8>,
    pub phase: ProposalPhase,
    #[serde(with = "serde_system_time")]
    pub proposed_at: SystemTime,
    #[serde(default, with = "serde_system_time_opt")]
    pub cooling_ends_at: Option<SystemTime>,
    #[serde(default, with = "serde_system_time_opt")]
    pub execution_ends_at: Option<SystemTime>,
}

/// Default cooling period (e.g. 1 day).
pub const DEFAULT_COOLING: Duration = Duration::from_secs(86400);
/// Default execution window (e.g. 1 week).
pub const DEFAULT_EXECUTION_WINDOW: Duration = Duration::from_secs(7 * 86400);

/// Keys the node knows how to apply on [`Governance::execute`].
///
/// Matches `boing_qa::GOVERNANCE_QA_REGISTRY_KEY`, `boing_qa::GOVERNANCE_QA_POOL_CONFIG_KEY`,
/// and `boing_tokenomics::GOVERNANCE_NETWORK_FEE_CONFIG_KEY` (string literals kept here so this
/// crate does not depend on those packages).
pub const ALLOWED_TARGET_KEYS: &[&str] = &["qa_registry", "qa_pool_config", "network_fee_config"];

/// Governance engine — manages proposals and time locks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Governance {
    proposals: HashMap<u64, Proposal>,
    next_id: u64,
    #[serde(with = "serde_duration")]
    cooling_period: Duration,
    #[serde(with = "serde_duration")]
    execution_window: Duration,
}

impl Default for Governance {
    fn default() -> Self {
        Self {
            proposals: HashMap::new(),
            next_id: 1,
            cooling_period: DEFAULT_COOLING,
            execution_window: DEFAULT_EXECUTION_WINDOW,
        }
    }
}

impl Governance {
    pub fn new(cooling_period: Duration, execution_window: Duration) -> Self {
        Self {
            proposals: HashMap::new(),
            next_id: 1,
            cooling_period,
            execution_window,
        }
    }

    /// Whether `target_key` is in [`ALLOWED_TARGET_KEYS`].
    pub fn is_allowed_key(target_key: &str) -> bool {
        ALLOWED_TARGET_KEYS.contains(&target_key)
    }

    /// Create a proposal for an allowlisted key.
    pub fn propose(
        &mut self,
        target_key: String,
        target_value: Vec<u8>,
    ) -> Result<u64, GovernanceError> {
        if !Self::is_allowed_key(&target_key) {
            return Err(GovernanceError::UnknownTargetKey(target_key));
        }
        if target_value.is_empty() {
            return Err(GovernanceError::EmptyTargetValue);
        }
        let id = self.next_id;
        self.next_id += 1;
        let now = SystemTime::now();
        self.proposals.insert(
            id,
            Proposal {
                id,
                target_key,
                target_value,
                phase: ProposalPhase::Proposal,
                proposed_at: now,
                cooling_ends_at: None,
                execution_ends_at: None,
            },
        );
        Ok(id)
    }

    /// Advance to cooling.
    ///
    /// Callers (operator RPC today) are responsible for attesting that any off-chain / stakeholder
    /// quorum was met before invoking this. The library does not count votes yet (deferred).
    pub fn advance_to_cooling(&mut self, id: u64, now: SystemTime) -> Result<(), GovernanceError> {
        let result = (|| {
            let p = self
                .proposals
                .get_mut(&id)
                .ok_or(GovernanceError::NotFound)?;
            if p.phase != ProposalPhase::Proposal {
                return Err(GovernanceError::InvalidPhase);
            }
            p.phase = ProposalPhase::Cooling;
            p.cooling_ends_at = Some(now + self.cooling_period);
            Ok(())
        })();
        if let Err(ref e) = result {
            boing_telemetry::component_warn(
                "boing_governance::engine",
                "governance",
                "advance_to_cooling_failed",
                e,
            );
        }
        result
    }

    /// Advance to execution (after cooling ends).
    pub fn advance_to_execution(
        &mut self,
        id: u64,
        now: SystemTime,
    ) -> Result<(), GovernanceError> {
        let result = (|| {
            let p = self
                .proposals
                .get_mut(&id)
                .ok_or(GovernanceError::NotFound)?;
            if p.phase != ProposalPhase::Cooling {
                return Err(GovernanceError::InvalidPhase);
            }
            let cooling_ends = p.cooling_ends_at.ok_or(GovernanceError::InvalidPhase)?;
            if now < cooling_ends {
                return Err(GovernanceError::CoolingNotEnded);
            }
            p.phase = ProposalPhase::Execution;
            p.execution_ends_at = Some(now + self.execution_window);
            Ok(())
        })();
        if let Err(ref e) = result {
            boing_telemetry::component_warn(
                "boing_governance::engine",
                "governance",
                "advance_to_execution_failed",
                e,
            );
        }
        result
    }

    /// Execute proposal (within execution window). Returns (key, value) to apply.
    ///
    /// Does not validate the value payload — the node applies allowlisted keys and rejects bad JSON.
    pub fn execute(
        &mut self,
        id: u64,
        now: SystemTime,
    ) -> Result<(String, Vec<u8>), GovernanceError> {
        let result = (|| {
            let p = self.proposals.get(&id).ok_or(GovernanceError::NotFound)?;
            if p.phase != ProposalPhase::Execution {
                return Err(GovernanceError::InvalidPhase);
            }
            if !Self::is_allowed_key(&p.target_key) {
                return Err(GovernanceError::UnknownTargetKey(p.target_key.clone()));
            }
            let window = p.execution_ends_at.ok_or(GovernanceError::InvalidPhase)?;
            if now > window {
                return Err(GovernanceError::ExecutionWindowExpired);
            }
            Ok((p.target_key.clone(), p.target_value.clone()))
        })();
        if result.is_ok() {
            self.proposals.remove(&id);
        } else if let Err(ref e) = result {
            boing_telemetry::component_warn(
                "boing_governance::engine",
                "governance",
                "execute_proposal_failed",
                e,
            );
        }
        result
    }

    pub fn get(&self, id: u64) -> Option<&Proposal> {
        self.proposals.get(&id)
    }

    /// Snapshot of open proposals (any phase), sorted by id.
    pub fn list(&self) -> Vec<&Proposal> {
        let mut v: Vec<&Proposal> = self.proposals.values().collect();
        v.sort_by_key(|p| p.id);
        v
    }

    pub fn cooling_period(&self) -> Duration {
        self.cooling_period
    }

    pub fn execution_window(&self) -> Duration {
        self.execution_window
    }
}

pub use slashing_appeal::{
    AppealStatus, SlashReason, SlashRecord, SlashRegistry, SlashingAppeal, SlashingError,
    MAX_SLASH_APPEAL_EVIDENCE_BYTES,
};

#[derive(Debug, thiserror::Error)]
pub enum GovernanceError {
    #[error("Proposal not found")]
    NotFound,
    #[error("Invalid phase")]
    InvalidPhase,
    #[error("Cooling period not ended")]
    CoolingNotEnded,
    #[error("Execution window expired")]
    ExecutionWindowExpired,
    #[error("Unknown governance target_key: {0}")]
    UnknownTargetKey(String),
    #[error("Empty target_value")]
    EmptyTargetValue,
}

mod serde_system_time {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let secs = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        s.serialize_u64(secs)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(d)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}

mod serde_system_time_opt {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(t: &Option<SystemTime>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match t {
            Some(t) => {
                let secs = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                s.serialize_some(&secs)
            }
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<SystemTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = Option::<u64>::deserialize(d)?;
        Ok(secs.map(|s| UNIX_EPOCH + Duration::from_secs(s)))
    }
}

mod serde_duration {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_u64(d.as_secs())
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phased_governance() {
        let mut gov = Governance::new(Duration::from_secs(10), Duration::from_secs(100));
        let id = gov
            .propose("network_fee_config".into(), br#"{"ok":true}"#.to_vec())
            .unwrap();
        assert_eq!(gov.get(id).unwrap().phase, ProposalPhase::Proposal);

        let t0 = UNIX_EPOCH + Duration::from_secs(0);
        gov.advance_to_cooling(id, t0).unwrap();
        assert_eq!(gov.get(id).unwrap().phase, ProposalPhase::Cooling);

        let t1 = UNIX_EPOCH + Duration::from_secs(11); // after cooling
        gov.advance_to_execution(id, t1).unwrap();
        assert_eq!(gov.get(id).unwrap().phase, ProposalPhase::Execution);

        let (key, val) = gov.execute(id, t1).unwrap();
        assert_eq!(key, "network_fee_config");
        assert_eq!(val, br#"{"ok":true}"#);
    }

    #[test]
    fn rejects_unknown_target_key() {
        let mut gov = Governance::default();
        assert!(matches!(
            gov.propose("not_a_real_key".into(), vec![1]),
            Err(GovernanceError::UnknownTargetKey(_))
        ));
    }

    #[test]
    fn roundtrip_json() {
        let mut gov = Governance::new(Duration::from_secs(5), Duration::from_secs(50));
        let id = gov.propose("qa_registry".into(), b"{}".to_vec()).unwrap();
        gov.advance_to_cooling(id, UNIX_EPOCH).unwrap();
        let bytes = serde_json::to_vec(&gov).unwrap();
        let loaded: Governance = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(loaded.get(id).unwrap().phase, ProposalPhase::Cooling);
        assert_eq!(loaded.cooling_period(), Duration::from_secs(5));
    }
}
