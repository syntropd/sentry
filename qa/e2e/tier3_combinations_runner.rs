//! Tier 3: Cross-Feature Combinations Integration Runner
//!
//! Executes all pairwise cross-feature combination test suites.

#[path = "scenarios/coredump_triage.rs"]
mod coredump_triage;
#[path = "scenarios/circuit_flap_lockout.rs"]
mod circuit_flap_lockout;
#[path = "scenarios/journal_psi_telemetry.rs"]
mod journal_psi_telemetry;
#[path = "scenarios/advisory_policy_gate.rs"]
mod advisory_policy_gate;
#[path = "scenarios/incident_mcp_dispatch.rs"]
mod incident_mcp_dispatch;
#[path = "scenarios/notify_circuit_alert.rs"]
mod notify_circuit_alert;
