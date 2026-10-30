//! Trigger conditions — when on-chain event X, run Y.

use boing_primitives::AccountId;

/// Condition that fires a trigger.
#[derive(Clone, Debug)]
pub enum TriggerCondition {
    /// Block height reached (one-shot once `height >= h` unless re-armed).
    BlockHeight(u64),
    /// Account balance exceeds threshold.
    BalanceExceeds { account: AccountId, threshold: u128 },
    /// Time elapsed (Unix timestamp).
    TimestampAfter(u64),
    /// Custom predicate: `"true"` / `"1"` / `"yes"` always fire; anything else never fires.
    /// Placeholder for host-provided predicates (audit #61 item 41 / 48).
    Custom(String),
}

/// Trigger — pairs a condition with an action ID.
#[derive(Clone, Debug)]
pub struct Trigger {
    pub id: u64,
    pub condition: TriggerCondition,
    pub action_id: u64,
    /// For [`TriggerCondition::BlockHeight`]: set after the first successful fire.
    pub fired: bool,
}

impl Trigger {
    pub fn new(id: u64, condition: TriggerCondition, action_id: u64) -> Self {
        Self {
            id,
            condition,
            action_id,
            fired: false,
        }
    }

    /// Check if condition is satisfied (simple eval; real impl would query chain).
    pub fn is_satisfied(&self, block_height: u64, _balance: Option<u128>, now_secs: u64) -> bool {
        match &self.condition {
            TriggerCondition::BlockHeight(h) => !self.fired && block_height >= *h,
            TriggerCondition::BalanceExceeds { threshold, .. } => {
                _balance.map(|b| b >= *threshold).unwrap_or(false)
            }
            TriggerCondition::TimestampAfter(t) => now_secs >= *t,
            TriggerCondition::Custom(pred) => {
                matches!(
                    pred.trim().to_ascii_lowercase().as_str(),
                    "true" | "1" | "yes"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_height_is_one_shot_when_marked_fired() {
        let mut t = Trigger::new(1, TriggerCondition::BlockHeight(10), 99);
        assert!(t.is_satisfied(10, None, 0));
        t.fired = true;
        assert!(!t.is_satisfied(11, None, 0));
    }

    #[test]
    fn custom_true_false() {
        let yes = Trigger::new(1, TriggerCondition::Custom("true".into()), 1);
        let no = Trigger::new(2, TriggerCondition::Custom("later".into()), 1);
        assert!(yes.is_satisfied(0, None, 0));
        assert!(!no.is_satisfied(0, None, 0));
    }
}
