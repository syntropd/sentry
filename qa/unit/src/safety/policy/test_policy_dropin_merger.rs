//! Unit tests for systemd drop-in policy merger.

use sentry_core::models::RemediationAction;
use sentry_safety::policy::load_policy_with_dropins;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_dropin_merging_alphanumeric_order() {
    let dir = tempdir().unwrap();
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir(&dropin_dir).unwrap();

    fs::write(
        &base_file,
        r#"
[global]
protected_units = ["base.service"]
allowed_actions = ["RELOAD"]
"#,
    )
    .unwrap();

    // 10-first.toml
    fs::write(
        dropin_dir.join("10-first.toml"),
        r#"
[global]
protected_units = ["first.service"]

[units."app.service"]
protected = true
"#,
    )
    .unwrap();

    // 20-override.toml (overrides app.service protected to false)
    fs::write(
        dropin_dir.join("20-override.toml"),
        r#"
[units."app.service"]
protected = false
force_action = "RESTART"
"#,
    )
    .unwrap();

    // Ignored files (dotfile, swp, bak)
    fs::write(dropin_dir.join(".hidden.toml"), "invalid toml").unwrap();
    fs::write(dropin_dir.join("app.toml.swp"), "invalid binary").unwrap();
    fs::write(dropin_dir.join("notes.txt"), "some text").unwrap();

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    // Both protected units should be present
    assert!(merged.global.protected_units.contains(&"base.service".to_string()));
    assert!(merged.global.protected_units.contains(&"first.service".to_string()));

    // 20-override should have overwritten app.service
    let app = merged.units.get("app.service").unwrap();
    assert_eq!(app.protected, Some(false));
    assert_eq!(app.force_action, Some(RemediationAction::Restart));
}

#[test]
fn test_dropin_fault_tolerance_skips_invalid_files() {
    let dir = tempdir().unwrap();
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir(&dropin_dir).unwrap();

    // Malformed drop-in
    fs::write(dropin_dir.join("05-bad.toml"), "invalid [ syntax").unwrap();

    // Valid drop-in
    fs::write(
        dropin_dir.join("15-good.toml"),
        r#"
[global]
protected_units = ["good.service"]
"#,
    )
    .unwrap();

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);
    assert!(merged.global.protected_units.contains(&"good.service".to_string()));
}
