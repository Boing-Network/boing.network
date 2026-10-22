//! Consensus slash + appeal book (Phase 14/15/16/17).
//!
//! FailedServe settle and equivocation/liveness slash-apply insert rows with consensus-allocated
//! `slash_id`s; `SubmitSlashAppeal` / `ResolveSlashAppeal` txs mutate this book so every validator
//! replays the same economics once a row exists.
//!
//! Phase 16: equal-weight N-of-M vote (⌈2M/3⌉).
//! Phase 17: stake-weighted ⌈2W/3⌉; vote records carry weight at cast; versioned persistence.

use std::collections::BTreeMap;

use boing_primitives::{
    hasher, phase17_resolve_threshold_met, AccountId, PHASE14_MAX_APPEAL_EVIDENCE_BYTES,
};
use serde::{Deserialize, Serialize};

/// Domain tag for SMT commitment of the slash-appeal registry (Phase 17 weighted votes).
pub const SLASH_APPEAL_REGISTRY_DOMAIN: &[u8] = b"BOING_SLASH_APPEAL_REGISTRY_V2";

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

/// One ResolveSlashAppeal vote (Phase 17): approve/reject plus bonded stake at cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveVote {
    pub approved: bool,
    pub weight: u128,
}

/// Outcome of casting a ResolveSlashAppeal vote (Phase 17).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastResolveOutcome {
    /// Threshold not yet met; slash remains unreverted.
    Pending {
        approve_weight: u128,
        reject_weight: u128,
        electorate_w: u128,
    },
    /// Approve or reject quorum reached; funds restored only when `approved` is true.
    Finalized { approved: bool },
}

/// Consensus slash record (seeded on FailedServe settle / equivocation / liveness).
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
    /// Phase 17: one vote per voter with stake weight recorded at cast.
    #[serde(default)]
    pub votes: BTreeMap<AccountId, ResolveVote>,
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
    #[error("voter already cast a resolve vote on this appeal")]
    AlreadyVoted,
    #[error("appeal window closed")]
    AppealWindowClosed,
    #[error("evidence exceeds max ({max} bytes)")]
    EvidenceTooLarge { max: usize },
    #[error("slash already reversed")]
    AlreadyReversed,
    #[error("slash id already recorded")]
    SlashIdExists,
}

/// Consensus registry of slashes and appeals (Phase 14/15/16/17).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashAppealRegistry {
    slashes: BTreeMap<u64, ConsensusSlashRecord>,
    appeals: BTreeMap<u64, ConsensusSlashAppeal>,
    /// Next appeal id.
    next_appeal_id: u64,
    /// Shared consensus slash-id allocator (Phase 15). Not persisted — derived via bump-past-max
    /// so Phase 14 `slash_appeals_registry.bin` files remain loadable.
    #[serde(skip)]
    next_slash_id: u64,
}

/// Phase 16 on-disk appeal shape (`votes: BTreeMap<AccountId, bool>`).
#[derive(Clone, Debug, Deserialize)]
struct LegacyPhase16Appeal {
    id: u64,
    slash_id: u64,
    evidence: Vec<u8>,
    status: ConsensusAppealStatus,
    #[serde(default)]
    votes: BTreeMap<AccountId, bool>,
}

#[derive(Clone, Debug, Deserialize)]
struct LegacyPhase16Registry {
    slashes: BTreeMap<u64, ConsensusSlashRecord>,
    appeals: BTreeMap<u64, LegacyPhase16Appeal>,
    next_appeal_id: u64,
}

/// Phase 14/15 on-disk appeal shape (no votes field).
#[derive(Clone, Debug, Deserialize)]
struct LegacyPhase14Appeal {
    id: u64,
    slash_id: u64,
    evidence: Vec<u8>,
    status: ConsensusAppealStatus,
}

#[derive(Clone, Debug, Deserialize)]
struct LegacyPhase14Registry {
    slashes: BTreeMap<u64, ConsensusSlashRecord>,
    appeals: BTreeMap<u64, LegacyPhase14Appeal>,
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
            next_slash_id: 1,
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

    /// Migrate Phase 16 bool votes → weighted records with `weight = 0`.
    pub fn migrate_bool_votes(votes: BTreeMap<AccountId, bool>) -> BTreeMap<AccountId, ResolveVote> {
        votes
            .into_iter()
            .map(|(id, approved)| {
                (
                    id,
                    ResolveVote {
                        approved,
                        weight: 0,
                    },
                )
            })
            .collect()
    }

