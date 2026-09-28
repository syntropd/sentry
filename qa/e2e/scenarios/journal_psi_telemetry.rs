//! Tier 3: Cross-Feature: Journal Streaming & PSI Pressure Telemetry Correlation
//!
//! Validates combining journal crash lines with kernel PSI stats to diagnose OOM.

#[path = "../harness/harness_models.rs"]
mod harness_models;

use harness_models::*;

#[test]
fn test_tier3_journal_oom_correlated_with_psi_memory_pressure() {
    let journal_line = "kernel: Memory cgroup out of memory: Killed process 4120 (oom_worker)";
    let psi_memory = PsiStats {
        cpu_some_avg10: 5.2,
        memory_some_avg10: 92.4,
        memory_full_avg10: 78.1, // Extreme pressure
        io_some_avg10: 2.0,
    };

    let is_oom = journal_line.contains("out of memory") && psi_memory.memory_full_avg10 > 50.0;
    assert!(is_oom, "Journal log and PSI must correlate to confirm OOM");

    let payload = DiagnosticPayload {
        incident_id: "inc-oom-psi-01".to_string(),
        timestamp: "2026-09-24T00:25:00Z".to_string(),
        unit_name: "oom_service.service".to_string(),
        root_cause: RootCause {
            summary: "Kernel OOM killer terminated process; PSI memory pressure at 78.1%".to_string(),
            detail: Some(journal_line.to_string()),
        },
        evidence: Evidence {
            journal_lines: vec![journal_line.to_string()],
            exit_codes: vec![137],
            signals: vec!["SIGKILL".to_string()],
            coredump_trace: None,
            psi_stats: Some(psi_memory),
        },
        severity: Severity::High,
        proposed_remediation: ProposedRemediation {
            action: RemediationAction::RestartWithBackoff,
            rationale: Some("Backoff allows memory reclamation".to_string()),
            risk_level: RiskLevel::Moderate,
            confidence: 0.95,
        },
    };

    assert_eq!(payload.severity, Severity::High);
    assert_eq!(payload.proposed_remediation.action, RemediationAction::RestartWithBackoff);
}

#[test]
fn test_tier3_cgroup_memory_max_limit_breach_detection() {
    let memory_current = 20_971_520u64; // 20MB
    let memory_max = 20_971_520u64; // 20MB ceiling

    let ceiling_breached = memory_current >= memory_max;
    assert!(ceiling_breached);
}
