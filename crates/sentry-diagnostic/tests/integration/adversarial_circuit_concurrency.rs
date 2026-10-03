//! Adversarial concurrency and edge-case stress test for circuit breaker and timeout.

use async_trait::async_trait;
use sentry_core::error::DiagnosticError;
use sentry_core::models::{DriverEvent, IncidentContext, UnitFailedDetails};
use sentry_diagnostic::circuit::{
    AdaptiveTimeoutConfig, LatencyTracker, ProviderBreaker, ProviderCircuitState,
};
use sentry_diagnostic::engine::DiagnosticEngine;
use sentry_diagnostic::fallback::DeterministicFallbackEngine;
use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::sanitize::DiagnosticSanitizer;
use sentry_diagnostic::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct ProbeProbeProvider {
    probe_call_count: AtomicUsize,
    delay: Duration,
}

#[async_trait]
impl LlmProvider for ProbeProbeProvider {
    async fn ping(&self) -> Result<ProviderHealth, DiagnosticError> {
        Ok(ProviderHealth {
            available: true,
            provider_name: "probe-test".to_string(),
            model_name: "probe-v1".to_string(),
            latency_ms: 5,
            details: None,
        })
    }

    async fn complete(&self, _prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError> {
        self.probe_call_count.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        Ok(RawLlmResponse {
            raw_text: r#"{"root_cause":{"summary":"ok","detail":"ok"},"evidence":{},"severity":"Low","proposed_remediation":{"action":"Reload","rationale":"r","risk_level":"Low","confidence":0.9}}"#.to_string(),
            model: "probe-v1".to_string(),
            prompt_tokens: Some(5),
            completion_tokens: Some(5),
            latency: self.delay,
        })
    }

    fn id(&self) -> &'static str {
        "probe-test"
    }
}

fn incident(unit: &str) -> IncidentContext {
    IncidentContext::new(
        unit,
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: unit.to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: Some(137),
            main_pid: Some(42),
        }),
    )
}

#[tokio::test]
async fn test_half_open_probe_stampede_suppression() {
    let mock = Arc::new(ProbeProbeProvider {
        probe_call_count: AtomicUsize::new(0),
        delay: Duration::from_millis(50),
    });

    let config = AdaptiveTimeoutConfig::default()
        .with_failure_threshold(1)
        .with_base_cooldown(Duration::from_secs(10));

    let mut breaker = ProviderBreaker::new(config.clone());
    breaker.on_error();
    assert_eq!(breaker.state(), ProviderCircuitState::Open);

    // Simulate cooldown elapsed so circuit is ready for HalfOpen probe
    breaker.set_tripped_at(Some(Instant::now() - Duration::from_secs(15)));

    let engine = Arc::new(DiagnosticEngine::with_circuit(
        Some(mock.clone()),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    ));
    *engine.circuit().lock().unwrap() = breaker;

    // Launch 25 concurrent tasks
    let mut tasks = Vec::new();
    for i in 0..25 {
        let eng = Arc::clone(&engine);
        let ctx = incident(&format!("srv-{i}.service"));
        tasks.push(tokio::spawn(async move {
            eng.diagnose(&ctx).await
        }));
    }

    for task in tasks {
        let payload = task.await.expect("Task failed");
        assert!(!payload.root_cause.summary.is_empty());
    }

    // Exactly 1 probe request must have been dispatched to provider
    let calls = mock.probe_call_count.load(Ordering::SeqCst);
    assert_eq!(calls, 1, "Expected exactly 1 probe call during HalfOpen, got: {calls}");
}

#[test]
fn test_latency_tracker_extreme_inputs_and_clamping() {
    let mut tracker = LatencyTracker::new();

    // Inverted config: min > max must not panic
    let inverted = AdaptiveTimeoutConfig::default()
        .with_min_timeout(Duration::from_secs(10))
        .with_max_timeout(Duration::from_secs(2));

    tracker.record_ms(0, 0.20);
    tracker.record_ms(u32::MAX, 0.20);
    let timeout = tracker.calculate_timeout(&inverted);
    assert!(timeout >= Duration::from_secs(10));

    // Extreme alphas: negative, > 1.0
    tracker.record_ms(100, -10.0);
    tracker.record_ms(200, 50.0);
    assert!(tracker.ema_ms().unwrap() > 0.0);

    // Bounded capacity stress
    for _ in 0..500 {
        tracker.record_ms(50, 0.20);
    }
    assert_eq!(tracker.count(), 64);
    assert_eq!(tracker.p95_ms(), 50);
}
