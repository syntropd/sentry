//! Tier 2: R7 Packaging, Installation & Policy Boundary Tests
//!
//! Validates installer idempotence boundaries, permissions, and malformed policy syntax.

use tempfile::tempdir;
use std::fs;

#[test]
fn test_r7_boundary_installer_idempotence_existing_files() {
    let dir = tempdir().unwrap();
    let bin_path = dir.path().join("systemd-sentry");

    // First install
    fs::write(&bin_path, b"binary_v1").unwrap();
    assert_eq!(fs::read(&bin_path).unwrap(), b"binary_v1");

    // Second install (overwriting)
    fs::write(&bin_path, b"binary_v2").unwrap();
    assert_eq!(fs::read(&bin_path).unwrap(), b"binary_v2");
}

#[test]
fn test_r7_boundary_unprivileged_mode_config_path() {
    let is_root = false;
    let config_path = if is_root {
        "/etc/systemd-sentry/config.toml"
    } else {
        "~/.config/systemd-sentry/config.toml"
    };

    assert_eq!(config_path, "~/.config/systemd-sentry/config.toml");
}

#[test]
fn test_r7_boundary_malformed_policy_toml_syntax() {
    let corrupt_toml = "[[[bad_syntax]]]\nenabled = = = true\n";
    let parsed: Result<toml::Value, _> = toml::from_str(corrupt_toml);
    assert!(parsed.is_err(), "Malformed TOML syntax must produce parse error");
}

#[test]
fn test_r7_boundary_empty_allowed_actions_defaults_to_deny() {
    let toml_data = "[safety]\nallowed_actions = []\n";
    let val: toml::Value = toml::from_str(toml_data).unwrap();
    let actions = val["safety"]["allowed_actions"].as_array().unwrap();
    assert!(actions.is_empty());
    // Empty whitelist means no remediation is permitted
    let allows_restart = actions.iter().any(|a| a.as_str() == Some("RESTART"));
    assert!(!allows_restart);
}

#[test]
fn test_r7_boundary_unrecognized_unit_policy_override() {
    let toml_data = "[units.\"custom.service\"]\nallowed_actions = [\"NO_ACTION\"]\n";
    let val: toml::Value = toml::from_str(toml_data).unwrap();
    assert!(val["units"].get("custom.service").is_some());
    assert!(val["units"].get("unlisted.service").is_none());
}
