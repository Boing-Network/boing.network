//! Transparent slashing with appeal — auditable records and appeal flow.
//!
//! Slashed validators can submit appeals; governance decides via phased proposal.
//! Delegators see exactly why validators were slashed.

use std::collections::HashMap;

/// Reason for a slash (auditable, transparent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlashReason {
    /// Equivocation in consensus (double-sign, conflicting votes).
    Equivocation,
    /// Missed blocks or liveness failure.
    Liveness,
    /// Incorrect execution or fraud proof.
    Fraud,
    /// Other (should include details).
    Other(String),
}

/// Record of a slashing event — transparent and auditable.
#[derive(Clone, Debug)]
pub struct SlashRecord {
    pub id: u64,
    /// Validator account that was slashed.
    pub validator: [u8; 32],
    pub amount: u128,
    pub reason: SlashReason,
    pub block_height: u64,
    /// Appeal deadline (block height); after this, appeal window closes.
    pub appeal_deadline: u64,
}

/// Status of an appeal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppealStatus {
    Pending,
    Approved,
    Rejected,
}

/// An appeal against a slash — validator contests with evidence.
#[derive(Clone, Debug)]
pub struct SlashingAppeal {
    pub id: u64,
    pub slash_id: u64,
    /// Evidence or justification (opaque bytes; could be fraud proof refutation, etc.).
    pub evidence: Vec<u8>,
    pub status: AppealStatus,
}

/// Registry of slashes and appeals — transparent, auditable.
pub struct SlashRegistry {
    slashes: HashMap<u64, SlashRecord>,
    appeals: HashMap<u64, SlashingAppeal>,
    next_slash_id: u64,
    next_appeal_id: u64,
}

impl SlashRegistry {
    pub fn new() -> Self {
        Self {
            slashes: HashMap::new(),
            appeals: HashMap::new(),
            next_slash_id: 1,
            next_appeal_id: 1,
        }
    }

    /// Record a slash. Returns slash ID.
    pub fn record_slash(
        &mut self,
        validator: [u8; 32],
        amount: u128,
        reason: SlashReason,
        block_height: u64,
        appeal_window_blocks: u64,
    ) -> u64 {
        let id = self.next_slash_id;
        self.next_slash_id += 1;
        self.slashes.insert(
            id,
            SlashRecord {
                id,
                validator,
                amount,
                reason: reason.clone(),
                block_height,
                appeal_deadline: block_height.saturating_add(appeal_window_blocks),
            },
        );
        id
    }

    /// Submit an appeal for a slash at `current_height`. Returns appeal ID or error.
    pub fn submit_appeal(
        &mut self,
        slash_id: u64,
        evidence: Vec<u8>,
        current_height: u64,
    ) -> Result<u64, SlashingError> {
        let result = (|| {
            let slash = self.slashes.get(&slash_id).ok_or(SlashingError::SlashNotFound)?;
            if current_height > slash.appeal_deadline {
                return Err(SlashingError::AppealWindowClosed);
            }
            if self.appeals.values().any(|a| a.slash_id == slash_id) {
                return Err(SlashingError::AppealAlreadyExists);
            }
            let id = self.next_appeal_id;
            self.next_appeal_id += 1;
            self.appeals.insert(
                id,
                SlashingAppeal {
                    id,
                    slash_id,
                    evidence,
                    status: AppealStatus::Pending,
                },
            );
            Ok(id)
        })();
        if let Err(ref e) = result {
            boing_telemetry::component_warn(
                "boing_governance::slashing",
                "governance",
                "submit_appeal_failed",
                e,
            );
        }
        result
    }

    /// Resolve an appeal (called when governance proposal executes).
    pub fn resolve_appeal(&mut self, appeal_id: u64, approved: bool) -> Result<(), SlashingError> {
        let result = (|| {
            let appeal = self
                .appeals
                .get_mut(&appeal_id)
                .ok_or(SlashingError::AppealNotFound)?;
            if appeal.status != AppealStatus::Pending {
                return Err(SlashingError::AppealAlreadyResolved);
            }
            appeal.status = if approved {
                AppealStatus::Approved
            } else {
                AppealStatus::Rejected
            };
            Ok(())
        })();
        if let Err(ref e) = result {
            boing_telemetry::component_warn(
                "boing_governance::slashing",
                "governance",
                "resolve_appeal_failed",
                e,
            );
        }
        result
    }

    /// Check if a slash was successfully appealed (reversed).
    pub fn is_slash_reversed(&self, slash_id: u64) -> bool {
        self.appeals
            .values()
            .any(|a| a.slash_id == slash_id && a.status == AppealStatus::Approved)
    }

    pub fn get_slash(&self, id: u64) -> Option<&SlashRecord> {
        self.slashes.get(&id)
    }

    pub fn get_appeal(&self, id: u64) -> Option<&SlashingAppeal> {
        self.appeals.get(&id)
    }

    /// List all slashes (for transparency / delegator visibility).
    pub fn list_slashes(&self) -> Vec<&SlashRecord> {
        self.slashes.values().collect()
    }
}

impl Default for SlashRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_and_appeal() {
        let mut reg = SlashRegistry::new();
        let validator = [1u8; 32];
        let slash_id = reg.record_slash(
            validator,
            1000,
            SlashReason::Equivocation,
            100,
            1000,
        );
        assert_eq!(slash_id, 1);
        assert!(!reg.is_slash_reversed(slash_id));

        let appeal_id = reg
            .submit_appeal(slash_id, b"evidence".to_vec(), 150)
            .unwrap();
        assert_eq!(appeal_id, 1);
        assert!(reg
            .submit_appeal(slash_id, vec![], 150)
            .is_err()); // duplicate

        reg.resolve_appeal(appeal_id, true).unwrap();
        assert!(reg.is_slash_reversed(slash_id));
    }

    #[test]
    fn test_appeal_rejected() {
        let mut reg = SlashRegistry::new();
        let slash_id = reg.record_slash([2u8; 32], 500, SlashReason::Liveness, 50, 100);
        let appeal_id = reg.submit_appeal(slash_id, vec![], 50).unwrap();
        reg.resolve_appeal(appeal_id, false).unwrap();
        assert!(!reg.is_slash_reversed(slash_id));
    }

    #[test]
    fn test_appeal_window_closed() {
        let mut reg = SlashRegistry::new();
        let slash_id = reg.record_slash([3u8; 32], 100, SlashReason::Equivocation, 10, 5);
        assert!(matches!(
            reg.submit_appeal(slash_id, vec![], 16),
            Err(SlashingError::AppealWindowClosed)
        ));
        assert!(reg.submit_appeal(slash_id, vec![], 15).is_ok());
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SlashingError {
    #[error("Slash record not found")]
    SlashNotFound,
    #[error("Appeal not found")]
    AppealNotFound,
    #[error("Appeal already exists for this slash")]
    AppealAlreadyExists,
    #[error("Appeal already resolved")]
    AppealAlreadyResolved,
    #[error("Appeal window closed")]
    AppealWindowClosed,
}