    /// Build a registry from a legacy Phase 16 bare-bincode decode (bool votes → weight 0).
    pub fn from_legacy_phase16(legacy: &[u8]) -> Result<Self, String> {
        let leg: LegacyPhase16Registry =
            bincode::deserialize(legacy).map_err(|e| e.to_string())?;
        let mut reg = Self {
            slashes: leg.slashes,
            appeals: BTreeMap::new(),
            next_appeal_id: leg.next_appeal_id.max(1),
            next_slash_id: 1,
        };
        for (id, a) in leg.appeals {
            reg.appeals.insert(
                id,
                ConsensusSlashAppeal {
                    id: a.id,
                    slash_id: a.slash_id,
                    evidence: a.evidence,
                    status: a.status,
                    votes: Self::migrate_bool_votes(a.votes),
                },
            );
        }
        reg.bump_next_slash_id_past_max();
        Ok(reg)
    }

    /// Build a registry from a legacy Phase 14/15 bare-bincode decode (no votes → empty).
    pub fn from_legacy_phase14(legacy: &[u8]) -> Result<Self, String> {
        let leg: LegacyPhase14Registry =
            bincode::deserialize(legacy).map_err(|e| e.to_string())?;
        let mut reg = Self {
            slashes: leg.slashes,
            appeals: BTreeMap::new(),
            next_appeal_id: leg.next_appeal_id.max(1),
            next_slash_id: 1,
        };
        for (id, a) in leg.appeals {
            reg.appeals.insert(
                id,
                ConsensusSlashAppeal {
                    id: a.id,
                    slash_id: a.slash_id,
                    evidence: a.evidence,
                    status: a.status,
                    votes: BTreeMap::new(),
                },
            );
        }
        reg.bump_next_slash_id_past_max();
        Ok(reg)
    }

    /// Ensure allocator is strictly above any already-recorded slash id (upgrade hygiene).
    fn bump_next_slash_id_past_max(&mut self) {
        let max = self.slashes.keys().next_back().copied().unwrap_or(0);
        if self.next_slash_id <= max {
            self.next_slash_id = max.saturating_add(1);
        }
        if self.next_slash_id == 0 {
            self.next_slash_id = 1;
        }
    }

    /// Allocate a new consensus slash id and insert the row.
    pub fn record_slash(
        &mut self,
        account: AccountId,
        amount: u128,
        reason: ConsensusSlashReason,
        block_height: u64,
        appeal_window_blocks: u64,
    ) -> Result<u64, SlashAppealError> {
        self.bump_next_slash_id_past_max();
        let id = self.next_slash_id;
        self.next_slash_id = id.saturating_add(1);
        self.record_slash_with_id(
            id,
            account,
            amount,
            reason,
            block_height,
            appeal_window_blocks,
        )
    }

