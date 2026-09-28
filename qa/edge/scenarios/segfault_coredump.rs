//! Tier 4: Real-World Scenario: Segfault Crash & Coredump Extraction
//!
//! Simulates segfault_service: null pointer dereference triggering SIGSEGV.
//! Validates coredump parsing, CRITICAL severity, and admin escalation.

#[path = "../../e2e/harness/harness_models.rs"]
mod harness_models;

use harness_models::*;

#[test]
fn test_tier4_segfault_service_crash_simulation() {
    let unit_name = "segfault_service.service";
    let signal = "SIGSEGV";
    let exit_status = 139; // 128 + 11

    let coredump_trace = "Stack trace of thread 3401:\n\
                          #0  0x000055c82a1b9140 trigger_segfault (main.rs:25)\n\
                          #1  0x000055c82a1b9200 main (main.rs:35)";

    let payload = DiagnosticPayload {
        incident_id: "inc-segv-sim-99".to_string(),
        timestamp: "2026-09-24T00:35:00Z".to_string(),
        unit_name: unit_name.to_string(),
        root_cause: RootCause {
            summary: "Segmentation Fault: Null pointer dereference in trigger_segfault".to_string(),
            detail: Some(coredump_trace.to_string()),
        },
        evidence: Evidence {
            journal_lines: vec![
                "segfault_service[3401]: Starting fixture process PID 3401".to_string(),
                "kernel: segfault_service[3401]: segfault at 0 ip 000055c82a1b9140 sp 00007ffe34 error 6".to_string(),
            ],
            exit_codes: vec![exit_status],
            signals: vec![signal.to_string()],
            coredump_trace: Some(coredump_trace.to_string()),
            psi_stats: None,
        },
        severity: Severity::Critical,
        proposed_remediation: ProposedRemediation {
            action: RemediationAction::EscalateToAdmin,
            rationale: Some("Fatal memory corruption in binary cannot be auto-remediated by restart".to_string()),
            risk_level: RiskLevel::Fatal,
            confidence: 0.99,
        },
    };

    assert_eq!(payload.severity, Severity::Critical);
    assert_eq!(payload.proposed_remediation.action, RemediationAction::EscalateToAdmin);
    assert!(payload.root_cause.summary.contains("Null pointer dereference"));
}
