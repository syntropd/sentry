//! Adversarial tests verifying unit_name and incident_id invariants and provider error handling.

use async_trait::async_trait;
use chrono::Utc;
use sentry_core::error::DiagnosticError;
use sentry_core::models::{
    DiagnosticPayload, DriverEvent, IncidentContext, ProposedRemediation, RemediationAction,
    RiskLevel, RootCause, Severity, UnitFailedDetails,
};
use sentry_diagnostic::engine::DiagnosticEngine;
use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

struct DynamicMockProvider {
    result: Result<String, DiagnosticError>,
}

#[async_trait]
impl LlmProvider for DynamicMockProvider {
    async fn ping(&self) -> Result<ProviderHealth, DiagnosticError> {
        Ok(ProviderHealth {
            available: true,
            provider_name: "mock".to_string(),
            model_name: "m".to_string(),
            latency_ms: 1,
            details: None,
        })
    }

    async fn complete(&self, _prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError> {
        match &self.result {
            Ok(text) => Ok(RawLlmResponse {
                raw_text: text.clone(),
                model: "m".to_string(),
                prompt_tokens: Some(10),
                completion_tokens: Some(10),
                latency: Duration::from_millis(2),
            }),
            Err(e) => Err(DiagnosticError::ProviderUnavailable(e.to_string())),
        }
    }

    fn id(&self) -> &'static str {
        "dynamic-mock"
    }
}

fn create_context(unit: &str, id: Uuid) -> IncidentContext {
    let mut ctx = IncidentContext::new(
        unit,
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: unit.to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: Some("resources".to_string()),
            exec_status: Some(137),
            main_pid: Some(555),
        }),
    );
    ctx.incident_id = id;
    ctx
}

#[tokio::test]
async fn test_hallucinated_identifiers_are_unconditionally_overridden() {
    let true_unit = "legitimate-app@instance-1.service";
    let true_id = Uuid::new_v4();
    let ctx = create_context(true_unit, true_id);

    // LLM outputs a valid schema, but hallucinates a malicious target and fake UUID
    let hallucinated_id = Uuid::nil();
    let hallucinated_payload = DiagnosticPayload {
        incident_id: hallucinated_id,
        timestamp: Utc::now(),
        unit_name: "attacker-controlled-unit.service".to_string(),
        root_cause: RootCause {
            summary: "Compromised".to_string(),
            detail: "Adversarial takeover attempt".to_string(),
        },
        evidence: Default::default(),
        severity: Severity::Critical,
        proposed_remediation: ProposedRemediation {
            action: RemediationAction::EscalateToAdmin,
            rationale: "Escalate".to_string(),
            risk_level: RiskLevel::High,
            confidence: 0.99,
        },
    };

    let mock = Arc::new(DynamicMockProvider {
        result: Ok(serde_json::to_string(&hallucinated_payload).unwrap()),
    });

    let engine = DiagnosticEngine::new(Some(mock));
    let diagnosis = engine.diagnose(&ctx).await;

    // Ground truth MUST be preserved, hallucination strictly rejected
    assert_eq!(diagnosis.unit_name, true_unit);
    assert_eq!(diagnosis.incident_id, true_id);
    assert_ne!(diagnosis.unit_name, "attacker-controlled-unit.service");
    assert_ne!(diagnosis.incident_id, hallucinated_id);
}

#[tokio::test]
async fn test_provider_errors_trigger_fallback_without_panic() {
    let ctx = create_context("critical-database.service", Uuid::new_v4());

    let provider_errors = [
        DiagnosticError::ProviderUnavailable("Connection refused (os error 111)".to_string()),
        DiagnosticError::ProviderUnavailable("HTTP 401 Unauthorized: bad bearer key".to_string()),
        DiagnosticError::ProviderUnavailable("HTTP 429 Too Many Requests: quota exhausted".to_string()),
        DiagnosticError::ProviderUnavailable("HTTP 500 Internal Server Error".to_string()),
        DiagnosticError::ProviderUnavailable("HTTP 503 Service Unavailable".to_string()),
        DiagnosticError::ProviderUnavailable("HTTP response stream exceeded size limit".to_string()),
        DiagnosticError::MalformedJson("premature EOF".to_string()),
        DiagnosticError::SchemaValidation("confidence out of bounds".to_string()),
    ];

    for err in provider_errors {
        let mock = Arc::new(DynamicMockProvider { result: Err(err) });
        let engine = DiagnosticEngine::new(Some(mock));

        let diagnosis = engine.diagnose(&ctx).await;

        // Invariant checks
        assert_eq!(diagnosis.unit_name, ctx.unit);
        assert_eq!(diagnosis.incident_id, ctx.incident_id);
        assert!(!diagnosis.root_cause.summary.is_empty());
        assert_eq!(diagnosis.proposed_remediation.action, RemediationAction::RestartWithBackoff);
    }
}

#[tokio::test]
async fn test_provider_returning_html_error_pages() {
    let ctx = create_context("web-worker.service", Uuid::new_v4());

    let html_errors = [
        "<!DOCTYPE html><html><head><title>502 Bad Gateway</title></head><body>Cloudflare Error</body></html>",
        "<html><body><h1>504 Gateway Time-out</h1>The server didn't respond in time.</body></html>",
        "Error 403 (Forbidden)!!1",
    ];

    for html in html_errors {
        let mock = Arc::new(DynamicMockProvider {
            result: Ok(html.to_string()),
        });
        let engine = DiagnosticEngine::new(Some(mock));

        let diagnosis = engine.diagnose(&ctx).await;
        assert_eq!(diagnosis.unit_name, ctx.unit);
        assert_eq!(diagnosis.incident_id, ctx.incident_id);
    }
}
