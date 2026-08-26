//! Governance JSON for the Unsure QA pool — membership, anti-congestion caps, vote thresholds, voter rewards.
//!
//! Apply via governance proposal `target_key` [`GOVERNANCE_QA_POOL_CONFIG_KEY`] or load from `qa_pool_config.json` on the node.
//! Production public membership (`public_membership`) lets any 32-byte account call `boing_qaPoolVote`;
//! `dev_open_voting` remains a local-dev shortcut when administrators are empty.

use boing_primitives::AccountId;
use serde::{Deserialize, Serialize};

/// JSON `default_on_expiry` field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QaPoolExpiryPolicy {
    #[default]
    Reject,
    Allow,
}

fn default_max_pending() -> u32 {
    32
}

fn default_max_per_deployer() -> u32 {
    2
}

fn default_review_secs() -> u64 {
    7 * 24 * 60 * 60
}

fn default_quorum() -> f64 {
    0.5
}

fn default_threshold() -> f64 {
    2.0 / 3.0
}

fn default_min_quorum_votes() -> u32 {
    3
}

fn default_reward_per_counted_vote() -> u128 {
    1
}

/// Serializable governance payload for the QA pool. Keeps the queue **bounded** so review cannot be flooded.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QaPoolGovernanceConfig {
    /// 32-byte account IDs as hex strings (optional `0x`). Used when `public_membership` is false
    /// (and `dev_open_voting` is false or admins are non-empty).
    #[serde(default)]
    pub administrators: Vec<String>,
    /// Max concurrent pending Unsure items globally. **Anti-congestion:** when reached, new Unsure submissions are refused.
    /// Set to `0` to disable the pool (no Unsure enqueue).
    #[serde(default = "default_max_pending")]
    pub max_pending_items: u32,
    /// Max pending pool slots per deployer (`0` = unlimited).
    #[serde(default = "default_max_per_deployer")]
    pub max_pending_per_deployer: u32,
    #[serde(default = "default_review_secs")]
    pub review_window_secs: u64,
    #[serde(default = "default_quorum")]
    pub quorum_fraction: f64,
    #[serde(default = "default_threshold")]
    pub allow_threshold_fraction: f64,
    #[serde(default = "default_threshold")]
    pub reject_threshold_fraction: f64,
    #[serde(default)]
    pub default_on_expiry: QaPoolExpiryPolicy,
    /// When true **and** `administrators` is empty, any account may vote (development / open testnet only).
    /// Prefer [`Self::public_membership`] for production.
    #[serde(default)]
    pub dev_open_voting: bool,
    /// Production public-membership model: any 32-byte account may vote (subject to eligibility).
    #[serde(default)]
    pub public_membership: bool,
    /// Minimum active stake required to vote when `public_membership` is true (`0` = no stake gate).
    #[serde(default)]
    pub min_voter_stake: u128,
    /// Absolute minimum Allow+Reject votes to meet quorum under public membership (anti-sybil vs electorate size 1).
    #[serde(default = "default_min_quorum_votes")]
    pub min_quorum_votes: u32,
    /// Whole-BOING paid from the protocol treasury to each counted voter when a vote is applied
    /// toward quorum. `0` disables payouts.
    #[serde(default = "default_reward_per_counted_vote")]
    pub reward_per_counted_vote: u128,
    /// If true, Abstain is paid the same as Allow/Reject. Default false: abstain does **not** pay
    /// and does **not** count toward quorum.
    #[serde(default)]
    pub pay_abstain: bool,
}

impl Default for QaPoolGovernanceConfig {
    fn default() -> Self {
        Self {
            administrators: Vec::new(),
            max_pending_items: default_max_pending(),
            max_pending_per_deployer: default_max_per_deployer(),
            review_window_secs: default_review_secs(),
            quorum_fraction: default_quorum(),
            allow_threshold_fraction: default_threshold(),
            reject_threshold_fraction: default_threshold(),
            default_on_expiry: QaPoolExpiryPolicy::Reject,
            dev_open_voting: false,
            public_membership: false,
            min_voter_stake: 0,
            min_quorum_votes: default_min_quorum_votes(),
            reward_per_counted_vote: default_reward_per_counted_vote(),
            pay_abstain: false,
        }
    }
}

impl QaPoolGovernanceConfig {
    /// Production defaults: public membership, bounded queue, 3-vote quorum floor, 1 BOING per counted vote.
    pub fn production_default() -> Self {
        Self {
            public_membership: true,
            min_quorum_votes: default_min_quorum_votes(),
            reward_per_counted_vote: default_reward_per_counted_vote(),
            ..Self::default()
        }
    }

    /// Local tests and dev nodes: open voting, generous caps, 1-vote quorum (legacy `dev_open_voting`).
    pub fn development_default() -> Self {
        Self {
            administrators: Vec::new(),
            max_pending_items: 256,
            max_pending_per_deployer: 16,
            review_window_secs: 7 * 24 * 60 * 60,
            quorum_fraction: 0.5,
            allow_threshold_fraction: 2.0 / 3.0,
            reject_threshold_fraction: 2.0 / 3.0,
            default_on_expiry: QaPoolExpiryPolicy::Reject,
            dev_open_voting: true,
            public_membership: false,
            min_voter_stake: 0,
            min_quorum_votes: 1,
            reward_per_counted_vote: 0,
            pay_abstain: false,
        }
    }

