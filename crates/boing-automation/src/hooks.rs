//! Trigger-based execution hooks — evaluate triggers on chain events.
//!
//! When a block is imported or a transaction is executed, the hook evaluates
//! registered triggers and returns action IDs to run.

use std::collections::HashMap;

use boing_primitives::{AccountId, Hash};

use crate::trigger::{Trigger, TriggerCondition};

/// Chain event that can fire triggers.
#[derive(Clone, Debug)]
pub enum ChainEvent {
    /// A new block was imported.
    BlockImported {
        height: u64,
        hash: Hash,
        proposer: AccountId,
        tx_count: usize,
    },
    /// A transaction was executed in a block.
    TxExecuted {
        block_height: u64,
        tx_id: Hash,
        sender: AccountId,
    },
}

/// Context for evaluating triggers (block height, timestamp, balance lookup).
#[derive(Clone, Debug, Default)]
pub struct EvalContext {
    pub block_height: u64,
    pub timestamp_secs: u64,
    /// Optional balance lookup: account -> balance.
    pub balances: HashMap<AccountId, u128>,
}

impl EvalContext {
    pub fn new(block_height: u64, timestamp_secs: u64) -> Self {
        Self {
            block_height,
            timestamp_secs,
            balances: HashMap::new(),
        }
    }

    pub fn with_balance(mut self, account: AccountId, balance: u128) -> Self {
        self.balances.insert(account, balance);
        self
    }

    fn balance_for(&self, account: &AccountId) -> Option<u128> {
        self.balances.get(account).copied()
    }
}

/// Registry of triggers; evaluates them against chain events.
#[derive(Clone, Debug)]
pub struct TriggerRegistry {
    triggers: HashMap<u64, Trigger>,
    next_id: u64,
}

impl TriggerRegistry {
    pub fn new() -> Self {
        Self {
            triggers: HashMap::new(),
            next_id: 0,
        }
    }

    /// Register a trigger; returns its ID.
    pub fn register(&mut self, condition: TriggerCondition, action_id: u64) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.triggers
            .insert(id, Trigger::new(id, condition, action_id));
        id
    }

    /// Unregister a trigger by ID.
    pub fn unregister(&mut self, id: u64) -> Option<Trigger> {
        self.triggers.remove(&id)
    }

    /// Evaluate all triggers against an event; returns action IDs that should run.
    pub fn evaluate(&self, _event: &ChainEvent, ctx: &EvalContext) -> Vec<u64> {
        let mut fired = Vec::new();
        for trigger in self.triggers.values() {
            let balance = match &trigger.condition {
                TriggerCondition::BalanceExceeds { account, .. } => ctx.balance_for(account),
                _ => None,
            };
            if trigger.is_satisfied(ctx.block_height, balance, ctx.timestamp_secs) {
                fired.push(trigger.action_id);
            }
        }
        fired
    }

    /// Process a chain event; returns action IDs to execute.
    pub fn on_event(&self, event: &ChainEvent, ctx: &EvalContext) -> Vec<u64> {
        self.evaluate(event, ctx)
    }

    /// Number of registered triggers.
    pub fn len(&self) -> usize {
        self.triggers.len()
    }

    /// Check if any triggers are registered.
    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }
}

impl Default for TriggerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
