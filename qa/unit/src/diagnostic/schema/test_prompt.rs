//! Unit tests for DiagnosticPrompt conversion and formatting.

use sentry_core::models::{CoredumpRecord, DriverEvent, IncidentContext, UnitFailedDetails};
use sentry_diagnostic::schema::DiagnosticPrompt;

#[test]
fn test_prompt_from_incident_context_with_unit_failed() {
    let details = UnitFailedDetails {
        unit: "web.service".to_string(),
        active_state: "failed".to_string(),
        sub_state: "failed".to_string(),
        result: Some("core-dump".to_string()),
        exec_status: Some(139),
        main_pid: Some(4210),
    };

    let ctx = IncidentContext::new("web.service", DriverEvent::UnitFailed(details))
        .with_journal_lines(vec!["segfault at 0x0".to_string()]);

    let prompt = DiagnosticPrompt::from_incident_context(&ctx);
    assert_eq!(prompt.unit_name, "web.service");
    assert_eq!(prompt.exit_code, Some(139));
    assert_eq!(prompt.signal, Some("core-dump".to_string()));
    assert_eq!(prompt.journal_lines.len(), 1);
    assert_eq!(prompt.active_state, "failed");
    assert_eq!(prompt.sub_state, "failed");

    let json_str = prompt.to_prompt_json();
    assert!(json_str.contains("web.service"));
    assert!(json_str.contains("139"));
}

#[test]
fn test_prompt_with_coredump_record() {
    let mut coredump = CoredumpRecord::new("app.service", 5001, 11, "SIGSEGV");
    coredump.stack_trace = Some("#0 0x1234 in main ()".to_string());

    let ctx = IncidentContext::new("app.service", DriverEvent::Coredump(coredump.clone()))
        .with_coredump(coredump);

    let prompt = DiagnosticPrompt::from_incident_context(&ctx);
    assert_eq!(prompt.signal, Some("SIGSEGV".to_string()));
    assert_eq!(
        prompt.coredump_trace,
        Some("#0 0x1234 in main ()".to_string())
    );
}

#[test]
fn test_prompt_system_prompt_not_empty() {
    assert!(!DiagnosticPrompt::SYSTEM_PROMPT.is_empty());
    assert!(DiagnosticPrompt::SYSTEM_PROMPT.contains("systemd-sentry"));
}