    /// Insert a slash with a known id (determinism when id already known). No-ops if `id`
    /// already exists. Bumps `next_slash_id` past `id` when needed.
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
            if self.next_slash_id <= id {
                self.next_slash_id = id.saturating_add(1);
            }
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
        if self.next_slash_id <= id {
            self.next_slash_id = id.saturating_add(1);
        }
        Ok(id)
    }

    /// Peek the next slash id that [`Self::record_slash`] would allocate (after bump-past-max).
    pub fn peek_next_slash_id(&self) -> u64 {
        let max = self.slashes.keys().next_back().copied().unwrap_or(0);
        let mut next = self.next_slash_id.max(1);
        if next <= max {
            next = max.saturating_add(1);
        }
        next
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
                votes: BTreeMap::new(),
            },
        );
        Ok(id)
    }

    /// Cast a ResolveSlashAppeal vote (Phase 17). Finalizes when approve or reject
    /// weight reaches ⌈2W/3⌉ of `electorate_w`. On approve finalize, marks the slash
    /// `reversed`. When `electorate_w == 0`, never finalizes (stay Pending).
    pub fn cast_resolve_vote(
        &mut self,
        appeal_id: u64,
        voter: AccountId,
        approved: bool,
        voter_weight: u128,
        electorate_w: u128,
    ) -> Result<CastResolveOutcome, SlashAppealError> {
        let slash_id = {
            let appeal = self
                .appeals
                .get_mut(&appeal_id)
                .ok_or(SlashAppealError::AppealNotFound)?;
            if appeal.status != ConsensusAppealStatus::Pending {
                return Err(SlashAppealError::AppealAlreadyResolved);
            }
            if appeal.votes.contains_key(&voter) {
                return Err(SlashAppealError::AlreadyVoted);
            }
            appeal.votes.insert(
                voter,
                ResolveVote {
                    approved,
                    weight: voter_weight,
                },
            );
            appeal.slash_id
        };

        let (approve_weight, reject_weight) = self.weight_tallies(appeal_id).expect("appeal present");

        if phase17_resolve_threshold_met(approve_weight, electorate_w) {
            self.finalize_appeal(appeal_id, slash_id, true)?;
            return Ok(CastResolveOutcome::Finalized { approved: true });
        }
        if phase17_resolve_threshold_met(reject_weight, electorate_w) {
            self.finalize_appeal(appeal_id, slash_id, false)?;
            return Ok(CastResolveOutcome::Finalized { approved: false });
        }

        Ok(CastResolveOutcome::Pending {
            approve_weight,
            reject_weight,
            electorate_w,
        })
    }

    fn finalize_appeal(
        &mut self,
        appeal_id: u64,
        slash_id: u64,
        approved: bool,
    ) -> Result<(), SlashAppealError> {
        {
            let appeal = self
                .appeals
                .get_mut(&appeal_id)
                .ok_or(SlashAppealError::AppealNotFound)?;
            appeal.status = if approved {
                ConsensusAppealStatus::Approved
            } else {
                ConsensusAppealStatus::Rejected
            };
        }
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
        Ok(())
    }

    /// Thin W=1 one-shot helper (tests / solo paths). Prefer [`Self::cast_resolve_vote`].
    pub fn resolve_appeal(
        &mut self,
        appeal_id: u64,
        approved: bool,
    ) -> Result<&ConsensusSlashRecord, SlashAppealError> {
        // Distinct sentinel so repeated legacy calls after finalize hit AppealAlreadyResolved,
        // not AlreadyVoted, matching Phase 14/15 test expectations.
        let voter = AccountId([0xff; 32]);
        match self.cast_resolve_vote(appeal_id, voter, approved, 1, 1)? {
            CastResolveOutcome::Finalized { .. } | CastResolveOutcome::Pending { .. } => {}
        }
        let slash_id = self
            .appeals
            .get(&appeal_id)
            .ok_or(SlashAppealError::AppealNotFound)?
            .slash_id;
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

    /// Approve / reject vote counts for RPC display.
    pub fn vote_tallies(&self, appeal_id: u64) -> Option<(usize, usize)> {
        let appeal = self.appeals.get(&appeal_id)?;
        let approve = appeal.votes.values().filter(|v| v.approved).count();
        let reject = appeal.votes.values().filter(|v| !v.approved).count();
        Some((approve, reject))
    }

    /// Approve / reject weight tallies (sum of recorded vote weights).
    pub fn weight_tallies(&self, appeal_id: u64) -> Option<(u128, u128)> {
        let appeal = self.appeals.get(&appeal_id)?;
        let mut approve = 0u128;
        let mut reject = 0u128;
        for v in appeal.votes.values() {
            if v.approved {
                approve = approve.saturating_add(v.weight);
            } else {
                reject = reject.saturating_add(v.weight);
            }
        }
        Some((approve, reject))
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

    fn seed_appeal(reg: &mut SlashAppealRegistry, slash_id: u64) -> u64 {
        reg.record_slash_with_id(
            slash_id,
            acct(1),
            1000,
            ConsensusSlashReason::BlobFailedServe,
            100,
            1000,
        )
        .unwrap();
        reg.submit_appeal(
            slash_id,
            b"evidence".to_vec(),
            150,
            PHASE14_MAX_APPEAL_EVIDENCE_BYTES,
        )
        .unwrap()
    }

    #[test]
    fn slash_appeal_submit_resolve_approved() {
        let mut reg = SlashAppealRegistry::new();
        let aid = seed_appeal(&mut reg, 7);
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
    fn phase17_w1_one_shot_finalize() {
        let mut reg = SlashAppealRegistry::new();
        let aid = seed_appeal(&mut reg, 1);
        let out = reg
            .cast_resolve_vote(aid, acct(10), true, 1, 1)
            .unwrap();
        assert_eq!(out, CastResolveOutcome::Finalized { approved: true });
        assert!(reg.is_reversed(1));
    }

    #[test]
    fn phase17_equal_stakes_needs_two_of_three() {
        // Stakes [100,100,100] → W=300; ⌈2*300/3⌉=200.
        let mut reg = SlashAppealRegistry::new();
        let aid = seed_appeal(&mut reg, 2);
        let out = reg
            .cast_resolve_vote(aid, acct(10), true, 100, 300)
            .unwrap();
        assert_eq!(
            out,
            CastResolveOutcome::Pending {
                approve_weight: 100,
                reject_weight: 0,
                electorate_w: 300
            }
        );
        assert!(!reg.is_reversed(2));
        assert_eq!(
            reg.get_appeal(aid).unwrap().status,
            ConsensusAppealStatus::Pending
        );
        let out2 = reg
            .cast_resolve_vote(aid, acct(11), true, 100, 300)
            .unwrap();
        assert_eq!(out2, CastResolveOutcome::Finalized { approved: true });
        assert!(reg.is_reversed(2));
        assert_eq!(
            reg.get_appeal(aid).unwrap().status,
            ConsensusAppealStatus::Approved
        );
    }

    #[test]
    fn phase17_whale_one_shot() {
        // Stakes [900,50,50] → W=1000; 900 alone meets ⌈2*1000/3⌉.
        let mut reg = SlashAppealRegistry::new();
        let aid = seed_appeal(&mut reg, 5);
        let out = reg
            .cast_resolve_vote(aid, acct(90), true, 900, 1000)
            .unwrap();
        assert_eq!(out, CastResolveOutcome::Finalized { approved: true });
        assert!(reg.is_reversed(5));
    }

    #[test]
    fn phase17_reject_path() {
        let mut reg = SlashAppealRegistry::new();
        let aid = seed_appeal(&mut reg, 3);
        assert!(matches!(
            reg.cast_resolve_vote(aid, acct(1), false, 100, 300)
                .unwrap(),
            CastResolveOutcome::Pending { .. }
        ));
        let out = reg
            .cast_resolve_vote(aid, acct(2), false, 100, 300)
            .unwrap();
        assert_eq!(out, CastResolveOutcome::Finalized { approved: false });
        assert!(!reg.is_reversed(3));
        assert_eq!(
            reg.get_appeal(aid).unwrap().status,
            ConsensusAppealStatus::Rejected
        );
    }

    #[test]
    fn phase17_w0_stays_pending() {
        let mut reg = SlashAppealRegistry::new();
        let aid = seed_appeal(&mut reg, 8);
        let out = reg
            .cast_resolve_vote(aid, acct(1), true, 0, 0)
            .unwrap();
        assert_eq!(
            out,
            CastResolveOutcome::Pending {
                approve_weight: 0,
                reject_weight: 0,
                electorate_w: 0
            }
        );
        assert!(!reg.is_reversed(8));
        assert_eq!(
            reg.get_appeal(aid).unwrap().status,
            ConsensusAppealStatus::Pending
        );
    }

    #[test]
    fn phase17_double_vote_rejected() {
        let mut reg = SlashAppealRegistry::new();
        let aid = seed_appeal(&mut reg, 4);
        reg.cast_resolve_vote(aid, acct(5), true, 100, 300)
            .unwrap();
        assert!(matches!(
            reg.cast_resolve_vote(aid, acct(5), false, 100, 300),
            Err(SlashAppealError::AlreadyVoted)
        ));
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
        let aid = reg
            .submit_appeal(9, b"x".to_vec(), 1, PHASE14_MAX_APPEAL_EVIDENCE_BYTES)
            .unwrap();
        reg.cast_resolve_vote(aid, acct(1), true, 50, 300).unwrap();
        let bytes = bincode::serialize(&reg).unwrap();
        let back: SlashAppealRegistry = bincode::deserialize(&bytes).unwrap();
        assert_eq!(back.list_slashes().len(), 1);
        assert_eq!(back.list_appeals().len(), 1);
        assert_eq!(back.get_appeal(aid).unwrap().votes.len(), 1);
        assert_eq!(
            back.get_appeal(aid).unwrap().votes.get(&acct(1)).unwrap().weight,
            50
        );
    }

    #[test]
    fn empty_votes_roundtrip() {
        let mut reg = SlashAppealRegistry::new();
        reg.record_slash_with_id(
            1,
            acct(1),
            10,
            ConsensusSlashReason::BlobFailedServe,
            1,
            10,
        )
        .unwrap();
        let aid = reg
            .submit_appeal(1, vec![], 1, PHASE14_MAX_APPEAL_EVIDENCE_BYTES)
            .unwrap();
        assert!(reg.get_appeal(aid).unwrap().votes.is_empty());
        let bytes = bincode::serialize(&reg).unwrap();
        let back: SlashAppealRegistry = bincode::deserialize(&bytes).unwrap();
        assert!(back.get_appeal(aid).unwrap().votes.is_empty());
    }

    #[test]
    fn migrate_phase16_bool_votes_weight_zero() {
        // Build a Phase-16-shaped registry via a temporary bool-vote layout.
        #[derive(Serialize)]
        struct P16Appeal {
            id: u64,
            slash_id: u64,
            evidence: Vec<u8>,
            status: ConsensusAppealStatus,
            votes: BTreeMap<AccountId, bool>,
        }
        #[derive(Serialize)]
        struct P16Reg {
            slashes: BTreeMap<u64, ConsensusSlashRecord>,
            appeals: BTreeMap<u64, P16Appeal>,
            next_appeal_id: u64,
        }
        let mut slashes = BTreeMap::new();
        slashes.insert(
            1,
            ConsensusSlashRecord {
                id: 1,
                account: acct(1),
                amount: 10,
                reason: ConsensusSlashReason::BlobFailedServe,
                block_height: 1,
                appeal_deadline: 100,
                reversed: false,
            },
        );
        let mut votes = BTreeMap::new();
        votes.insert(acct(2), true);
        let mut appeals = BTreeMap::new();
        appeals.insert(
            1,
            P16Appeal {
                id: 1,
                slash_id: 1,
                evidence: b"e".to_vec(),
                status: ConsensusAppealStatus::Pending,
                votes,
            },
        );
        let bytes = bincode::serialize(&P16Reg {
            slashes,
            appeals,
            next_appeal_id: 2,
        })
        .unwrap();
        let reg = SlashAppealRegistry::from_legacy_phase16(&bytes).unwrap();
        let v = reg.get_appeal(1).unwrap().votes.get(&acct(2)).unwrap();
        assert!(v.approved);
        assert_eq!(v.weight, 0);
        assert_eq!(reg.list_slashes().len(), 1);
    }

    #[test]
    fn migrate_phase14_no_votes() {
        #[derive(Serialize)]
        struct P14Appeal {
            id: u64,
            slash_id: u64,
            evidence: Vec<u8>,
            status: ConsensusAppealStatus,
        }
        #[derive(Serialize)]
        struct P14Reg {
            slashes: BTreeMap<u64, ConsensusSlashRecord>,
            appeals: BTreeMap<u64, P14Appeal>,
            next_appeal_id: u64,
        }
        let mut slashes = BTreeMap::new();
        slashes.insert(
            3,
            ConsensusSlashRecord {
                id: 3,
                account: acct(3),
                amount: 7,
                reason: ConsensusSlashReason::Equivocation,
                block_height: 2,
                appeal_deadline: 20,
                reversed: false,
            },
        );
        let mut appeals = BTreeMap::new();
        appeals.insert(
            1,
            P14Appeal {
                id: 1,
                slash_id: 3,
                evidence: vec![],
                status: ConsensusAppealStatus::Pending,
            },
        );
        let bytes = bincode::serialize(&P14Reg {
            slashes,
            appeals,
            next_appeal_id: 2,
        })
        .unwrap();
        let reg = SlashAppealRegistry::from_legacy_phase14(&bytes).unwrap();
        assert!(reg.get_appeal(1).unwrap().votes.is_empty());
        assert_eq!(reg.get_slash(3).unwrap().amount, 7);
    }

    #[test]
    fn shared_allocator_bumps_past_legacy_ids() {
        let mut reg = SlashAppealRegistry::new();
        for id in 1..=3u64 {
            reg.record_slash_with_id(
                id,
                acct(id as u8),
                10,
                ConsensusSlashReason::BlobFailedServe,
                1,
                100,
            )
            .unwrap();
        }
        assert_eq!(reg.peek_next_slash_id(), 4);
        let id = reg
            .record_slash(
                acct(9),
                50,
                ConsensusSlashReason::Equivocation,
                2,
                1000,
            )
            .unwrap();
        assert_eq!(id, 4);
        let id2 = reg
            .record_slash(
                acct(8),
                20,
                ConsensusSlashReason::Liveness,
                3,
                1000,
            )
            .unwrap();
        assert_eq!(id2, 5);
        assert_eq!(reg.list_slashes().len(), 5);
    }

    #[test]
    fn record_slash_allocates_monotonic_ids() {
        let mut reg = SlashAppealRegistry::new();
        let a = reg
            .record_slash(
                acct(1),
                100,
                ConsensusSlashReason::Equivocation,
                1,
                10,
            )
            .unwrap();
        let b = reg
            .record_slash(
                acct(2),
                200,
                ConsensusSlashReason::Liveness,
                2,
                10,
            )
            .unwrap();
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        assert_ne!(a, b);
    }
}
