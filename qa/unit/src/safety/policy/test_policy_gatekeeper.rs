//! Unit tests for PolicyGatekeeper zero-trust remediation evaluation.

use sentry_core::error::SafetyError;
use sentry_core::models::RemediationAction;
use sentry_safety::policy::{PolicyConfig, PolicyGatekeeper, UnitPolicyOverride};

#[test]
fn test_gatekeeper_blocks_protected_units() {
    let mut policy = PolicyConfig::default();
    policy.global.protected_units.push("systemd-journald.service".to_string());
    let gate = PolicyGatekeeper::new(policy);

    // Active modification on protected unit must fail
    let res = gate.validate("systemd-journald.service", RemediationAction::Restart);
    assert_eq!(
        res,
        Err(SafetyError::UnitBlacklisted("systemd-journald.service".to_string()))
    );

    // Safe passive actions are allowed through
    let res = gate.validate("systemd-journald.service", RemediationAction::NoAction);
    assert_eq!(res, Ok(RemediationAction::NoAction));
}

#[test]
fn test_gatekeeper_applies_unit_force_action() {
    let mut policy = PolicyConfig::default();
    policy.units.insert(
        "flaky-api.service".to_string(),
        UnitPolicyOverride {
            protected: Some(false),
            allowed_actions: None,
            force_action: Some(RemediationAction::RestartWithBackoff),
        },
    );
    let gate = PolicyGatekeeper::new(policy);

    // LLM proposes immediate Restart, policy overrides to RestartWithBackoff
    let res = gate.validate("flaky-api.service", RemediationAction::Restart);
    assert_eq!(res, Ok(RemediationAction::RestartWithBackoff));
}

#[test]
fn test_gatekeeper_rejects_unauthorized_global_action() {
    let mut policy = PolicyConfig::default();
    // Only Reload is allowed globally
    policy.global.allowed_actions = vec![RemediationAction::Reload];
    let gate = PolicyGatekeeper::new(policy);

    let res = gate.validate("custom.service", RemediationAction::Restart);
    assert!(matches!(res, Err(SafetyError::ActionDisallowed { .. })));
}
