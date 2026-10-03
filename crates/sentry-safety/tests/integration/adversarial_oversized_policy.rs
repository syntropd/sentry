//! Adversarial stress tests for oversized policy files and boundary conditions.

use sentry_core::error::ConfigError;
use sentry_safety::policy::{load_policy_file, MAX_POLICY_FILE_SIZE};
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_reject_exact_oversize_boundary_65537_bytes() {
    let tmp = NamedTempFile::new().expect("Failed creating temp file");
    // Generate valid TOML base, padded with comment lines to reach exactly 65,537 bytes
    let base = "[global]\nprotected_units = [\"test.service\"]\n";
    let padding_needed = (MAX_POLICY_FILE_SIZE as usize + 1).saturating_sub(base.len());
    let mut payload = base.to_string();
    payload.push_str(&"#".repeat(padding_needed));

    assert_eq!(
        payload.len(),
        65_537,
        "Payload must be exactly 65,537 bytes (MAX + 1)"
    );

    fs::write(tmp.path(), payload.as_bytes()).expect("Failed writing oversized file");

    let result = load_policy_file(tmp.path());
    match result {
        Err(ConfigError::ParseError(msg)) => {
            assert!(
                msg.contains("exceeds security limit"),
                "Error message should mention exceeding security limit: {msg}"
            );
            assert!(
                msg.contains("65537"),
                "Error message should report exact file length: {msg}"
            );
            assert!(
                msg.contains("65536"),
                "Error message should report MAX_POLICY_FILE_SIZE: {msg}"
            );
        }
        other => panic!("Expected ConfigError::ParseError for 65,537 bytes, got: {other:?}"),
    }
}

#[test]
fn test_reject_massive_policy_file_1mb() {
    let tmp = NamedTempFile::new().expect("Failed creating temp file");
    let one_mb = vec![b'#'; 1024 * 1024];
    fs::write(tmp.path(), &one_mb).expect("Failed writing 1MB file");

    let result = load_policy_file(tmp.path());
    match result {
        Err(ConfigError::ParseError(msg)) => {
            assert!(
                msg.contains("exceeds security limit"),
                "Expected security limit error: {msg}"
            );
        }
        other => panic!("Expected ConfigError::ParseError for 1MB file, got: {other:?}"),
    }
}

#[test]
fn test_accept_exact_max_boundary_65536_bytes() {
    let tmp = NamedTempFile::new().expect("Failed creating temp file");
    let base = "[global]\nprotected_units = [\"boundary.service\"]\n";
    let padding_needed = (MAX_POLICY_FILE_SIZE as usize).saturating_sub(base.len());
    let mut payload = base.to_string();
    payload.push_str(&"#".repeat(padding_needed));

    assert_eq!(
        payload.len(),
        65_536,
        "Payload must be exactly 65,536 bytes (exact limit)"
    );

    fs::write(tmp.path(), payload.as_bytes()).expect("Failed writing exact max boundary file");

    let result = load_policy_file(tmp.path());
    assert!(
        result.is_ok(),
        "File of exactly 64 KiB (65,536 bytes) must be accepted, got: {result:?}"
    );
    let policy = result.unwrap();
    assert_eq!(
        policy.global.protected_units,
        vec!["boundary.service".to_string()]
    );
}

#[test]
fn test_empty_policy_file_0_bytes() {
    let tmp = NamedTempFile::new().expect("Failed creating temp file");
    fs::write(tmp.path(), b"").expect("Failed writing 0-byte file");

    let result = load_policy_file(tmp.path());
    assert!(
        result.is_ok(),
        "Empty 0-byte file must parse cleanly into default configuration, got: {result:?}"
    );
    let policy = result.unwrap();
    assert!(policy.global.protected_units.is_empty());
    assert!(policy.units.is_empty());
}

#[test]
fn test_non_existent_policy_file_io_error() {
    let non_existent = std::path::Path::new("/tmp/systemd_sentry_non_existent_policy_file.toml");
    let result = load_policy_file(non_existent);
    assert!(
        matches!(result, Err(ConfigError::Io(_))),
        "Non-existent policy file must return ConfigError::Io, got: {result:?}"
    );
}
