//! Consensus slash + appeal book (Phase 14).
//!
//! FailedServe settle inserts a slash row with a known `slash_id`; `SubmitSlashAppeal` /
//! `ResolveSlashAppeal` txs mutate this book so every validator replays the same economics.

use std::collections::BTreeMap;

use boing_primitives::{hasher, AccountId, PHASE14_MAX_APPEAL_EVIDENCE_BYTES};
use serde::{Deserialize, Serialize};

/// Domain tag for SMT commitment of the slash-appeal registry.
pub const SLASH_APPEAL_REGISTRY_DOMAIN: &[u8] = b"BOING_SLASH_APPEAL_REGISTRY_V1";

/// Reason for a consensus slash record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusSlashReason {
    Equivocation,
    Liveness,
    Fraud,
    BlobFailedServe,
    Other(String),
}

/// Status of a consensus slash appeal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusAppealStatus {
    Pending,
    Approved,
    Rejected,
}

/// Consensus slash record (seeded on FailedServe settle with a known id).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusSlashRecord {
    pub id: u64,
    pub account: AccountId,
    pub amount: u128,
    pub reason: ConsensusSlashReason,
    pub block_height: u64,
    pub appeal_deadline: u64,
    /// Set when an approved appeal restored funds.
    pub reversed: bool,
}

/// Consensus appeal against a slash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusSlashAppeal {
    pub id: u64,
    pub slash_id: u64,
    pub evidence: Vec<u8>,
    pub status: ConsensusAppealStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SlashAppealError {
    #[error("slash record not found")]
    SlashNotFound,
    #[error("appeal not found")]
    AppealNotFound,
    #[error("appeal already exists for this slash")]
    AppealAlreadyExists,
    #[error("appeal already resolved")]
    AppealAlreadyResolved,
    #[error("appeal window closed")]
    AppealWindowClosed,
    #[error("evidence exceeds max ({max} bytes)")]
    EvidenceTooLarge { max: usize },
    #[error("slash already reversed")]
    AlreadyReversed,
    #[error("slash id already recorded")]
    SlashIdExists,
}

/// Consensus registry of slashes and appeals (Phase 14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashAppealRegistry {
    slashes: BTreeMap<u64, ConsensusSlashRecord>,
    appeals: BTreeMap<u64, ConsensusSlashAppeal>,
    /// Next appeal id (slash ids come from FailedServe / caller-chosen ids).
    next_appeal_id: u64,
}

impl Default for SlashAppealRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SlashAppealRegistry {
    pub fn new() -> Self {
        Self {
            slashes: BTreeMap::new(),
            appeals: BTreeMap::new(),
            next_appeal_id: 1,
        }
    }

    pub fn smt_key() -> [u8; 32] {
        let mut h = hasher();
        h.update(SLASH_APPEAL_REGISTRY_DOMAIN);
        *h.finalize().as_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.slashes.is_empty() && self.appeals.is_empty()
    }

    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher();
        h.update(SLASH_APPEAL_REGISTRY_DOMAIN);
        h.update(&bincode::serialize(self).unwrap_or_default());
        *h.finalize().as_bytes()
    }

    /// Insert a slash with a known id (FailedServe path). No-ops if `id` already exists.
    pub fn record_slash_with_id(
        &mut self,
        id: u64,
        account: AccountId,
        amount: u128,
        reason: ConsensusSlashReason,
        block_height: u64,
        appeal_window_blocks: u64,
    ) -> Result<u64, SlashAppealError> {
        if self.slashes.contains_key(&id) {
            return Ok(id);
        }
        self.slashes.insert(
            id,
            ConsensusSlashRecord {
                id,
                account,
                amount,
                reason,
                block_height,
                appeal_deadline: block_height.saturating_add(appeal_window_blocks),
                reversed: false,
            },
        );
        Ok(id)
    }

    /// Submit an appeal at `tip_height`. Evidence must be ≤ `max_evidence` (typically
    /// [`PHASE14_MAX_APPEAL_EVIDENCE_BYTES`]).
    pub fn submit_appeal(
        &mut self,
        slash_id: u64,
        evidence: Vec<u8>,
        tip_height: u64,
        max_evidence: usize,
    ) -> Result<u64, SlashAppealError> {
        if evidence.len() > max_evidence {
            return Err(SlashAppealError::EvidenceTooLarge { max: max_evidence });
        }
        let slash = self
            .slashes
            .get(&slash_id)
            .ok_or(SlashAppealError::SlashNotFound)?;
        if tip_height > slash.appeal_deadline {
            return Err(SlashAppealError::AppealWindowClosed);
        }
        if slash.reversed {
            return Err(SlashAppealError::AlreadyReversed);
        }
        if self.appeals.values().any(|a| a.slash_id == slash_id) {
            return Err(SlashAppealError::AppealAlreadyExists);
        }
        let id = self.next_appeal_id;
        self.next_appeal_id = id.saturating_add(1);
        self.appeals.insert(
            id,
            ConsensusSlashAppeal {
                id,
                slash_id,
                evidence,
                status: ConsensusAppealStatus::Pending,
            },
        );
        Ok(id)
    }

    /// Resolve a pending appeal. On approve, marks the slash `reversed` and returns a reference
    /// to the slash record for the caller to restore funds.
    pub fn resolve_appeal(
        &mut self,
        appeal_id: u64,
        approved: bool,
    ) -> Result<&ConsensusSlashRecord, SlashAppealError> {
        let slash_id = {
            let appeal = self
                .appeals
                .get_mut(&appeal_id)
                .ok_or(SlashAppealError::AppealNotFound)?;
            if appeal.status != ConsensusAppealStatus::Pending {
                return Err(SlashAppealError::AppealAlreadyResolved);
            }
            appeal.status = if approved {
                ConsensusAppealStatus::Approved
            } else {
                ConsensusAppealStatus::Rejected
            };
            appeal.slash_id
        };
        let slash = self
            .slashes
            .get_mut(&slash_id)
            .ok_or(SlashAppealError::SlashNotFound)?;
        if approved {
            if slash.reversed {
                return Err(SlashAppealError::AlreadyReversed);
            }
            slash.reversed = true;
        }
        Ok(self.slashes.get(&slash_id).expect("slash present"))
    }

    pub fn get_slash(&self, id: u64) -> Option<&ConsensusSlashRecord> {
        self.slashes.get(&id)
    }

    pub fn get_appeal(&self, id: u64) -> Option<&ConsensusSlashAppeal> {
        self.appeals.get(&id)
    }

    pub fn list_slashes(&self) -> Vec<&ConsensusSlashRecord> {
        self.slashes.values().collect()
    }

    pub fn list_appeals(&self) -> Vec<&ConsensusSlashAppeal> {
        self.appeals.values().collect()
    }

    pub fn is_reversed(&self, slash_id: u64) -> bool {
        self.slashes
            .get(&slash_id)
            .map(|s| s.reversed)
            .unwrap_or(false)
    }
}

