//! Test trigger-based execution hooks.

use boing_automation::{ChainEvent, EvalContext, TriggerCondition, TriggerRegistry};
use boing_primitives::{AccountId, Hash};

#[test]
fn test_block_height_trigger_fires() {
    let mut reg = TriggerRegistry::new();
    reg.register(TriggerCondition::BlockHeight(10), 100);

    let event = ChainEvent::BlockImported {
        height: 10,
        hash: Hash::ZERO,
        proposer: AccountId([1u8; 32]),
        tx_count: 0,
    };
    let ctx = EvalContext::new(10, 1000);
    let fired = reg.on_event(&event, &ctx);
    assert_eq!(fired, vec![100]);
    // One-shot: later heights must not re-fire (audit #61 item 48).
    assert!(reg
        .on_event(&event, &EvalContext::new(11, 1001))
        .is_empty());
}

#[test]
fn test_block_height_trigger_not_yet() {
    let mut reg = TriggerRegistry::new();
    reg.register(TriggerCondition::BlockHeight(10), 100);

    let event = ChainEvent::BlockImported {
        height: 9,
        hash: Hash::ZERO,
        proposer: AccountId([1u8; 32]),
        tx_count: 0,
    };
    let ctx = EvalContext::new(9, 999);
    let fired = reg.on_event(&event, &ctx);
    assert!(fired.is_empty());
}

#[test]
fn test_balance_exceeds_trigger() {
    let mut reg = TriggerRegistry::new();
    let account = AccountId([2u8; 32]);
    reg.register(
        TriggerCondition::BalanceExceeds {
            account,
            threshold: 1000,
        },
        200,
    );

    let event = ChainEvent::BlockImported {
        height: 5,
        hash: Hash::ZERO,
        proposer: AccountId([1u8; 32]),
        tx_count: 1,
    };
    let ctx = EvalContext::new(5, 100).with_balance(account, 1500);
    let fired = reg.on_event(&event, &ctx);
    assert_eq!(fired, vec![200]);
}

#[test]
fn test_balance_exceeds_below_threshold() {
    let mut reg = TriggerRegistry::new();
    let account = AccountId([2u8; 32]);
    reg.register(
        TriggerCondition::BalanceExceeds {
            account,
            threshold: 1000,
        },
        200,
    );

    let ctx = EvalContext::new(5, 100).with_balance(account, 500);
    let event = ChainEvent::BlockImported {
        height: 5,
        hash: Hash::ZERO,
        proposer: AccountId([1u8; 32]),
        tx_count: 0,
    };
    let fired = reg.on_event(&event, &ctx);
    assert!(fired.is_empty());
}

#[test]
fn test_timestamp_trigger() {
    let mut reg = TriggerRegistry::new();
    reg.register(TriggerCondition::TimestampAfter(1000), 300);

    let event = ChainEvent::BlockImported {
        height: 0,
        hash: Hash::ZERO,
        proposer: AccountId([1u8; 32]),
        tx_count: 0,
    };
    let ctx = EvalContext::new(0, 1500);
    let fired = reg.on_event(&event, &ctx);
    assert_eq!(fired, vec![300]);
}

#[test]
fn test_multiple_triggers_fire() {
    let mut reg = TriggerRegistry::new();
    reg.register(TriggerCondition::BlockHeight(5), 10);
    reg.register(TriggerCondition::BlockHeight(5), 20);
    reg.register(TriggerCondition::BlockHeight(10), 30);

    let event = ChainEvent::BlockImported {
        height: 5,
        hash: Hash::ZERO,
        proposer: AccountId([1u8; 32]),
        tx_count: 0,
    };
    let ctx = EvalContext::new(5, 0);
    let fired = reg.on_event(&event, &ctx);
    assert_eq!(fired.len(), 2);
    assert!(fired.contains(&10));
    assert!(fired.contains(&20));
}
