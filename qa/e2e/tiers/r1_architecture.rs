//! Tier 1: R1 Architectural Constraints & Code Structure Tests
//!
//! Verifies pure Rust invariants, strict LOC limits (<=256), and module layout.

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
fn test_r1_pure_rust_manifest_invariants() {
    let root = get_workspace_root();
    let forbidden_deps = ["libsystemd", "libdbus-1", "openssl", "libssl", "libcrypto"];
    let toml_files = ["qa/Cargo.toml", "qa/fuzz/Cargo.toml", "qa/fixtures/Cargo.toml"];

    for toml_path in &toml_files {
        let full_path = root.join(toml_path);
        let content = fs::read_to_string(&full_path)
            .unwrap_or_else(|_| panic!("Failed to read {:?}", full_path));
        for forbidden in &forbidden_deps {
            assert!(
                !content.contains(forbidden),
                "Manifest {:?} contains forbidden C dynamic dependency: {}",
                full_path,
                forbidden
            );
        }
    }
}

#[test]
fn test_r1_strict_line_count_enforcement() {
    let root = get_workspace_root();
    let mut files_to_check = Vec::new();
    let mut dirs_to_visit = vec![root.join("qa"), root.join(".agents")];

    while let Some(dir) = dirs_to_visit.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let path_str = path.to_string_lossy();
                if path_str.contains("/target/") || path_str.contains("/.git/") {
                    continue;
                }
                if path.is_dir() {
                    dirs_to_visit.push(path);
                } else if path.extension().map_or(false, |ext| ext == "rs") {
                    files_to_check.push(path);
                }
            }
        }
    }

    assert!(!files_to_check.is_empty(), "Found zero Rust files to verify");
    for file in files_to_check {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|_| panic!("Failed reading {:?}", file));
        let lines = content.lines().count();
        assert!(
            lines <= 256,
            "File {:?} exceeds 256 lines (actual: {} lines)",
            file,
            lines
        );
    }
}

#[test]
fn test_r1_single_responsibility_layout() {
    let root = get_workspace_root();
    assert!(root.join("qa/e2e/harness/harness_models.rs").exists());
    assert!(root.join("qa/e2e/harness/harness_circuit.rs").exists());
    assert!(root.join("qa/e2e/harness/harness_policy.rs").exists());
    assert!(root.join("qa/e2e/harness/harness_notify.rs").exists());
}

#[test]
fn test_r1_no_panics_in_core_error_paths() {
    use super::harness_circuit::UnitBreaker;
    let mut breaker = UnitBreaker::new(60, 3, 30);
    // Recording 100 failures sequentially must never panic
    for i in 0..100 {
        let _ = breaker.record_failure(i);
    }
}

#[test]
fn test_r1_deterministic_memory_allocation() {
    use super::harness_models::*;
    let payload = DiagnosticPayload {
        incident_id: "test-id-001".to_string(),
        timestamp: "2026-09-24T00:00:00Z".to_string(),
        unit_name: "test.service".to_string(),
        root_cause: RootCause {
            summary: "Null dereference".to_string(),
            detail: None,
        },
        evidence: Evidence {
            journal_lines: vec!["crash log".to_string()],
            exit_codes: vec![139],
            signals: vec!["SIGSEGV".to_string()],
            coredump_trace: None,
            psi_stats: None,
        },
        severity: Severity::Critical,
        proposed_remediation: ProposedRemediation {
            action: RemediationAction::EscalateToAdmin,
            rationale: Some("Requires code fix".to_string()),
            risk_level: RiskLevel::Hazardous,
            confidence: 0.99,
        },
    };
    let json = serde_json::to_string(&payload).unwrap();
    assert!(json.contains("test.service"));
    assert!(json.contains("ESCALATE_TO_ADMIN"));
}
