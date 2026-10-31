//! Activation-gated **consensus transaction** scaffolding for phased governance
//! (audit #61 item 41 leftover).
//!
//! Wire indices **37–39** (`GovernancePropose` / `GovernanceVote` / `GovernanceAdvanceCooling`)
//! ship with StateStore commitment + mempool admission. This module keeps activation helpers and
//! planned body shapes aligned with `TransactionPayload`.
//!
//! See [`docs/GOVERNANCE-CONSENSUS-TX-ACTIVATION.md`](../../../docs/GOVERNANCE-CONSENSUS-TX-ACTIVATION.md).

use crate::{GovernanceError, ALLOWED_TARGET_KEYS};

/// Env var for the height at which governance consensus txs become admitted.
///
/// Unset / empty / `never` → never admitted (default). A concrete `u64` is reserved for a
/// coordinated network upgrade; nodes that disagree on this value diverge at that height.
pub const GOVERNANCE_CONSENSUS_TX_ACTIVATION_ENV: &str =
    "BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT";

/// Wire indices (append-only after `ReportEquivocation` = 36). **Do not reorder.**
pub const WIRE_GOVERNANCE_PROPOSE: u32 = 37;
pub const WIRE_GOVERNANCE_VOTE: u32 = 38;
pub const WIRE_GOVERNANCE_ADVANCE_COOLING: u32 = 39;

/// Deprecated aliases — prefer [`WIRE_GOVERNANCE_PROPOSE`] etc.
pub const PLANNED_WIRE_GOVERNANCE_PROPOSE: u32 = WIRE_GOVERNANCE_PROPOSE;
pub const PLANNED_WIRE_GOVERNANCE_VOTE: u32 = WIRE_GOVERNANCE_VOTE;
pub const PLANNED_WIRE_GOVERNANCE_ADVANCE_COOLING: u32 = WIRE_GOVERNANCE_ADVANCE_COOLING;

/// Planned propose body (matches operator RPC `boing_governancePropose` allowlist).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedGovernancePropose {
    pub target_key: String,
    pub target_value: Vec<u8>,
}

/// Planned stake-weighted ballot on a proposal still in the Proposal phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedGovernanceVote {
    pub proposal_id: u64,
    pub approve: bool,
}

/// Planned cooling advance — only succeeds once on-chain quorum is met (no operator attest).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedGovernanceAdvanceCooling {
    pub proposal_id: u64,
}

/// Parse activation height from an env-style string.
///
/// * `None` / `""` / `"never"` / `"off"` → inactive forever
/// * decimal `u64` → activate at that height (inclusive)
pub fn parse_activation_height(raw: Option<&str>) -> Result<Option<u64>, String> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.eq_ignore_ascii_case("never") || s.eq_ignore_ascii_case("off") {
        return Ok(None);
    }
    s.parse::<u64>()
        .map(Some)
        .map_err(|e| format!("invalid {GOVERNANCE_CONSENSUS_TX_ACTIVATION_ENV}: {e}"))
}

/// Whether consensus governance txs are admitted at `height`.
#[must_use]
pub fn consensus_txs_active(activation: Option<u64>, height: u64) -> bool {
    match activation {
        Some(h) => height >= h,
        None => false,
    }
}

/// Structural checks shared by the future wire path and today's RPC propose.
pub fn validate_planned_propose(p: &PlannedGovernancePropose) -> Result<(), GovernanceError> {
    if !ALLOWED_TARGET_KEYS.contains(&p.target_key.as_str()) {
        return Err(GovernanceError::UnknownTargetKey(p.target_key.clone()));
    }
    if p.target_value.is_empty() {
        return Err(GovernanceError::EmptyTargetValue);
    }
    Ok(())
}

/// Reject reason used by mempool/VM stubs until the wire variants ship and activate.
#[must_use]
pub fn not_yet_admitted_message(activation: Option<u64>, height: u64) -> String {
    match activation {
        None => format!(
            "governance consensus txs not admitted (set {GOVERNANCE_CONSENSUS_TX_ACTIVATION_ENV} to a height to enable)"
        ),
        Some(h) if height < h => format!(
            "governance consensus txs activate at height {h} (current {height})"
        ),
        Some(_) => "governance consensus txs are active at this height".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_defaults_off() {
        assert_eq!(parse_activation_height(None).unwrap(), None);
        assert_eq!(parse_activation_height(Some("")).unwrap(), None);
        assert_eq!(parse_activation_height(Some("never")).unwrap(), None);
        assert!(!consensus_txs_active(None, 0));
        assert!(!consensus_txs_active(None, u64::MAX));
    }

    #[test]
    fn activation_height_gate() {
        assert_eq!(parse_activation_height(Some("1500")).unwrap(), Some(1500));
        assert!(!consensus_txs_active(Some(1500), 1499));
        assert!(consensus_txs_active(Some(1500), 1500));
        assert!(consensus_txs_active(Some(1500), 1501));
    }

    #[test]
    fn planned_propose_allowlist() {
        assert!(validate_planned_propose(&PlannedGovernancePropose {
            target_key: "network_fee_config".into(),
            target_value: b"{}".to_vec(),
        })
        .is_ok());
        assert!(matches!(
            validate_planned_propose(&PlannedGovernancePropose {
                target_key: "nope".into(),
                target_value: b"{}".to_vec(),
            }),
            Err(GovernanceError::UnknownTargetKey(_))
        ));
    }

    #[test]
    fn planned_wire_indices_follow_report_equivocation() {
        assert_eq!(PLANNED_WIRE_GOVERNANCE_PROPOSE, 37);
        assert_eq!(PLANNED_WIRE_GOVERNANCE_VOTE, 38);
        assert_eq!(PLANNED_WIRE_GOVERNANCE_ADVANCE_COOLING, 39);
    }
}
