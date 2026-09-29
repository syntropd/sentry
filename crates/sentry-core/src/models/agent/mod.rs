//! Pure domain models for autonomous agentic triage loops.
//!
//! Encapsulates the core planning, observation recording, and diagnostic verdict
//! abstractions utilized by the autonomous triage loops in Sentry.
//!
//! Modules:
//! - [`agent_plan`]: Multi-turn diagnostic plan state.
//! - [`agent_verdict`]: Final synthesized triage verdict and rationale.
//! - [`step_record`]: Record of executed tool or context query and reflection.

pub mod agent_plan;
pub mod agent_verdict;
pub mod step_record;

pub use agent_plan::AgentPlan;
pub use agent_verdict::AgentVerdict;
pub use step_record::{AgentAction, StepRecord};
