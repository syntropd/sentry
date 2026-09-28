//! Unit tests for multi-stage SanitizationPipeline.

use sentry_core::models::{DiagnosticPayload, RemediationAction, RiskLevel, RootCause, Severity};
use sentry_diagnostic::sanitize::SanitizationPipeline;
use uuid::Uuid;

fn sample_payload() -> DiagnosticPayload {
    DiagnosticPayload {
        incident_id: Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        unit_name: "test.service".to_string(),
        root_cause: RootCause {
            summary: "Crash".to_string(),
            detail: "Detailed crash".to_string(),
        },
        evidence: Default::default(),
        severity: Severity::High,
        proposed_remediation: sentry_core::models::ProposedRemediation {
            action: RemediationAction::RestartWithBackoff,
            rationale: "Safe restart".to_string(),
            risk_level: RiskLevel::Medium,
            confidence: 0.90,
        },
    }
}

#[test]
fn test_pipeline_valid_payload() {
    let pipeline = SanitizationPipeline::new();
    let payload = sample_payload();
    let json_str = serde_json::to_string(&payload).unwrap();

    let parsed = pipeline.process(&json_str).expect("Valid parse");
    assert_eq!(parsed.unit_name, "test.service");
    assert_eq!(parsed.severity, Severity::High);
}

#[test]
fn test_pipeline_with_markdown_fences_and_filler() {
    let pipeline = SanitizationPipeline::new();
    let payload = sample_payload();
    let json_str = serde_json::to_string(&payload).unwrap();
    let wrapped = format!("Here is the triage:\n```json\n{json_str}\n```\nHope it helps!");

    let parsed = pipeline.process(&wrapped).expect("Valid parse after strip");
    assert_eq!(parsed.unit_name, "test.service");
}

#[test]
fn test_pipeline_rejects_out_of_bounds_confidence() {
    let pipeline = SanitizationPipeline::new();
    let mut payload = sample_payload();
    payload.proposed_remediation.confidence = 1.5;
    let json_str = serde_json::to_string(&payload).unwrap();

    assert!(pipeline.process(&json_str).is_err());
}

#[test]
fn test_pipeline_rejects_garbage() {
    let pipeline = SanitizationPipeline::new();
    assert!(pipeline.process("not a json payload").is_err());
}