    /// Parse administrator hex strings to account IDs (invalid entries skipped).
    pub fn administrator_accounts(&self) -> Vec<AccountId> {
        self.administrators
            .iter()
            .filter_map(|s| parse_account_hex(s))
            .collect()
    }

    /// Whether new Unsure items may be enqueued (governance + capacity policy).
    pub fn accepts_new_pending(&self) -> bool {
        if self.max_pending_items == 0 {
            return false;
        }
        self.public_membership
            || self.dev_open_voting
            || !self.administrator_accounts().is_empty()
    }

    /// Membership gate (ignores stake and deployer conflict). Used by the in-memory pool.
    pub fn voter_may_vote(&self, voter: AccountId) -> bool {
        self.voter_eligibility(voter, None, None).is_ok()
    }

    /// Full eligibility: membership, optional min stake, deployer cannot vote on their own item.
    pub fn voter_eligibility(
        &self,
        voter: AccountId,
        deployer: Option<AccountId>,
        voter_stake: Option<u128>,
    ) -> Result<(), QaVoterIneligible> {
        if let Some(d) = deployer {
            if d == voter {
                return Err(QaVoterIneligible::DeployerConflict);
            }
        }
        if self.public_membership {
            if self.min_voter_stake > 0 {
                let stake = voter_stake.unwrap_or(0);
                if stake < self.min_voter_stake {
                    return Err(QaVoterIneligible::InsufficientStake);
                }
            }
            return Ok(());
        }
        let admins = self.administrator_accounts();
        if self.dev_open_voting && admins.is_empty() {
            return Ok(());
        }
        if admins.contains(&voter) {
            return Ok(());
        }
        Err(QaVoterIneligible::NotMember)
    }

    /// Effective electorate size for quorum math.
    /// Public membership uses `min_quorum_votes` as the denominator (not an unbounded public set).
    pub fn effective_electorate_size(&self) -> usize {
        if self.public_membership {
            return (self.min_quorum_votes as usize).max(1);
        }
        let admins = self.administrator_accounts();
        if self.dev_open_voting && admins.is_empty() {
            1
        } else {
            admins.len().max(1)
        }
    }

    /// Allow/Reject/Pending from counted votes (Abstain already excluded from `allow`/`reject`).
    pub fn quorum_decision(&self, allow: usize, reject: usize) -> QaQuorumDecision {
        let total = allow.saturating_add(reject);
        let electorate = self.effective_electorate_size().max(1);
        if (total as f64 / electorate as f64) < self.quorum_fraction {
            return QaQuorumDecision::Pending;
        }
        if total == 0 {
            return QaQuorumDecision::Pending;
        }
        let allow_ratio = allow as f64 / total as f64;
        let reject_ratio = reject as f64 / total as f64;
        if allow_ratio >= self.allow_threshold_fraction {
            return QaQuorumDecision::Allow;
        }
        if reject_ratio >= self.reject_threshold_fraction {
            return QaQuorumDecision::Reject;
        }
        QaQuorumDecision::Pending
    }
}

/// Outcome of [`QaPoolGovernanceConfig::quorum_decision`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QaQuorumDecision {
    Pending,
    Allow,
    Reject,
}

/// Why a voter cannot cast a QA pool vote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QaVoterIneligible {
    NotMember,
    DeployerConflict,
    InsufficientStake,
}

fn parse_account_hex(s: &str) -> Option<AccountId> {
    let s = s.trim();
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(AccountId(arr))
}

/// Governance proposal key for [`QaPoolGovernanceConfig`] JSON (`target_value`).
pub const GOVERNANCE_QA_POOL_CONFIG_KEY: &str = "qa_pool_config";

/// Deserialize pool config from JSON bytes (e.g. governance execution or `qa_pool_config.json`).
pub fn qa_pool_config_from_json(bytes: &[u8]) -> Result<QaPoolGovernanceConfig, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_accepts_with_public_membership() {
        let c = QaPoolGovernanceConfig::production_default();
        assert!(c.accepts_new_pending());
        assert!(c.public_membership);
        assert!(c.voter_may_vote(AccountId::from_bytes([9u8; 32])));
        assert_eq!(c.effective_electorate_size(), 3);
    }

    #[test]
    fn development_accepts_with_open_voting() {
        let c = QaPoolGovernanceConfig::development_default();
        assert!(c.accepts_new_pending());
        assert!(!c.public_membership);
    }

    #[test]
    fn deployer_cannot_vote_on_own_item() {
        let c = QaPoolGovernanceConfig::production_default();
        let d = AccountId::from_bytes([1u8; 32]);
        assert_eq!(
            c.voter_eligibility(d, Some(d), Some(0)),
            Err(QaVoterIneligible::DeployerConflict)
        );
    }

    #[test]
    fn min_stake_gate() {
        let mut c = QaPoolGovernanceConfig::production_default();
        c.min_voter_stake = 100;
        let v = AccountId::from_bytes([2u8; 32]);
        assert_eq!(
            c.voter_eligibility(v, None, Some(0)),
            Err(QaVoterIneligible::InsufficientStake)
        );
        assert!(c.voter_eligibility(v, None, Some(100)).is_ok());
    }
}
