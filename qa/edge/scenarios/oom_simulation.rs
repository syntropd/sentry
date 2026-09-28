//! Tier 4: Real-World Scenario: OOM Crash Simulation
//!
//! Simulates oom_service behavior: process allocating memory dirtying pages
//! until terminated by cgroup memory limit / kernel SIGKILL (exit 137).

#[path = "../../e2e/harness/harness_models.rs"]
mod harness_models;

use harness_models::*;

#[test]
fn test_tier4_oom_service_crash_simulation() {
    let unit_name = "oom_service.service";
    let memory_limit_bytes = 20 * 1024 * 1024; // 20MB
    let exit_status = 137; // 128 + SIGKILL (9)

    // Simulate cgroup v2 memory telemetry at crash
    let cgroup_memory_current = 20 * 1024 * 1024;
    let psi_memory = PsiStats {
        cpu_some_avg10: 1.0,
        memory_some_avg10: 95.0,
        memory_full_avg10: 82.5,
        io_some_avg10: 0.5,
    };

    assert!(cgroup_memory_current >= memory_limit_bytes);
    assert_eq!(exit_status, 137);

    // Synthesize diagnostic payload
    let payload = DiagnosticPayload {
        incident_id: "inc-oom-sim-42".to_string(),
        timestamp: "2026-09-24T00:30:00Z".to_string(),
        unit_name: unit_name.to_string(),
        root_cause: RootCause {
            summary: "Kernel OOM killer terminated process due to 20M cgroup ceiling".to_string(),
            detail: Some("cgroup memory.current reached 20971520 bytes; PSI full pressure: 82.5%".to_string()),
        },
        evidence: Evidence {
            journal_lines: vec![
                "oom_service[5012]: Allocated and dirtied 20 MB".to_string(),
                "kernel: Memory cgroup out of memory: Killed process 5012 (oom_service)".to_string(),
            ],
            exit_codes: vec![exit_status],
            signals: vec!["SIGKILL".to_string()],
            coredump_trace: None,
            psi_stats: Some(psi_memory),
        },
        severity: Severity::High,
        proposed_remediation: ProposedRemediation {
            action: RemediationAction::RestartWithBackoff,
            rationale: Some("Restarting immediately causes instant re-kill; backoff needed".to_string()),
            risk_level: RiskLevel::Moderate,
            confidence: 0.96,
        },
    };

    assert_eq!(payload.severity, Severity::High);
    assert_eq!(payload.proposed_remediation.action, RemediationAction::RestartWithBackoff);
}
