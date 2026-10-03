//! Adversarial tests for DeterministicFallbackEngine and extreme subsystem inputs.

use chrono::Utc;
use sentry_core::models::{
    CgroupTelemetry, CoredumpRecord, DriverEvent, IncidentContext, JournalEntryDetails,
    MemoryEvents, PressureTelemetry, PsiLine, PsiRecord, RemediationAction, Severity,
    UnitFailedDetails,
};
use sentry_diagnostic::fallback::DeterministicFallbackEngine;
use std::collections::HashMap;
use uuid::Uuid;

fn make_base_context(unit: &str, event: DriverEvent) -> IncidentContext {
    IncidentContext {
        incident_id: Uuid::new_v4(),
        timestamp: Utc::now(),
        unit: unit.to_string(),
        failure_event: event,
        journal_lines: Vec::new(),
        telemetry: None,
        cgroup: None,
        coredump: None,
    }
}

#[test]
fn test_fallback_with_unknown_and_extreme_exit_codes() {
    let engine = DeterministicFallbackEngine::new();
    let extreme_codes = [
        None,
        Some(0),
        Some(-1),
        Some(-9),
        Some(-139),
        Some(i32::MIN),
        Some(i32::MAX),
        Some(42),
        Some(126),
        Some(127),
        Some(255),
        Some(999999),
    ];

    for code in extreme_codes {
        let ctx = make_base_context(
            "unknown-exit.service",
            DriverEvent::UnitFailed(UnitFailedDetails {
                unit: "unknown-exit.service".to_string(),
                active_state: "failed".to_string(),
                sub_state: "failed".to_string(),
                result: None,
                exec_status: code,
                main_pid: Some(1001),
            }),
        );

        let payload = engine.triage(&ctx);
        assert_eq!(payload.unit_name, "unknown-exit.service");
        assert_eq!(payload.incident_id, ctx.incident_id);
        assert!(!payload.root_cause.summary.is_empty());
        assert!((0.0..=1.0).contains(&payload.proposed_remediation.confidence));
    }
}

#[test]
fn test_fallback_with_corrupt_and_extreme_signals() {
    let engine = DeterministicFallbackEngine::new();
    let strange_signals = [
        "",
        "\0",
        "SIG\0KILL",
        "SIG9999999",
        "SIG_UNKNOWN_ANOMALY",
        "SIG🚨CRASH",
        "     ",
        "sigkill",
        "SEGV",
    ];

    for sig in strange_signals {
        let ctx = make_base_context(
            "signal-target.service",
            DriverEvent::UnitFailed(UnitFailedDetails {
                unit: "signal-target.service".to_string(),
                active_state: "failed".to_string(),
                sub_state: "failed".to_string(),
                result: Some(sig.to_string()),
                exec_status: None,
                main_pid: Some(2002),
            }),
        );

        let payload = engine.triage(&ctx);
        assert_eq!(payload.unit_name, "signal-target.service");
        assert_eq!(payload.incident_id, ctx.incident_id);
        if sig == "sigkill" || sig == "SEGV" {
            assert!(payload.severity == Severity::High || payload.severity == Severity::Critical);
        }
    }
}

#[test]
fn test_fallback_with_extreme_journal_inputs() {
    let engine = DeterministicFallbackEngine::new();

    let ctx_empty = make_base_context(
        "empty-journal.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "empty-journal.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: None,
            main_pid: None,
        }),
    );
    let p1 = engine.triage(&ctx_empty);
    assert_eq!(p1.unit_name, "empty-journal.service");

    let mut ctx_many = ctx_empty.clone();
    ctx_many.journal_lines = vec!["".to_string(); 5000];
    let p2 = engine.triage(&ctx_many);
    assert_eq!(p2.unit_name, "empty-journal.service");

    let mut huge_line = "A".repeat(100_000);
    huge_line.push_str("\0segfault at 00000000\0");
    let mut ctx_huge = ctx_empty.clone();
    ctx_huge.journal_lines = vec![huge_line];
    let p3 = engine.triage(&ctx_huge);
    assert_eq!(p3.severity, Severity::Critical);
    assert!(p3.root_cause.summary.contains("Memory corruption"));
}

#[test]
fn test_fallback_with_extreme_cgroup_telemetry() {
    let engine = DeterministicFallbackEngine::new();

    let mut cgroup = CgroupTelemetry {
        unit: Some("cgroup-stress.service".to_string()),
        cgroup_path: "/sys/fs/cgroup/system.slice/cgroup-stress.service".to_string(),
        memory_current_bytes: Some(u64::MAX),
        memory_max_bytes: Some(0),
        memory_events: MemoryEvents {
            low: 0,
            high: 0,
            max: u64::MAX,
            oom: u64::MAX,
            oom_kill: u64::MAX,
            oom_group_kill: 0,
        },
        cpu_stat: Default::default(),
        io_stats: Vec::new(),
        populated: Some(true),
        frozen: Some(false),
        timestamp_usec: 12345678,
        synthetic: false,
    };

    let mut ctx = make_base_context(
        "cgroup-stress.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "cgroup-stress.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: None,
            main_pid: None,
        }),
    );
    ctx.cgroup = Some(cgroup.clone());

    let p = engine.triage(&ctx);
    assert_eq!(p.severity, Severity::High);
    assert_eq!(p.proposed_remediation.action, RemediationAction::RestartWithBackoff);

    cgroup.memory_current_bytes = None;
    cgroup.memory_max_bytes = None;
    cgroup.memory_events.oom = 0;
    cgroup.memory_events.oom_kill = 0;
    ctx.cgroup = Some(cgroup);

    let p_none = engine.triage(&ctx);
    assert_eq!(p_none.unit_name, "cgroup-stress.service");
}

#[test]
fn test_fallback_with_divergent_driver_events() {
    let engine = DeterministicFallbackEngine::new();

    let dummy_psi = PressureTelemetry::new(
        Some("other-event.service".to_string()),
        PsiRecord::new(PsiLine::new(1.0, 1.0, 1.0, 100), None),
        PsiRecord::new(PsiLine::new(50.0, 20.0, 10.0, 500), None),
        PsiRecord::new(PsiLine::new(0.0, 0.0, 0.0, 0), None),
        1000,
    );

    let mut coredump_rec = CoredumpRecord::new("other-event.service", 9999, 11, "SIGSEGV");
    coredump_rec.stack_trace = Some("corrupted trace #0 0x0".to_string());

    let events = [
        DriverEvent::JournalEntry(JournalEntryDetails {
            unit: Some("other-event.service".to_string()),
            message: "Random journal log".to_string(),
            priority: 3,
            timestamp_usec: 1000,
            extra: HashMap::new(),
        }),
        DriverEvent::Pressure(dummy_psi),
        DriverEvent::Coredump(coredump_rec.clone()),
    ];

    for ev in events {
        let mut ctx = make_base_context("other-event.service", ev);
        if let DriverEvent::Coredump(rec) = &ctx.failure_event {
            ctx.coredump = Some(rec.clone());
        }

        let p = engine.triage(&ctx);
        assert_eq!(p.unit_name, "other-event.service");
        assert_eq!(p.incident_id, ctx.incident_id);
    }
}
