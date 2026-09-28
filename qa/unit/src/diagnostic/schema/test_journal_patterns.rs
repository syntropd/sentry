//! Unit tests for deterministic journal pattern matching.

use sentry_core::models::{RemediationAction, Severity};
use sentry_diagnostic::fallback::scan_journal_patterns;

#[test]
fn test_match_oom_journal_pattern() {
    let lines = vec![
        "Starting worker...".to_string(),
        "kernel: Out of memory: Kill process 1234 (app) score 800".to_string(),
    ];
    let res = scan_journal_patterns(&lines).expect("Matches OOM");
    assert_eq!(res.severity, Severity::High);
    assert_eq!(res.action, RemediationAction::RestartWithBackoff);
}

#[test]
fn test_match_segfault_journal_pattern() {
    let lines = vec!["kernel: segfault at 0000000000000000 ip 00007f...".to_string()];
    let res = scan_journal_patterns(&lines).expect("Matches segfault");
    assert_eq!(res.severity, Severity::Critical);
    assert_eq!(res.action, RemediationAction::EscalateToAdmin);
}

#[test]
fn test_match_port_conflict_pattern() {
    let lines = vec!["nginx: [emerg] bind() to 0.0.0.0:80 failed (Address already in use)".to_string()];
    let res = scan_journal_patterns(&lines).expect("Matches port conflict");
    assert_eq!(res.severity, Severity::High);
    assert_eq!(res.action, RemediationAction::RestartWithBackoff);
}

#[test]
fn test_match_exec_spawn_failure_pattern() {
    let lines = vec!["systemd[1]: Failed at step EXEC spawning /usr/bin/missing: No such file or directory".to_string()];
    let res = scan_journal_patterns(&lines).expect("Matches exec failure");
    assert_eq!(res.severity, Severity::Critical);
    assert_eq!(res.action, RemediationAction::NoAction);
}

#[test]
fn test_no_match_returns_none() {
    let lines = vec!["normal operational log line".to_string()];
    assert!(scan_journal_patterns(&lines).is_none());
}
