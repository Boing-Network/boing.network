//! Phased governance — time-locked parameter changes.
//!
//! Proposal → Cooling → Execution phases allow community response.
//! Includes transparent slashing with appeal mechanism.
//!
//! **Wiring status (audit #61 item 41):** [`SlashRegistry`] is used by `boing-node` (local mirror +
//! persistence). The phased [`Governance`] engine is also held by `boing-node` and driven through
//! operator RPCs (`boing_governancePropose` / `Vote` / `AdvanceCooling` / `AdvanceExecution` /
//! `Execute`). Allowlisted keys apply fee / QA policy via the same setters as
//! `boing_operatorApply*`. Stake-weighted **quorum votes** are implemented in this crate and
//! enforced before cooling when [`Governance::strict_quorum`] is on (default for new engines).
//! **Still deferred to a coordinated upgrade:** consensus `TransactionPayload` variants that
//! commit the same engine into the state root — see [`consensus_tx`] and
//! `docs/GOVERNANCE-CONSENSUS-TX-ACTIVATION.md`.

pub mod consensus_tx;
pub mod slashing_appeal;

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Lowercase hex encoding of a 32-byte account id (governance vote map key).
#[must_use]
pub fn voter_key_hex(voter: &[u8; 32]) -> String {
    hex_encode(voter)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

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

/// One stake-weighted ballot on a proposal (voter key = 32-byte account id).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalVote {
    pub approve: bool,
    /// Bonded stake (or other weight) at cast time.
    pub weight: u128,
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
    /// Stake-weighted ballots while in [`ProposalPhase::Proposal`].
    /// Keys are lowercase hex account ids ([`voter_key_hex`]) so JSON persistence works.
    #[serde(default)]
    pub votes: BTreeMap<String, ProposalVote>,
}

/// Default cooling period (e.g. 1 day).
pub const DEFAULT_COOLING: Duration = Duration::from_secs(86400);
/// Default execution window (e.g. 1 week).
pub const DEFAULT_EXECUTION_WINDOW: Duration = Duration::from_secs(7 * 86400);

/// Approve bps required of cast (yes+no) weight to advance cooling — ⌈2/3⌉ ≈ 6667.
pub const DEFAULT_QUORUM_APPROVE_BPS: u32 = 6_667;

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
    /// When true, [`Self::advance_to_cooling`] requires [`Self::quorum_met`].
    /// Serde default `false` keeps legacy `governance.json` operator-attested until operators
    /// flip the flag (or a consensus-tx activation enables it network-wide).
    #[serde(default)]
    pub strict_quorum: bool,
    /// Approve share of cast weight required, in basis points (default [`DEFAULT_QUORUM_APPROVE_BPS`]).
    #[serde(default = "default_quorum_approve_bps")]
    pub quorum_approve_bps: u32,
}

fn default_quorum_approve_bps() -> u32 {
    DEFAULT_QUORUM_APPROVE_BPS
}

impl Default for Governance {
    fn default() -> Self {
        Self {
            proposals: HashMap::new(),
            next_id: 1,
            cooling_period: DEFAULT_COOLING,
            execution_window: DEFAULT_EXECUTION_WINDOW,
            // New engines require stake-weighted quorum before cooling.
            strict_quorum: true,
            quorum_approve_bps: DEFAULT_QUORUM_APPROVE_BPS,
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
            strict_quorum: true,
            quorum_approve_bps: DEFAULT_QUORUM_APPROVE_BPS,
        }
    }

