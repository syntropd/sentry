//! Unit tests for `get_incident` tool execution.

use sentry_core::models::{DiagnosticPayload, RemediationAction, RiskLevel, RootCause, Severity};
use sentry_mcp::storage::McpState;
use sentry_mcp::tools::execute_get_incident;
use serde_json::json;
use uuid::Uuid;

fn sample_incident() -> DiagnosticPayload {
    DiagnosticPayload {
        incident_id: Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        unit_name: "nginx.service".to_string(),
        root_cause: RootCause {
            summary: "Port collision".to_string(),
            detail: "Port 80 busy".to_string(),
        },
        evidence: Default::default(),
        severity: Severity::High,
        proposed_remediation: sentry_core::models::ProposedRemediation {
            action: RemediationAction::RestartWithBackoff,
            rationale: "Retry after cooldown".to_string(),
            risk_level: RiskLevel::Medium,
            confidence: 0.90,
        },
    }
}

#[test]
fn test_get_incident_found() {
    let state = McpState::new();
    let inc = sample_incident();
    let id_str = inc.incident_id.to_string();
    state.record_incident(inc);

    let args = json!({ "incident_id": id_str });
    let result = execute_get_incident(&state, Some(&args));

    assert_eq!(result["isError"], false);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains(&id_str));
    assert!(text.contains("nginx.service"));
}

#[test]
fn test_get_incident_not_found() {
    let state = McpState::new();
    let args = json!({ "incident_id": "00000000-0000-0000-0000-000000000000" });
    let result = execute_get_incident(&state, Some(&args));

    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Incident not found"));
}

#[test]
fn test_get_incident_missing_argument() {
    let state = McpState::new();
    let result = execute_get_incident(&state, None);

    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Missing required argument"));
}
