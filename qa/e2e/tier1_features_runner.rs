//! Tier 1: Feature Coverage Integration Runner
//!
//! Executes all Tier 1 tests covering R1 through R7.

#[path = "harness/harness_models.rs"]
pub mod harness_models;

#[path = "harness/harness_circuit.rs"]
pub mod harness_circuit;

#[path = "harness/harness_policy.rs"]
pub mod harness_policy;

#[path = "harness/harness_notify.rs"]
pub mod harness_notify;

#[path = "tiers/r1_architecture.rs"]
mod r1_architecture;
#[path = "tiers/r2_integration.rs"]
mod r2_integration;
#[path = "tiers/r3_diagnostic.rs"]
mod r3_diagnostic;
#[path = "tiers/r4_safety.rs"]
mod r4_safety;
#[path = "tiers/r5_ux.rs"]
mod r5_ux;
#[path = "tiers/r6_qa.rs"]
mod r6_qa;
#[path = "tiers/r7_packaging.rs"]
mod r7_packaging;