    /// Legacy operator-attested mode (cooling without votes) — used by older integration tests
    /// and by nodes that have not yet flipped [`Self::strict_quorum`].
    pub fn new_operator_attested(cooling_period: Duration, execution_window: Duration) -> Self {
        let mut g = Self::new(cooling_period, execution_window);
        g.strict_quorum = false;
        g
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
                votes: BTreeMap::new(),
            },
        );
        Ok(id)
    }

    /// Record (or replace) a stake-weighted ballot. Only valid in [`ProposalPhase::Proposal`].
    ///
    /// `weight == 0` is rejected. Re-voting replaces the previous ballot for `voter`.
    pub fn cast_vote(
        &mut self,
        id: u64,
        voter: [u8; 32],
        approve: bool,
        weight: u128,
    ) -> Result<(), GovernanceError> {
        if weight == 0 {
            return Err(GovernanceError::ZeroVoteWeight);
        }
        let p = self
            .proposals
            .get_mut(&id)
            .ok_or(GovernanceError::NotFound)?;
        if p.phase != ProposalPhase::Proposal {
            return Err(GovernanceError::InvalidPhase);
        }
        p.votes
            .insert(voter_key_hex(&voter), ProposalVote { approve, weight });
        Ok(())
    }

    /// `(approve_weight, reject_weight, cast_weight)` for a proposal.
    pub fn tally(&self, id: u64) -> Result<(u128, u128, u128), GovernanceError> {
        let p = self.proposals.get(&id).ok_or(GovernanceError::NotFound)?;
        let mut yes = 0u128;
        let mut no = 0u128;
        for v in p.votes.values() {
            if v.approve {
                yes = yes.saturating_add(v.weight);
            } else {
                no = no.saturating_add(v.weight);
            }
        }
        Ok((yes, no, yes.saturating_add(no)))
    }

    /// Whether approve weight meets [`Self::quorum_approve_bps`] of cast (yes+no) weight.
    ///
    /// Requires at least one non-zero cast vote. Abstentions are not modeled (omit the voter).
    pub fn quorum_met(&self, id: u64) -> Result<bool, GovernanceError> {
        let (yes, _no, cast) = self.tally(id)?;
        if cast == 0 {
            return Ok(false);
        }
        let bps = u128::from(self.quorum_approve_bps.min(10_000));
        // yes * 10_000 >= cast * bps  (ceil-style threshold via integer compare)
        Ok(yes.saturating_mul(10_000) >= cast.saturating_mul(bps))
    }

    /// Advance to cooling.
    ///
    /// When [`Self::strict_quorum`] is true, requires [`Self::quorum_met`]. When false (legacy
    /// operator-attested mode), cooling may advance with zero votes — the operator RPC path is
    /// then the attest. Consensus-tx activation will force `strict_quorum = true` network-wide.
    pub fn advance_to_cooling(&mut self, id: u64, now: SystemTime) -> Result<(), GovernanceError> {
        let result = (|| {
            if self.strict_quorum && !self.quorum_met(id)? {
                return Err(GovernanceError::QuorumNotMet);
            }
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

    pub fn set_strict_quorum(&mut self, on: bool) {
        self.strict_quorum = on;
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
    #[error("Vote weight must be non-zero")]
    ZeroVoteWeight,
    #[error("Governance quorum not met (need approve share of cast stake weight)")]
    QuorumNotMet,
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
    fn test_phased_governance_with_quorum() {
        let mut gov = Governance::new(Duration::from_secs(10), Duration::from_secs(100));
        let id = gov
            .propose("network_fee_config".into(), br#"{"ok":true}"#.to_vec())
            .unwrap();
        assert_eq!(gov.get(id).unwrap().phase, ProposalPhase::Proposal);

        let t0 = UNIX_EPOCH + Duration::from_secs(0);
        assert!(matches!(
            gov.advance_to_cooling(id, t0),
            Err(GovernanceError::QuorumNotMet)
        ));

        gov.cast_vote(id, [1u8; 32], true, 70).unwrap();
        gov.cast_vote(id, [2u8; 32], false, 30).unwrap();
        assert!(gov.quorum_met(id).unwrap());
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
    fn quorum_fails_below_two_thirds() {
        let mut gov = Governance::new(Duration::from_secs(1), Duration::from_secs(10));
        let id = gov.propose("qa_registry".into(), b"{}".to_vec()).unwrap();
        gov.cast_vote(id, [9u8; 32], true, 66).unwrap();
        gov.cast_vote(id, [8u8; 32], false, 34).unwrap();
        // 66/100 = 6600 bps < 6667
        assert!(!gov.quorum_met(id).unwrap());
        assert!(matches!(
            gov.advance_to_cooling(id, UNIX_EPOCH),
            Err(GovernanceError::QuorumNotMet)
        ));
    }

    #[test]
    fn operator_attested_skips_quorum() {
        let mut gov = Governance::new_operator_attested(Duration::from_secs(1), Duration::from_secs(10));
        let id = gov.propose("qa_registry".into(), b"{}".to_vec()).unwrap();
        gov.advance_to_cooling(id, UNIX_EPOCH).unwrap();
        assert_eq!(gov.get(id).unwrap().phase, ProposalPhase::Cooling);
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
        gov.cast_vote(id, [7u8; 32], true, 1).unwrap();
        gov.advance_to_cooling(id, UNIX_EPOCH).unwrap();
        let bytes = serde_json::to_vec(&gov).unwrap();
        let loaded: Governance = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(loaded.get(id).unwrap().phase, ProposalPhase::Cooling);
        assert_eq!(loaded.cooling_period(), Duration::from_secs(5));
        assert!(loaded.strict_quorum);
        assert_eq!(loaded.get(id).unwrap().votes.len(), 1);
    }

    #[test]
    fn legacy_json_without_votes_deserializes() {
        let raw = r#"{"proposals":{},"next_id":1,"cooling_period":86400,"execution_window":604800}"#;
        let g: Governance = serde_json::from_str(raw).unwrap();
        assert!(!g.strict_quorum);
        assert_eq!(g.quorum_approve_bps, DEFAULT_QUORUM_APPROVE_BPS);
    }
}
