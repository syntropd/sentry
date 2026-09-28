//! Unit tests for DiagnosticEngine end-to-end orchestration and fallback.

use async_trait::async_trait;
use sentry_core::error::DiagnosticError;
use sentry_core::models::{
    DiagnosticPayload, DriverEvent, IncidentContext, RemediationAction, RiskLevel, RootCause,
    Severity, UnitFailedDetails,
};
use sentry_diagnostic::engine::DiagnosticEngine;
use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

struct MockProvider {
    raw_response: Result<String, String>,
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn ping(&self) -> Result<ProviderHealth, DiagnosticError> {
        Ok(ProviderHealth {
            available: true,
            provider_name: "mock".to_string(),
            model_name: "mock-model".to_string(),
            latency_ms: 5,
            details: None,
        })
    }

    async fn complete(&self, _prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError> {
        match &self.raw_response {
            Ok(text) => Ok(RawLlmResponse {
                raw_text: text.clone(),
                model: "mock-model".to_string(),
                prompt_tokens: Some(10),
                completion_tokens: Some(20),
                latency: Duration::from_millis(50),
            }),
            Err(e) => Err(DiagnosticError::ProviderUnavailable(e.clone())),
        }
    }

    fn id(&self) -> &'static str {
        "mock"
    }
}

fn sample_incident_context() -> IncidentContext {
    IncidentContext::new(
        "app.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "app.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: Some(137),
            main_pid: Some(1234),
        }),
    )
}

#[tokio::test]
async fn test_engine_fallback_without_provider() {
    let engine = DiagnosticEngine::new(None);
    let ctx = sample_incident_context();
    let payload = engine.diagnose(&ctx).await;

    assert_eq!(payload.unit_name, "app.service");
    assert_eq!(payload.severity, Severity::High);
    assert_eq!(
        payload.proposed_remediation.action,
        RemediationAction::RestartWithBackoff
    );
}

#[tokio::test]
async fn test_engine_successful_provider_diagnosis() {
    let valid_payload = DiagnosticPayload {
        incident_id: Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        unit_name: "other.service".to_string(), // Engine must overwrite this with ground truth
        root_cause: RootCause {
            summary: "Mock analysis".to_string(),
            detail: "Mock detailed analysis".to_string(),
        },
        evidence: Default::default(),
        severity: Severity::Low,
        proposed_remediation: sentry_core::models::ProposedRemediation {
            action: RemediationAction::Reload,
            rationale: "Reload is safe".to_string(),
            risk_level: RiskLevel::Low,
            confidence: 0.99,
        },
    };

    let json_text = serde_json::to_string(&valid_payload).unwrap();
    let mock = Arc::new(MockProvider {
        raw_response: Ok(json_text),
    });

    let engine = DiagnosticEngine::new(Some(mock));
    let ctx = sample_incident_context();
    let payload = engine.diagnose(&ctx).await;

    assert_eq!(payload.unit_name, "app.service"); // Ground truth preserved
    assert_eq!(payload.incident_id, ctx.incident_id); // Ground truth preserved
    assert_eq!(payload.root_cause.summary, "Mock analysis");
    assert_eq!(payload.proposed_remediation.action, RemediationAction::Reload);
}

#[tokio::test]
async fn test_engine_provider_failure_triggers_fallback() {
    let mock = Arc::new(MockProvider {
        raw_response: Err("timeout".to_string()),
    });

    let engine = DiagnosticEngine::new(Some(mock));
    let ctx = sample_incident_context();
    let payload = engine.diagnose(&ctx).await;

    // Must fall back to exit 137 triage
    assert_eq!(payload.unit_name, "app.service");
    assert_eq!(payload.severity, Severity::High);
    assert!(payload.root_cause.summary.contains("Out-Of-Memory"));
}

#[tokio::test]
async fn test_engine_corrupt_json_triggers_fallback() {
    let mock = Arc::new(MockProvider {
        raw_response: Ok("garbage { not valid json".to_string()),
    });

    let engine = DiagnosticEngine::new(Some(mock));
    let ctx = sample_incident_context();
    let payload = engine.diagnose(&ctx).await;

    assert_eq!(payload.unit_name, "app.service");
    assert_eq!(payload.severity, Severity::High);
}
