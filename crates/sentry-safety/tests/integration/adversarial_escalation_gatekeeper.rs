//! Adversarial tests for zero-trust gatekeeper, privilege escalation, and protected units.

use sentry_core::error::SafetyError;
use sentry_core::models::RemediationAction;
use sentry_safety::policy::{
    PolicyConfig, PolicyGatekeeper, UnitPolicyOverride, DEFAULT_PROTECTED_UNITS,
};

const ALL_ACTIONS: [RemediationAction; 6] = [
    RemediationAction::NoAction,
    RemediationAction::Restart,
    RemediationAction::RestartWithBackoff,
    RemediationAction::Reload,
    RemediationAction::ResetFailed,
    RemediationAction::EscalateToAdmin,
];

#[test]
fn test_adversarial_all_actions_against_all_default_protected_units() {
    let policy = PolicyConfig::default();
    let gate = PolicyGatekeeper::new(policy);

    assert_eq!(
        DEFAULT_PROTECTED_UNITS.len(),
        9,
        "DEFAULT_PROTECTED_UNITS must contain exactly 9 units"
    );

    let mut tests_run = 0;
    for &unit in DEFAULT_PROTECTED_UNITS {
        for action in ALL_ACTIONS {
            let result = gate.validate(unit, action);
            tests_run += 1;

            if action.is_active_modification() {
                assert_eq!(
                    result,
                    Err(SafetyError::UnitBlacklisted(unit.to_string())),
                    "Active mutation {action} on protected unit '{unit}' MUST be rejected"
                );
            } else {
                assert_eq!(
                    result,
                    Ok(action),
                    "Passive action {action} on protected unit '{unit}' should be permitted"
                );
            }
        }
    }

    assert_eq!(
        tests_run, 54,
        "Total 54 unit-action combinations must be tested and verified"
    );
}

#[test]
fn test_adversarial_per_unit_override_cannot_unprotect_global_units() {
    let mut policy = PolicyConfig::default();

    // Adversary attempts to override protected status of systemd-resolved and dbus
    for &unit in &["systemd-resolved.service", "dbus.service"] {
        policy.units.insert(
            unit.to_string(),
            UnitPolicyOverride {
                protected: Some(false),
                allowed_actions: Some(vec![RemediationAction::Restart]),
                force_action: Some(RemediationAction::Restart),
            },
        );
    }

    let gate = PolicyGatekeeper::new(policy);

    for &unit in &["systemd-resolved.service", "dbus.service"] {
        let result = gate.validate(unit, RemediationAction::Restart);
        assert_eq!(
            result,
            Err(SafetyError::UnitBlacklisted(unit.to_string())),
            "Protected unit '{unit}' MUST NOT be unprotectable via unit overrides"
        );
    }
}

#[test]
fn test_adversarial_action_escalation_blocked_by_global_policy() {
    let mut policy = PolicyConfig::default();
    // Highly restricted policy allowing ONLY reload
    policy.global.allowed_actions = vec![RemediationAction::Reload];
    let gate = PolicyGatekeeper::new(policy);

    let unit = "my-custom-service.service";

    // Propose disallowed active actions
    for forbidden in [
        RemediationAction::Restart,
        RemediationAction::RestartWithBackoff,
        RemediationAction::ResetFailed,
    ] {
        let result = gate.validate(unit, forbidden);
        assert!(
            matches!(result, Err(SafetyError::ActionDisallowed { .. })),
            "Action {forbidden} must be disallowed when not in allowed_actions list"
        );
    }

    // Propose explicitly allowed active action
    assert_eq!(
        gate.validate(unit, RemediationAction::Reload),
        Ok(RemediationAction::Reload)
    );

    // Passive actions must still pass
    assert_eq!(
        gate.validate(unit, RemediationAction::NoAction),
        Ok(RemediationAction::NoAction)
    );
    assert_eq!(
        gate.validate(unit, RemediationAction::EscalateToAdmin),
        Ok(RemediationAction::EscalateToAdmin)
    );
}

#[test]
fn test_adversarial_unit_override_protection_blocks_mutations() {
    let mut policy = PolicyConfig::default();
    let custom_critical = "custom-database.service";

    // Explicitly protect non-system unit via per-unit override
    policy.units.insert(
        custom_critical.to_string(),
        UnitPolicyOverride {
            protected: Some(true),
            allowed_actions: Some(vec![RemediationAction::Restart]),
            force_action: Some(RemediationAction::Restart),
        },
    );

    let gate = PolicyGatekeeper::new(policy);

    for action in [
        RemediationAction::Restart,
        RemediationAction::RestartWithBackoff,
        RemediationAction::Reload,
        RemediationAction::ResetFailed,
    ] {
        let result = gate.validate(custom_critical, action);
        assert_eq!(
            result,
            Err(SafetyError::UnitBlacklisted(custom_critical.to_string())),
            "Explicitly protected unit '{custom_critical}' must reject active action {action}"
        );
    }
}
