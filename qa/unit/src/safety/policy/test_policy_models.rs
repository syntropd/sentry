//! Unit tests for Policy data models and default configurations.

use sentry_core::models::RemediationAction;
use sentry_safety::policy::{DEFAULT_PROTECTED_UNITS, PolicyConfig};

#[test]
fn test_default_policy_contains_critical_units() {
    let policy = PolicyConfig::default();
    for &unit in DEFAULT_PROTECTED_UNITS {
        assert!(
            policy.global.protected_units.contains(&unit.to_string()),
            "Expected default protected unit {unit}"
        );
    }

    assert!(
        policy
            .global
            .protected_units
            .contains(&"systemd-resolved.service".to_string()),
        "Expected systemd-resolved.service in default protected units"
    );

    assert!(policy.global.allowed_actions.contains(&RemediationAction::RestartWithBackoff));
    assert!(policy.global.allowed_actions.contains(&RemediationAction::Reload));
    assert!(policy.global.allowed_actions.contains(&RemediationAction::ResetFailed));
}

#[test]
fn test_policy_config_toml_roundtrip() {
    let toml_str = r#"
[global]
protected_units = ["custom-db.service"]
allowed_actions = ["RESTART", "RELOAD"]
cooldown_seconds = 120

[units."web.service"]
protected = false
allowed_actions = ["RESTART"]
force_action = "RESTART"
"#;

    let parsed: PolicyConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(parsed.global.protected_units, vec!["custom-db.service"]);
    assert_eq!(parsed.global.cooldown_seconds, 120);

    let unit = parsed.units.get("web.service").unwrap();
    assert_eq!(unit.protected, Some(false));
    assert_eq!(unit.force_action, Some(RemediationAction::Restart));
}
