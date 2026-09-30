//! Adversarial stress tests for corrupted drop-ins, non-UTF8 bytes, and fault tolerance.

use sentry_safety::policy::{load_policy_with_dropins, PolicyConfig};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_dropin_corrupt_toml_syntax_skipped() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    fs::write(&base_file, "[global]\nprotected_units = [\"base.service\"]\n")
        .expect("Failed writing base");

    // Corrupt drop-ins
    fs::write(dropin_dir.join("01-unclosed.toml"), "[global\nprotected = [")
        .expect("Failed writing corrupt 1");
    fs::write(dropin_dir.join("02-equals.toml"), "==invalid==\nfoo")
        .expect("Failed writing corrupt 2");
    fs::write(
        dropin_dir.join("03-type-mismatch.toml"),
        "[global]\nprotected_units = 12345\n",
    )
    .expect("Failed writing corrupt 3");

    // Valid drop-in
    fs::write(
        dropin_dir.join("50-valid.toml"),
        "[global]\nprotected_units = [\"survivor.service\"]\n",
    )
    .expect("Failed writing valid dropin");

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    assert!(merged.global.protected_units.contains(&"base.service".to_string()));
    assert!(merged.global.protected_units.contains(&"survivor.service".to_string()));
    assert_eq!(merged.global.protected_units.len(), 2);
}

#[test]
fn test_dropin_invalid_utf8_bytes_skipped() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    fs::write(&base_file, "[global]\nprotected_units = [\"base.service\"]\n")
        .expect("Failed writing base");

    // Malicious drop-in containing raw non-UTF8 bytes with .toml extension
    let bad_bytes: [u8; 12] = [0xFF, 0xFE, 0xFD, 0x80, 0x00, 0x81, 0x82, 0x99, 0xAA, 0xBB, 0xCC, 0xDD];
    fs::write(dropin_dir.join("15-bad-utf8.toml"), bad_bytes)
        .expect("Failed writing bad utf8 file");

    // Valid sibling drop-in
    fs::write(
        dropin_dir.join("90-valid.toml"),
        "[global]\nprotected_units = [\"utf8-survivor.service\"]\n",
    )
    .expect("Failed writing valid sibling");

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    assert!(merged.global.protected_units.contains(&"base.service".to_string()));
    assert!(merged.global.protected_units.contains(&"utf8-survivor.service".to_string()));
}

#[test]
fn test_dropin_non_toml_files_and_directories_ignored() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    // Diverse non-toml file types
    fs::write(dropin_dir.join("notes.txt"), "This is a text note").unwrap();
    fs::write(dropin_dir.join("script.sh"), "#!/bin/bash\nexit 0").unwrap();
    fs::write(dropin_dir.join("binary.bin"), [0x00, 0x01, 0x02, 0x03]).unwrap();
    fs::write(dropin_dir.join("config.yaml"), "key: value").unwrap();
    fs::write(dropin_dir.join("policy.json"), "{\"key\": \"value\"}").unwrap();

    // Subdirectory inside dropin_dir (even if named with .toml suffix)
    let nested_dir = dropin_dir.join("nested.toml");
    fs::create_dir_all(&nested_dir).expect("Failed creating nested directory");
    fs::write(nested_dir.join("10-inner.toml"), "[global]\nprotected_units = [\"inner.service\"]\n").unwrap();

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    // Non-toml files must be completely ignored; nested dropin should not be loaded by flat read_dir
    assert!(!merged.global.protected_units.contains(&"inner.service".to_string()));
}

#[test]
fn test_dropin_oversized_in_dropin_dir_gracefully_skipped() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    // Drop-in exceeding 64 KiB
    let huge_data = vec![b'#'; 70_000];
    fs::write(dropin_dir.join("10-huge.toml"), huge_data).expect("Failed writing huge drop-in");

    // Valid sibling drop-in
    fs::write(
        dropin_dir.join("20-small.toml"),
        "[global]\nprotected_units = [\"small.service\"]\n",
    )
    .expect("Failed writing small drop-in");

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);
    assert!(merged.global.protected_units.contains(&"small.service".to_string()));
}

#[test]
fn test_corrupt_base_file_falls_back_to_defaults() {
    let dir = tempdir().expect("Failed creating tempdir");
    let base_file = dir.path().join("policy.toml");
    let dropin_dir = dir.path().join("policy.d");
    fs::create_dir_all(&dropin_dir).expect("Failed creating dropin_dir");

    // Corrupted base file
    fs::write(&base_file, "INVALID [[[ TOML :::").expect("Failed writing corrupt base");

    // Valid drop-in
    fs::write(
        dropin_dir.join("10-extension.toml"),
        "[global]\nprotected_units = [\"added.service\"]\n",
    )
    .expect("Failed writing drop-in");

    let merged = load_policy_with_dropins(&base_file, &dropin_dir);

    // Base fallback must provide DEFAULT_PROTECTED_UNITS
    assert!(merged.global.protected_units.contains(&"systemd-journald.service".to_string()));
    assert!(merged.global.protected_units.contains(&"systemd-resolved.service".to_string()));
    // And drop-in adds "added.service"
    assert!(merged.global.protected_units.contains(&"added.service".to_string()));
}

#[test]
fn test_missing_base_and_missing_dropin_dir() {
    let non_existent_base = std::path::Path::new("/tmp/non_existent_sentry_base.toml");
    let non_existent_dropins = std::path::Path::new("/tmp/non_existent_sentry_dropins.d");

    let policy = load_policy_with_dropins(non_existent_base, non_existent_dropins);
    assert_eq!(policy, PolicyConfig::default());
}
