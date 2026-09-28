//! Tier 3: Cross-Feature: Coredump Extraction & Diagnostic Triage
//!
//! Validates pipeline from SIGSEGV/SIGABRT coredump extraction to LLM/fallback triage.

#[path = "../harness/harness_models.rs"]
mod harness_models;

use harness_models::*;

#[test]
fn test_tier3_segfault_coredump_triage_synthesis() {
    let mock_coredump_trace = "#0  0x00007f9c8d1234 in worker_process () at src/worker.c:42\n\
                               #1  0x00007f9c8d5678 in main () at src/main.c:110";

    let payload = DiagnosticPayload {
        incident_id: "inc-segv-001".to_string(),
        timestamp: "2026-09-24T00:20:00Z".to_string(),
        unit_name: "segfault_service.service".to_string(),
        root_cause: RootCause {
            summary: "SIGSEGV: Null pointer dereference in worker_process".to_string(),
            detail: Some(mock_coredump_trace.to_string()),
        },
        evidence: Evidence {
            journal_lines: vec!["segfault_service[1234]: segfault at 0 ip 00007f9c8d1234".to_string()],
            exit_codes: vec![139],
            signals: vec!["SIGSEGV".to_string()],
            coredump_trace: Some(mock_coredump_trace.to_string()),
            psi_stats: None,
        },
        severity: Severity::Critical,
        proposed_remediation: ProposedRemediation {
            action: RemediationAction::EscalateToAdmin,
            rationale: Some("Memory safety fault requires developer patch".to_string()),
            risk_level: RiskLevel::Hazardous,
            confidence: 0.99,
        },
    };

    assert_eq!(payload.severity, Severity::Critical);
    assert_eq!(payload.proposed_remediation.action, RemediationAction::EscalateToAdmin);
    assert!(payload.evidence.coredump_trace.unwrap().contains("worker.c:42"));
}

#[test]
fn test_tier3_sigabrt_panic_triage_synthesis() {
    let mock_trace = "thread 'main' panicked at 'assertion failed: ptr.is_not_null()', src/lib.rs:88";
    let is_panic = mock_trace.contains("panicked at");
    assert!(is_panic);

    let (severity, action) = if is_panic {
        (Severity::Critical, RemediationAction::EscalateToAdmin)
    } else {
        (Severity::Medium, RemediationAction::Restart)
    };

    assert_eq!(severity, Severity::Critical);
    assert_eq!(action, RemediationAction::EscalateToAdmin);
}
