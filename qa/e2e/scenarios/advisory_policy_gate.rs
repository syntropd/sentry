//! Tier 3: Cross-Feature: Advisory LLM Gate & Policy Whitelist Enforcement
//!
//! Validates that LLM suggestions are filtered against policy whitelist and blacklists.

#[path = "../harness/harness_models.rs"]
mod harness_models;

#[path = "../harness/harness_policy.rs"]
mod harness_policy;

use harness_models::{RemediationAction, Severity};
use harness_policy::{PolicyEngine, PolicyVerdict};

#[test]
fn test_tier3_llm_restart_vetoed_for_disallowed_database_service() {
    let mut policy = PolicyEngine::default();
    // Configure postgresql.service to only allow NO_ACTION
    let mut pg_allowed = std::collections::HashSet::new();
    pg_allowed.insert(RemediationAction::NoAction);
    policy.unit_allowed_actions.insert("postgresql.service".to_string(), pg_allowed);

    // LLM suggests RESTART for postgresql.service
    let llm_action = RemediationAction::Restart;
    let verdict = policy.evaluate("postgresql.service", llm_action, Severity::High);

    assert_eq!(
        verdict,
        PolicyVerdict::DisallowedAction("Restart".to_string()),
        "LLM restart recommendation must be vetoed by unit policy"
    );
}

#[test]
fn test_tier3_critical_severity_requires_operator_confirmation() {
    let policy = PolicyEngine::default();
    // Service has critical segfault, LLM suggests RESTART
    let verdict = policy.evaluate("api.service", RemediationAction::Restart, Severity::Critical);

    assert_eq!(
        verdict,
        PolicyVerdict::RequiresConfirmation(Severity::Critical),
        "Critical severity incident must require operator confirmation before auto-restart"
    );
}

#[test]
fn test_tier3_allowed_restart_with_backoff_passes_policy_gate() {
    let policy = PolicyEngine::default();
    let verdict = policy.evaluate(
        "worker.service",
        RemediationAction::RestartWithBackoff,
        Severity::Medium,
    );

    assert_eq!(verdict, PolicyVerdict::Permitted);
}
