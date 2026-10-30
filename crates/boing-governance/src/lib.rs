//! Phased governance — time-locked parameter changes.
//!
//! Proposal → Cooling → Execution phases allow community response.
//! Includes transparent slashing with appeal mechanism.

pub mod slashing_appeal;

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Phase of a governance proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalPhase {
    /// Voting / proposal open.
    Proposal,
    /// Cooling period — community can exit.
    Cooling,
    /// Executable.
    Execution,
}

/// A governance proposal with time locks.
#[derive(Clone, Debug)]
pub struct Proposal {
    pub id: u64,
    pub target_key: String,
    pub target_value: Vec<u8>,
    pub phase: ProposalPhase,
    pub proposed_at: SystemTime,
    pub cooling_ends_at: Option<SystemTime>,
    pub execution_ends_at: Option<SystemTime>,
}

/// Default cooling period (e.g. 1 day).
pub const DEFAULT_COOLING: Duration = Duration::from_secs(86400);
/// Default execution window (e.g. 1 week).
pub const DEFAULT_EXECUTION_WINDOW: Duration = Duration::from_secs(7 * 86400);

/// Governance engine — manages proposals and time locks.
pub struct Governance {
    proposals: HashMap<u64, Proposal>,
    next_id: u64,
    cooling_period: Duration,
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

    /// Create a proposal.
    pub fn propose(&mut self, target_key: String, target_value: Vec<u8>) -> u64 {
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
        id
    }

    /// Advance to cooling (e.g. after quorum vote).
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn test_phased_governance() {
        let mut gov = Governance::new(Duration::from_secs(10), Duration::from_secs(100));
        let id = gov.propose("gas_multiplier".into(), vec![150]); // 1.5x
        assert_eq!(gov.get(id).unwrap().phase, ProposalPhase::Proposal);

        let t0 = UNIX_EPOCH + Duration::from_secs(0);
        gov.advance_to_cooling(id, t0).unwrap();
        assert_eq!(gov.get(id).unwrap().phase, ProposalPhase::Cooling);

        let t1 = UNIX_EPOCH + Duration::from_secs(11); // after cooling
        gov.advance_to_execution(id, t1).unwrap();
        assert_eq!(gov.get(id).unwrap().phase, ProposalPhase::Execution);

        let (key, val) = gov.execute(id, t1).unwrap();
        assert_eq!(key, "gas_multiplier");
        assert_eq!(val, vec![150]);
    }
}
