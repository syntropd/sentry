//! Unit tests for DeterministicFallbackEngine.

use sentry_core::models::{
    CgroupTelemetry, DriverEvent, IncidentContext, MemoryEvents, RemediationAction,
    Severity, UnitFailedDetails,
};
use sentry_diagnostic::fallback::DeterministicFallbackEngine;

#[test]
fn test_fallback_engine_with_journal_oom() {
    let engine = DeterministicFallbackEngine::new();
    let ctx = IncidentContext::new(
        "api.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "api.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: None,
            main_pid: None,
        }),
    )
    .with_journal_lines(vec!["kernel: Out of memory: Kill process".to_string()]);

    let payload = engine.triage(&ctx);
    assert_eq!(payload.unit_name, "api.service");
    assert_eq!(payload.severity, Severity::High);
    assert_eq!(
        payload.proposed_remediation.action,
        RemediationAction::RestartWithBackoff
    );
}

#[test]
fn test_fallback_engine_with_segfault_exit_code() {
    let engine = DeterministicFallbackEngine::new();
    let ctx = IncidentContext::new(
        "crash.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "crash.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: Some(139),
            main_pid: None,
        }),
    );

    let payload = engine.triage(&ctx);
    assert_eq!(payload.severity, Severity::Critical);
    assert_eq!(
        payload.proposed_remediation.action,
        RemediationAction::EscalateToAdmin
    );
}

#[test]
fn test_fallback_engine_with_cgroup_memory_limit() {
    let engine = DeterministicFallbackEngine::new();
    let cgroup = CgroupTelemetry {
        unit: Some("worker.service".to_string()),
        cgroup_path: "/system.slice/worker.service".to_string(),
        memory_current_bytes: Some(1000),
        memory_max_bytes: Some(1000),
        memory_events: MemoryEvents::default(),
        cpu_stat: Default::default(),
        io_stats: vec![],
        populated: Some(true),
        frozen: Some(false),
        timestamp_usec: 0,
        synthetic: false,
    };

    let ctx = IncidentContext::new(
        "worker.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "worker.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: None,
            main_pid: None,
        }),
    )
    .with_cgroup(cgroup);

    let payload = engine.triage(&ctx);
    assert_eq!(payload.severity, Severity::High);
    assert!(payload.root_cause.summary.contains("Cgroup memory limit"));
}

#[test]
fn test_fallback_engine_generic_fallback() {
    let engine = DeterministicFallbackEngine::new();
    let ctx = IncidentContext::new(
        "unknown.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "unknown.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: None,
            main_pid: None,
        }),
    );

    let payload = engine.triage(&ctx);
    assert_eq!(payload.severity, Severity::Medium);
    assert_eq!(payload.proposed_remediation.confidence, 0.30);
}
