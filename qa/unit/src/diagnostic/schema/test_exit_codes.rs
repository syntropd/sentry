//! Unit tests for deterministic exit code triage.

use sentry_core::models::{RemediationAction, RiskLevel, Severity};
use sentry_diagnostic::fallback::triage_exit_code;

#[test]
fn test_triage_exit_137_oom() {
    let result = triage_exit_code(Some(137), None).expect("Matches OOM");
    assert_eq!(result.severity, Severity::High);
    assert_eq!(result.action, RemediationAction::RestartWithBackoff);
    assert_eq!(result.risk_level, RiskLevel::Medium);
    assert!(result.root_cause.summary.contains("Out-Of-Memory"));
}

#[test]
fn test_triage_sigkill() {
    let result = triage_exit_code(None, Some("SIGKILL")).expect("Matches SIGKILL");
    assert_eq!(result.severity, Severity::High);
    assert_eq!(result.action, RemediationAction::RestartWithBackoff);
}

#[test]
fn test_triage_exit_139_segfault() {
    let result = triage_exit_code(Some(139), None).expect("Matches segfault");
    assert_eq!(result.severity, Severity::Critical);
    assert_eq!(result.action, RemediationAction::EscalateToAdmin);
    assert_eq!(result.risk_level, RiskLevel::High);
    assert!(result.root_cause.summary.contains("Segmentation Fault"));
}

#[test]
fn test_triage_exit_134_sigabrt() {
    let result = triage_exit_code(Some(134), None).expect("Matches SIGABRT");
    assert_eq!(result.severity, Severity::Critical);
    assert_eq!(result.action, RemediationAction::EscalateToAdmin);
}

#[test]
fn test_triage_exit_203_exec() {
    let result = triage_exit_code(Some(203), None).expect("Matches EXIT_EXEC");
    assert_eq!(result.severity, Severity::Critical);
    assert_eq!(result.action, RemediationAction::NoAction);
    assert_eq!(result.risk_level, RiskLevel::High);
}

#[test]
fn test_triage_unknown_exit_code_returns_none() {
    assert!(triage_exit_code(Some(42), None).is_none());
}

#[test]
fn test_triage_raw_signal_numbers() {
    // Raw signal 11 (SIGSEGV as reported by systemd ExecMainStatus)
    let segv = triage_exit_code(Some(11), None).expect("Matches raw SIGSEGV");
    assert_eq!(segv.severity, Severity::Critical);
    assert_eq!(segv.action, RemediationAction::EscalateToAdmin);

    // Raw signal 6 (SIGABRT)
    let abrt = triage_exit_code(Some(6), None).expect("Matches raw SIGABRT");
    assert_eq!(abrt.severity, Severity::Critical);
    assert_eq!(abrt.action, RemediationAction::EscalateToAdmin);

    // Raw signal 9 (SIGKILL)
    let kill = triage_exit_code(Some(9), None).expect("Matches raw SIGKILL");
    assert_eq!(kill.severity, Severity::High);
    assert_eq!(kill.action, RemediationAction::RestartWithBackoff);

    // Raw signal 4 (SIGILL)
    let ill = triage_exit_code(Some(4), None).expect("Matches raw SIGILL");
    assert_eq!(ill.severity, Severity::Critical);

    // Raw signal 7 (SIGBUS)
    let bus = triage_exit_code(Some(7), None).expect("Matches raw SIGBUS");
    assert_eq!(bus.severity, Severity::Critical);

    // Raw signal 8 (SIGFPE)
    let fpe = triage_exit_code(Some(8), None).expect("Matches raw SIGFPE");
    assert_eq!(fpe.severity, Severity::Critical);
}

#[test]
fn test_triage_systemd_result_values() {
    // Watchdog
    let wd = triage_exit_code(None, Some("watchdog")).expect("Matches watchdog");
    assert_eq!(wd.severity, Severity::High);
    assert_eq!(wd.action, RemediationAction::RestartWithBackoff);
    assert!(wd.root_cause.summary.contains("watchdog"));

    // Start limit hit
    let limit = triage_exit_code(None, Some("start-limit-hit")).expect("Matches start-limit-hit");
    assert_eq!(limit.severity, Severity::Critical);
    assert_eq!(limit.action, RemediationAction::EscalateToAdmin);

    // Timeout
    let to = triage_exit_code(None, Some("timeout")).expect("Matches timeout");
    assert_eq!(to.severity, Severity::High);
    assert_eq!(to.action, RemediationAction::RestartWithBackoff);
}
