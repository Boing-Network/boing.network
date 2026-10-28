//! Trigger conditions — when on-chain event X, run Y.

use boing_primitives::AccountId;

/// Condition that fires a trigger.
#[derive(Clone, Debug)]
pub enum TriggerCondition {
    /// Block height reached.
    BlockHeight(u64),
    /// Account balance exceeds threshold.
    BalanceExceeds { account: AccountId, threshold: u128 },
    /// Time elapsed (Unix timestamp).
    TimestampAfter(u64),
    /// Custom predicate (placeholder for future extension).
    Custom(String),
}

/// Trigger — pairs a condition with an action ID.
#[derive(Clone, Debug)]
pub struct Trigger {
    pub id: u64,
    pub condition: TriggerCondition,
    pub action_id: u64,
}

impl Trigger {
    pub fn new(id: u64, condition: TriggerCondition, action_id: u64) -> Self {
        Self {
            id,
            condition,
            action_id,
        }
    }

    /// Check if condition is satisfied (simple eval; real impl would query chain).
    pub fn is_satisfied(&self, block_height: u64, _balance: Option<u128>, now_secs: u64) -> bool {
        match &self.condition {
            TriggerCondition::BlockHeight(h) => block_height >= *h,
            TriggerCondition::BalanceExceeds { threshold, .. } => {
                _balance.map(|b| b >= *threshold).unwrap_or(false)
            }
            TriggerCondition::TimestampAfter(t) => now_secs >= *t,
            TriggerCondition::Custom(_) => false,
        }
    }
}
