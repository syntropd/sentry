//! Unit tests for `list_incidents` tool execution.

use sentry_core::models::{DiagnosticPayload, RemediationAction, RiskLevel, RootCause, Severity};
use sentry_mcp::storage::mcp_state::MAX_STORED_INCIDENTS;
use sentry_mcp::storage::McpState;
use sentry_mcp::tools::execute_list_incidents;
use serde_json::json;
use uuid::Uuid;

fn make_incident(unit: &str, severity: Severity) -> DiagnosticPayload {
    DiagnosticPayload {
        incident_id: Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        unit_name: unit.to_string(),
        root_cause: RootCause {
            summary: format!("Fault in {unit}"),
            detail: "Fault details".to_string(),
        },
        evidence: Default::default(),
        severity,
        proposed_remediation: sentry_core::models::ProposedRemediation {
            action: RemediationAction::Restart,
            rationale: "Restarting".to_string(),
            risk_level: RiskLevel::Low,
            confidence: 0.95,
        },
    }
}

#[test]
fn test_list_incidents_empty() {
    let state = McpState::new();
    let result = execute_list_incidents(&state, None);
    assert_eq!(result["isError"], false);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert_eq!(text, "[]");
}

#[test]
fn test_list_incidents_filtering() {
    let state = McpState::new();
    state.record_incident(make_incident("redis.service", Severity::High));
    state.record_incident(make_incident("web.service", Severity::Low));
    state.record_incident(make_incident("web.service", Severity::Critical));

    // Filter by unit
    let args = json!({ "unit_name": "web.service" });
    let res = execute_list_incidents(&state, Some(&args));
    let text = res["content"][0]["text"].as_str().unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert_eq!(items.len(), 2);

    // Filter by severity
    let args = json!({ "severity": "HIGH" });
    let res = execute_list_incidents(&state, Some(&args));
    let text = res["content"][0]["text"].as_str().unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["unit_name"], "redis.service");
}

#[test]
fn test_bounded_mcp_state_eviction_on_storm() {
    let state = McpState::new();
    for i in 0..105 {
        state.record_incident(make_incident(&format!("storm-{i}.service"), Severity::Medium));
    }

    let all = state.list_incidents(200, None, None);
    assert_eq!(all.len(), MAX_STORED_INCIDENTS);
    assert_eq!(MAX_STORED_INCIDENTS, 100);

    // Most recent must be storm-104
    assert_eq!(all[0].unit_name, "storm-104.service");
    // Oldest retained must be storm-5 (0..4 were evicted)
    assert_eq!(all[99].unit_name, "storm-5.service");
}
