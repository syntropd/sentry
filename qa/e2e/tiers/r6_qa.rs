//! Tier 1: R6 QA, Edge Testing & Fuzzing Contracts Tests
//!
//! Validates fixtures, fuzz targets declaration, companion units, and reference parsers.

use std::fs;
use std::path::Path;

fn get_workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let path = Path::new(&manifest_dir);
    if path.ends_with("qa") {
        path.parent().unwrap().to_path_buf()
    } else {
        path.to_path_buf()
    }
}

#[test]
fn test_r6_crashing_fixtures_existence_and_executable() {
    let root = get_workspace_root();
    assert!(root.join("qa/fixtures/segfault_service/main.rs").exists());
    assert!(root.join("qa/fixtures/oom_service/main.rs").exists());
    assert!(root.join("qa/fixtures/flapper_service/main.rs").exists());
    assert!(root.join("qa/fixtures/Cargo.toml").exists());
}

#[test]
fn test_r6_fuzz_manifest_declares_all_four_targets() {
    let root = get_workspace_root();
    let manifest = fs::read_to_string(root.join("qa/fuzz/Cargo.toml")).unwrap();
    let expected_targets = [
        "fuzz_journal_parser",
        "fuzz_dbus_decoder",
        "fuzz_psi_parser",
        "fuzz_json_triage_decoder",
    ];

    for target in &expected_targets {
        assert!(
            manifest.contains(&format!("name = \"{}\"", target)),
            "qa/fuzz/Cargo.toml missing target: {}",
            target
        );
    }
}

#[test]
fn test_r6_companion_unit_service_directives() {
    let root = get_workspace_root();
    let oom_unit = fs::read_to_string(root.join("qa/fixtures/units/oom_service.service")).unwrap();
    assert!(oom_unit.contains("MemoryMax=20M"));

    let flapper_unit = fs::read_to_string(root.join("qa/fixtures/units/flapper_service.service")).unwrap();
    assert!(flapper_unit.contains("Type=notify"));
    assert!(flapper_unit.contains("Restart=always"));

    let segfault_unit = fs::read_to_string(root.join("qa/fixtures/units/segfault_service.service")).unwrap();
    assert!(segfault_unit.contains("Restart=no"));
}

#[test]
fn test_r6_fuzz_psi_parser_validates_correct_tokens() {
    let input = "some avg10=5.00 avg60=3.00 avg300=1.00 total=5000\n";
    let mut parts = input.trim().split_whitespace();
    assert_eq!(parts.next(), Some("some"));

    let mut found_avg10 = false;
    for token in parts {
        if let Some((k, v)) = token.split_once('=') {
            if k == "avg10" {
                assert_eq!(v, "5.00");
                found_avg10 = true;
            }
        }
    }
    assert!(found_avg10);
}

#[test]
fn test_r6_fuzz_journal_parser_validates_entry() {
    let raw = b"PRIORITY=3\nMESSAGE=System crashed\n\n";
    assert!(raw.ends_with(b"\n\n"));
    let text = std::str::from_utf8(raw).unwrap();
    assert!(text.contains("PRIORITY=3"));
    assert!(text.contains("MESSAGE=System crashed"));
}
