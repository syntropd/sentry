//! Unit tests for `explain_incident` tool execution.

use sentry_core::models::{DiagnosticPayload, RemediationAction, RiskLevel, RootCause, Severity};
use sentry_mcp::storage::McpState;
use sentry_mcp::tools::execute_explain_incident;
use serde_json::json;
use uuid::Uuid;

fn make_incident_with_coredump() -> DiagnosticPayload {
    let mut payload = DiagnosticPayload {
        incident_id: Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        unit_name: "segfault.service".to_string(),
        root_cause: RootCause {
            summary: "Segfault in worker thread".to_string(),
            detail: "Null dereference at address 0x0".to_string(),
        },
        evidence: Default::default(),
        severity: Severity::Critical,
        proposed_remediation: sentry_core::models::ProposedRemediation {
            action: RemediationAction::EscalateToAdmin,
            rationale: "Requires source fix".to_string(),
            risk_level: RiskLevel::High,
            confidence: 0.98,
        },
    };
    payload.evidence.coredump = Some("#0 0x0 in crash () from /bin/app".to_string());
    payload
}

#[test]
fn test_explain_incident_root_cause() {
    let state = McpState::new();
    let inc = make_incident_with_coredump();
    let id_str = inc.incident_id.to_string();
    state.record_incident(inc);

    let args = json!({ "incident_id": id_str, "focus": "root_cause" });
    let res = execute_explain_incident(&state, Some(&args));

    assert_eq!(res["isError"], false);
    let text = res["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Segfault in worker thread"));
}

#[test]
fn test_explain_incident_coredump_trace() {
    let state = McpState::new();
    let inc = make_incident_with_coredump();
    let id_str = inc.incident_id.to_string();
    state.record_incident(inc);

    let args = json!({ "incident_id": id_str, "focus": "coredump_trace" });
    let res = execute_explain_incident(&state, Some(&args));

    assert_eq!(res["isError"], false);
    let text = res["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("#0 0x0 in crash"));
}

#[test]
fn test_explain_incident_safety_validation() {
    let state = McpState::new();
    let inc = make_incident_with_coredump();
    let id_str = inc.incident_id.to_string();
    state.record_incident(inc);

    let args = json!({ "incident_id": id_str, "focus": "safety_validation" });
    let res = execute_explain_incident(&state, Some(&args));

    assert_eq!(res["isError"], false);
    let text = res["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("EscalateToAdmin"));
}