/// Default evidence cap used by callers that do not override.
pub fn default_max_appeal_evidence() -> usize {
    PHASE14_MAX_APPEAL_EVIDENCE_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(b: u8) -> AccountId {
        AccountId([b; 32])
    }

    #[test]
    fn slash_appeal_submit_resolve_approved() {
        let mut reg = SlashAppealRegistry::new();
        reg.record_slash_with_id(
            7,
            acct(1),
            1000,
            ConsensusSlashReason::BlobFailedServe,
            100,
            1000,
        )
        .unwrap();
        let aid = reg
            .submit_appeal(7, b"evidence".to_vec(), 150, PHASE14_MAX_APPEAL_EVIDENCE_BYTES)
            .unwrap();
        assert_eq!(aid, 1);
        assert!(reg
            .submit_appeal(7, vec![], 150, PHASE14_MAX_APPEAL_EVIDENCE_BYTES)
            .is_err());
        let slash = reg.resolve_appeal(aid, true).unwrap();
        assert!(slash.reversed);
        assert!(reg.is_reversed(7));
        assert!(matches!(
            reg.resolve_appeal(aid, true),
            Err(SlashAppealError::AppealAlreadyResolved)
        ));
    }

    #[test]
    fn reject_does_not_reverse() {
        let mut reg = SlashAppealRegistry::new();
        reg.record_slash_with_id(
            1,
            acct(2),
            500,
            ConsensusSlashReason::BlobFailedServe,
            10,
            100,
        )
        .unwrap();
        let aid = reg
            .submit_appeal(1, vec![], 10, PHASE14_MAX_APPEAL_EVIDENCE_BYTES)
            .unwrap();
        reg.resolve_appeal(aid, false).unwrap();
        assert!(!reg.is_reversed(1));
    }

    #[test]
    fn window_closed_and_evidence_oversize() {
        let mut reg = SlashAppealRegistry::new();
        reg.record_slash_with_id(
            1,
            acct(3),
            100,
            ConsensusSlashReason::BlobFailedServe,
            10,
            5,
        )
        .unwrap();
        assert!(matches!(
            reg.submit_appeal(1, vec![], 16, PHASE14_MAX_APPEAL_EVIDENCE_BYTES),
            Err(SlashAppealError::AppealWindowClosed)
        ));
        assert!(matches!(
            reg.submit_appeal(
                1,
                vec![0u8; PHASE14_MAX_APPEAL_EVIDENCE_BYTES + 1],
                10,
                PHASE14_MAX_APPEAL_EVIDENCE_BYTES
            ),
            Err(SlashAppealError::EvidenceTooLarge { .. })
        ));
    }

    #[test]
    fn serde_roundtrip() {
        let mut reg = SlashAppealRegistry::new();
        reg.record_slash_with_id(
            9,
            acct(9),
            42,
            ConsensusSlashReason::Equivocation,
            1,
            10,
        )
        .unwrap();
        let _ = reg
            .submit_appeal(9, b"x".to_vec(), 1, PHASE14_MAX_APPEAL_EVIDENCE_BYTES)
            .unwrap();
        let bytes = bincode::serialize(&reg).unwrap();
        let back: SlashAppealRegistry = bincode::deserialize(&bytes).unwrap();
        assert_eq!(back.list_slashes().len(), 1);
        assert_eq!(back.list_appeals().len(), 1);
    }
}
