//! Integration tests for DiagnosticEngine adaptive timeouts and zero-latency circuit fallback.

use async_trait::async_trait;
use sentry_core::error::DiagnosticError;
use sentry_core::models::{
    DiagnosticPayload, DriverEvent, IncidentContext, ProposedRemediation, RemediationAction,
    RiskLevel, RootCause, Severity, UnitFailedDetails,
};
use sentry_diagnostic::circuit::{AdaptiveTimeoutConfig, ProviderCircuitState};
use sentry_diagnostic::engine::DiagnosticEngine;
use sentry_diagnostic::fallback::DeterministicFallbackEngine;
use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::sanitize::DiagnosticSanitizer;
use sentry_diagnostic::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct MockDelayProvider {
    delay: Duration,
    calls: AtomicUsize,
    response_json: String,
}

impl MockDelayProvider {
    fn new(delay: Duration) -> Self {
        let payload = DiagnosticPayload {
            incident_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            unit_name: "placeholder.service".to_string(),
            root_cause: RootCause {
                summary: "AI summary".to_string(),
                detail: "AI detail".to_string(),
            },
            evidence: Default::default(),
            severity: Severity::Low,
            proposed_remediation: ProposedRemediation {
                action: RemediationAction::Reload,
                rationale: "AI rationale".to_string(),
                risk_level: RiskLevel::Low,
                confidence: 0.95,
            },
        };
        Self {
            delay,
            calls: AtomicUsize::new(0),
            response_json: serde_json::to_string(&payload).unwrap(),
        }
    }
}

#[async_trait]
impl LlmProvider for MockDelayProvider {
    async fn ping(&self) -> Result<ProviderHealth, DiagnosticError> {
        Ok(ProviderHealth {
            available: true,
            provider_name: "delay-mock".to_string(),
            model_name: "delay-model".to_string(),
            latency_ms: 1,
            details: None,
        })
    }

    async fn complete(&self, _prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        Ok(RawLlmResponse {
            raw_text: self.response_json.clone(),
            model: "delay-model".to_string(),
            prompt_tokens: Some(10),
            completion_tokens: Some(10),
            latency: self.delay,
        })
    }

    fn id(&self) -> &'static str {
        "delay-mock"
    }
}

fn sample_incident() -> IncidentContext {
    IncidentContext::new(
        "web.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "web.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: Some(137), // OOM kill exit code
            main_pid: Some(999),
        }),
    )
}

#[tokio::test]
async fn test_slow_provider_clean_abort_and_fallback() {
    let mock = Arc::new(MockDelayProvider::new(Duration::from_secs(5)));
    let config = AdaptiveTimeoutConfig::default()
        .with_initial_timeout(Duration::from_millis(50))
        .with_min_timeout(Duration::from_millis(50))
        .with_max_timeout(Duration::from_millis(200));

    let engine = DiagnosticEngine::with_circuit(
        Some(mock),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    );

    let ctx = sample_incident();
    let start = Instant::now();
    let payload = engine.diagnose(&ctx).await;
    let elapsed = start.elapsed();

    // Verify clean abort well before the 5s mock completes (aborts in < 150ms)
    assert!(
        elapsed < Duration::from_millis(150),
        "Expected timeout abort in <150ms, took {elapsed:?}"
    );

    // Verify immediate fallback to DeterministicFallbackEngine (exit 137 -> OOM)
    assert_eq!(payload.unit_name, "web.service");
    assert_eq!(payload.severity, Severity::High);
    assert!(payload.root_cause.summary.contains("Out-Of-Memory"));

    // Verify circuit breaker recorded the timeout
    let breaker = engine.circuit();
    let locked = breaker.lock().unwrap();
    assert_eq!(locked.consecutive_failures(), 1);
}

#[tokio::test]
async fn test_circuit_trip_and_zero_latency_fallback() {
    let mock = Arc::new(MockDelayProvider::new(Duration::from_secs(5)));
    let config = AdaptiveTimeoutConfig::default()
        .with_initial_timeout(Duration::from_millis(30))
        .with_min_timeout(Duration::from_millis(30))
        .with_failure_threshold(3)
        .with_base_cooldown(Duration::from_secs(60));

    let engine = DiagnosticEngine::with_circuit(
        Some(mock.clone()),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    );

    let ctx = sample_incident();

    // Trigger 3 consecutive timeouts to trip the circuit to Open
    for _ in 0..3 {
        let _ = engine.diagnose(&ctx).await;
    }

    assert_eq!(mock.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        engine.circuit().lock().unwrap().state(),
        ProviderCircuitState::Open
    );

    // 4th request while Open must short-circuit with 0ms network latency
    let start = Instant::now();
    let payload = engine.diagnose(&ctx).await;
    let elapsed = start.elapsed();

    // Zero-latency fallback: pure memory triage executes in < 5ms
    assert!(
        elapsed < Duration::from_millis(5),
        "Short-circuit fallback took {elapsed:?}"
    );
    assert_eq!(payload.unit_name, "web.service");
    assert_eq!(payload.severity, Severity::High);

    // Assert the provider was never called for the 4th request
    assert_eq!(
        mock.calls.load(Ordering::SeqCst),
        3,
        "Provider must not be called when circuit is Open"
    );
}

#[tokio::test]
async fn test_fast_provider_records_latency_and_succeeds() {
    let mock = Arc::new(MockDelayProvider::new(Duration::from_millis(20)));
    let config = AdaptiveTimeoutConfig::default()
        .with_initial_timeout(Duration::from_secs(2));

    let engine = DiagnosticEngine::with_circuit(
        Some(mock.clone()),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    );

    let ctx = sample_incident();
    let payload = engine.diagnose(&ctx).await;

    assert_eq!(payload.unit_name, "web.service");
    assert_eq!(payload.proposed_remediation.action, RemediationAction::Reload);

    let breaker = engine.circuit();
    let locked = breaker.lock().unwrap();
    assert_eq!(locked.state(), ProviderCircuitState::Closed);
    assert_eq!(locked.consecutive_failures(), 0);
    assert_eq!(locked.tracker().count(), 1);
}
