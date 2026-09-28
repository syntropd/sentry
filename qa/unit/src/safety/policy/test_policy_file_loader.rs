//! Unit tests for bounded policy file loader.

use sentry_safety::policy::{load_policy_file, MAX_POLICY_FILE_SIZE};
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_load_valid_policy_file() {
    let tmp = NamedTempFile::new().unwrap();
    let content = r#"
[global]
protected_units = ["vault.service"]
allowed_actions = ["RELOAD"]
"#;
    fs::write(tmp.path(), content).unwrap();

    let loaded = load_policy_file(tmp.path()).unwrap();
    assert_eq!(loaded.global.protected_units, vec!["vault.service"]);
}

#[test]
fn test_reject_oversized_policy_file() {
    let tmp = NamedTempFile::new().unwrap();
    // Exceed MAX_POLICY_FILE_SIZE (64 KiB)
    let oversize = vec![b' '; (MAX_POLICY_FILE_SIZE as usize) + 10];
    fs::write(tmp.path(), oversize).unwrap();

    let res = load_policy_file(tmp.path());
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("exceeds security limit"));
}

#[test]
fn test_reject_malformed_toml_policy_file() {
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), "invalid [[ toml {{{").unwrap();

    let res = load_policy_file(tmp.path());
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Failed parsing policy TOML"));
}
