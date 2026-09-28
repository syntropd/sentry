//! Unit tests for MCP resource reader.

use sentry_core::models::{DiagnosticPayload, RemediationAction, RiskLevel, RootCause, Severity};
use sentry_mcp::resources::read_resource;
use sentry_mcp::storage::McpState;
use uuid::Uuid;

#[test]
fn test_read_policy_resource() {
    let state = McpState::new();
    state.set_policy("[general]\nenabled = true\n".to_string());

    let res = read_resource(&state, "policy://current").expect("Read policy");
    let content = &res["contents"][0];
    assert_eq!(content["mimeType"], "text/x-toml");
    assert!(content["text"].as_str().unwrap().contains("enabled = true"));
}

#[test]
fn test_read_incident_resource() {
    let state = McpState::new();
    let id = Uuid::new_v4();
    let inc = DiagnosticPayload {
        incident_id: id,
        timestamp: chrono::Utc::now(),
        unit_name: "test.service".to_string(),
        root_cause: RootCause {
            summary: "OOM".to_string(),
            detail: "killed".to_string(),
        },
        evidence: Default::default(),
        severity: Severity::High,
        proposed_remediation: sentry_core::models::ProposedRemediation {
            action: RemediationAction::RestartWithBackoff,
            rationale: "OOM backoff".to_string(),
            risk_level: RiskLevel::Medium,
            confidence: 0.90,
        },
    };
    state.record_incident(inc);

    let uri = format!("incident://{id}");
    let res = read_resource(&state, &uri).expect("Read incident");
    let content = &res["contents"][0];
    assert_eq!(content["mimeType"], "application/json");
    assert!(content["text"].as_str().unwrap().contains(&id.to_string()));
}

#[test]
fn test_read_telemetry_resource() {
    let state = McpState::new();
    let res = read_resource(&state, "telemetry://nginx.service").expect("Read telemetry");
    let content = &res["contents"][0];
    assert_eq!(content["mimeType"], "application/json");
    assert!(content["text"].as_str().unwrap().contains("nginx.service"));
}

#[test]
fn test_read_invalid_scheme_returns_error() {
    let state = McpState::new();
    assert!(read_resource(&state, "unknown://abc").is_err());
}
