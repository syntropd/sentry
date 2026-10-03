//! Adversarial tests for drop-in filename filtering and alphanumeric ordering.

use sentry_core::models::RemediationAction;
use sentry_safety::policy::load_policy_with_dropins;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_dropin_hidden_files_ignored() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    fs::write(&base_file, "[global]\nprotected_units = [\"base.service\"]\n").unwrap();

    // Various hidden file patterns
    fs::write(dropin_dir.join(".override.toml"), "[global]\nprotected_units = [\"hidden1.service\"]\n").unwrap();
    fs::write(dropin_dir.join(".hidden.toml"), "[global]\nprotected_units = [\"hidden2.service\"]\n").unwrap();
    fs::write(dropin_dir.join(".10-base.toml"), "[global]\nprotected_units = [\"hidden3.service\"]\n").unwrap();
    fs::write(dropin_dir.join(".toml"), "[global]\nprotected_units = [\"hidden4.service\"]\n").unwrap();

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    assert_eq!(merged.global.protected_units, vec!["base.service".to_string()]);
}

#[test]
fn test_dropin_editor_backup_and_swap_files_ignored() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    fs::write(&base_file, "[global]\nprotected_units = [\"base.service\"]\n").unwrap();

    // Editor backup, swap, and temporary file patterns
    fs::write(dropin_dir.join("file.toml~"), "[global]\nprotected_units = [\"bak1.service\"]\n").unwrap();
    fs::write(dropin_dir.join("10-base.toml~"), "[global]\nprotected_units = [\"bak2.service\"]\n").unwrap();
    fs::write(dropin_dir.join("file.toml.swp"), "[global]\nprotected_units = [\"swp1.service\"]\n").unwrap();
    fs::write(dropin_dir.join("20-custom.toml.swp"), "[global]\nprotected_units = [\"swp2.service\"]\n").unwrap();
    fs::write(dropin_dir.join("file.toml.bak"), "[global]\nprotected_units = [\"bak3.service\"]\n").unwrap();
    fs::write(dropin_dir.join("file.toml.old"), "[global]\nprotected_units = [\"old1.service\"]\n").unwrap();
    fs::write(dropin_dir.join("#10-base.toml#"), "[global]\nprotected_units = [\"emacs1.service\"]\n").unwrap();
    fs::write(dropin_dir.join("file.toml#"), "[global]\nprotected_units = [\"emacs2.service\"]\n").unwrap();

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    assert_eq!(merged.global.protected_units, vec!["base.service".to_string()]);
}

#[test]
fn test_dropin_exact_alphanumeric_order_precedence() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    fs::write(&base_file, "[global]\nprotected_units = []\n").unwrap();

    // Write drop-ins in intentionally reversed chronological order on filesystem
    fs::write(
        dropin_dir.join("90-final.toml"),
        r#"
[units."worker.service"]
force_action = "NO_ACTION"
"#,
    )
    .unwrap();

    fs::write(
        dropin_dir.join("25-gamma.toml"),
        r#"
[units."worker.service"]
force_action = "RESET_FAILED"
"#,
    )
    .unwrap();

    fs::write(
        dropin_dir.join("15-beta.toml"),
        r#"
[units."worker.service"]
force_action = "RESTART_WITH_BACKOFF"
"#,
    )
    .unwrap();

    fs::write(
        dropin_dir.join("05-alpha.toml"),
        r#"
[units."worker.service"]
force_action = "RELOAD"
"#,
    )
    .unwrap();

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    // Alphanumeric sorting (05 -> 15 -> 25 -> 90) must ensure 90-final wins
    let worker_policy = merged
        .units
        .get("worker.service")
        .expect("worker.service should exist");
    assert_eq!(
        worker_policy.force_action,
        Some(RemediationAction::NoAction),
        "Later alphanumeric file (90-final) must override earlier files"
    );
}

#[test]
fn test_dropin_protected_units_cumulative_union() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    fs::write(&base_file, "[global]\nprotected_units = [\"u0.service\"]\n").unwrap();

    fs::write(dropin_dir.join("10-p1.toml"), "[global]\nprotected_units = [\"u1.service\"]\n").unwrap();
    fs::write(dropin_dir.join("20-p2.toml"), "[global]\nprotected_units = [\"u2.service\"]\n").unwrap();
    fs::write(dropin_dir.join("30-p3.toml"), "[global]\nprotected_units = [\"u3.service\"]\n").unwrap();
    // 40 introduces u1 again and u4
    fs::write(dropin_dir.join("40-dup.toml"), "[global]\nprotected_units = [\"u1.service\", \"u4.service\"]\n").unwrap();

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    assert_eq!(
        merged.global.protected_units,
        vec![
            "u0.service".to_string(),
            "u1.service".to_string(),
            "u2.service".to_string(),
            "u3.service".to_string(),
            "u4.service".to_string(),
        ]
    );
}

#[test]
fn test_dropin_cannot_unprotect_base_protected_units() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    fs::write(&base_file, "[global]\nprotected_units = [\"critical.service\"]\n").unwrap();

    // Adversarial drop-in trying to disable protection for critical.service
    fs::write(
        dropin_dir.join("99-malicious.toml"),
        r#"
[units."critical.service"]
protected = false
force_action = "RESTART"
"#,
    )
    .unwrap();

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    // Global protected units list must still contain critical.service
    assert!(merged.global.protected_units.contains(&"critical.service".to_string()));
}
