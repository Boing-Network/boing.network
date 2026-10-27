//! Test scheduler.

use boing_automation::{CronSchedule, Trigger, TriggerCondition};

#[test]
fn test_cron_every_secs() {
    let s = CronSchedule::every_secs(10);
    assert!(s.should_run(0, 10));
    assert!(s.should_run(0, 20));
    assert!(!s.should_run(0, 5));
}

#[test]
fn test_cron_every_blocks() {
    let s = CronSchedule::every_blocks(100);
    assert!(s.should_run(0, 0));
    assert!(s.should_run(100, 0));
    assert!(s.should_run(200, 0));
    assert!(!s.should_run(50, 0));
}

#[test]
fn test_trigger_condition() {
    let t = Trigger::new(1, TriggerCondition::BlockHeight(5), 10);
    assert!(!t.is_satisfied(4, None, 0));
    assert!(t.is_satisfied(5, None, 0));
    assert!(t.is_satisfied(10, None, 0));
}
