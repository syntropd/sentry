//! Unit tests for MCP prompt generator.

use sentry_core::models::{DiagnosticPayload, RemediationAction, RiskLevel, RootCause, Severity};
use sentry_mcp::prompts::{generate_prompt, list_prompts};
use sentry_mcp::storage::McpState;
use serde_json::json;
use uuid::Uuid;

#[test]
fn test_generate_triage_incident_prompt() {
    let state = McpState::new();
    let id = Uuid::new_v4();
    let inc = DiagnosticPayload {
        incident_id: id,
        timestamp: chrono::Utc::now(),
        unit_name: "db.service".to_string(),
        root_cause: RootCause {
            summary: "Crash".to_string(),
            detail: "Fatal error".to_string(),
        },
        evidence: Default::default(),
        severity: Severity::Critical,
        proposed_remediation: sentry_core::models::ProposedRemediation {
            action: RemediationAction::EscalateToAdmin,
            rationale: "Data safety".to_string(),
            risk_level: RiskLevel::High,
            confidence: 0.99,
        },
    };
    state.record_incident(inc);

    let args = json!({ "incident_id": id.to_string() });
    let prompt = generate_prompt(&state, "triage_incident", Some(&args)).expect("Generated prompt");

    let text = prompt["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(text.contains(&id.to_string()));
    assert!(text.contains("db.service"));
    assert!(text.contains("Safety Policy"));
}

#[test]
fn test_generate_flapping_service_prompt() {
    let state = McpState::new();
    let args = json!({ "unit_name": "flapper.service" });
    let prompt = generate_prompt(&state, "analyze_flapping_service", Some(&args))
        .expect("Generated flap prompt");

    let text = prompt["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(text.contains("flapper.service"));
    assert!(text.contains("Circuit Breaker State"));
}

#[test]
fn test_generate_prompt_missing_args_returns_error() {
    let state = McpState::new();
    assert!(generate_prompt(&state, "triage_incident", None).is_err());
}

#[test]
fn test_list_prompts_contains_required_templates() {
    let prompts = list_prompts();
    let arr = prompts.as_array().expect("Prompts is array");

    let names: Vec<&str> = arr.iter().filter_map(|p| p["name"].as_str()).collect();
    assert!(names.contains(&"triage_incident"));
    assert!(names.contains(&"analyze_flapping_service"));
    assert_eq!(names.len(), 2);
}
