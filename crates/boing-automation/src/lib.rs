//! Boing Automation — native scheduler and trigger primitives.
//!
//! Enables decentralized executors to run scheduled and event-driven tasks.
//!
//! **Wiring status (audit #61 item 41):** this crate is a library. The node does not yet run
//! [`Scheduler`] or [`TriggerRegistry`] in consensus; hosts must poll tip height into
//! [`Scheduler::run`] and feed [`EvalContext`] from chain state.

pub mod executor;
pub mod hooks;
pub mod scheduler;
pub mod trigger;
pub mod verification;

pub use executor::{AutomationTask, ExecutorIncentive};
pub use hooks::{ChainEvent, EvalContext, TriggerRegistry};
pub use scheduler::{CronSchedule, Scheduler};
pub use trigger::{Trigger, TriggerCondition};
pub use verification::{
    ExecutionProof, ExecutorAttestation, FraudProof, OracleAttestation, VerificationError, ZkpProof,
};
