//! Adversarial stress suite targeting timeout and circuit breaker edge cases.

use async_trait::async_trait;
use sentry_core::error::DiagnosticError;
use sentry_core::models::{DriverEvent, IncidentContext, UnitFailedDetails};
use sentry_diagnostic::circuit::{AdaptiveTimeoutConfig, LatencyTracker, ProviderCircuitState};
use sentry_diagnostic::engine::DiagnosticEngine;
use sentry_diagnostic::fallback::DeterministicFallbackEngine;
use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::sanitize::DiagnosticSanitizer;
use sentry_diagnostic::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct ControllableMockProvider {
    delay: Duration,
    calls: AtomicUsize,
    should_fail: bool,
}

#[async_trait]
impl LlmProvider for ControllableMockProvider {
    async fn ping(&self) -> Result<ProviderHealth, DiagnosticError> {
        Ok(ProviderHealth {
            available: true,
            provider_name: "controllable-mock".into(),
            model_name: "mock".into(),
            latency_ms: 1,
            details: None,
        })
    }

    async fn complete(&self, _prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.delay > Duration::ZERO {
            tokio::time::sleep(self.delay).await;
        }
        if self.should_fail {
            return Err(DiagnosticError::ProviderUnavailable("failure".into()));
        }
        Ok(RawLlmResponse {
            raw_text: r#"{"root_cause":{"summary":"AI triage","detail":"None"},"severity":"low","proposed_remediation":{"action":"reload","rationale":"ok","risk_level":"low","confidence":0.95}}"#.into(),
            model: "mock".into(),
            prompt_tokens: Some(5),
            completion_tokens: Some(5),
            latency: self.delay,
        })
    }

    fn id(&self) -> &'static str {
        "controllable-mock"
    }
}

fn sample_ctx(unit: &str) -> IncidentContext {
    IncidentContext::new(
        unit,
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: unit.to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: Some(137),
            main_pid: Some(1234),
        }),
    )
}

#[tokio::test]
async fn test_cascading_timeouts_trip_circuit() {
    let mock = Arc::new(ControllableMockProvider {
        delay: Duration::from_millis(200),
        calls: AtomicUsize::new(0),
        should_fail: false,
    });
    let config = AdaptiveTimeoutConfig::default()
        .with_min_timeout(Duration::from_millis(20))
        .with_initial_timeout(Duration::from_millis(20))
        .with_failure_threshold(3);

    let engine = DiagnosticEngine::with_circuit(
        Some(mock.clone()),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    );
    let ctx = sample_ctx("service-timeout.service");
    let breaker = engine.circuit();

    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::Closed);
    let _ = engine.diagnose(&ctx).await;
    assert_eq!(breaker.lock().unwrap().consecutive_failures(), 1);
    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::Closed);

    let _ = engine.diagnose(&ctx).await;
    assert_eq!(breaker.lock().unwrap().consecutive_failures(), 2);
    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::Closed);

    let _ = engine.diagnose(&ctx).await;
    assert_eq!(breaker.lock().unwrap().consecutive_failures(), 3);
    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::Open);
    assert!(breaker.lock().unwrap().tripped_at().is_some());
}

#[tokio::test]
async fn test_short_circuiting_100_concurrent_requests_under_10ms() {
    let mock = Arc::new(ControllableMockProvider {
        delay: Duration::from_millis(500),
        calls: AtomicUsize::new(0),
        should_fail: false,
    });
    let config = AdaptiveTimeoutConfig::default().with_failure_threshold(1);
    let engine = Arc::new(DiagnosticEngine::with_circuit(
        Some(mock.clone()),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    ));

    engine.circuit().lock().unwrap().on_error();
    assert_eq!(engine.circuit().lock().unwrap().state(), ProviderCircuitState::Open);

    let start = Instant::now();
    let mut handles = Vec::with_capacity(100);
    for i in 0..100 {
        let eng = engine.clone();
        handles.push(tokio::spawn(async move {
            eng.diagnose(&sample_ctx(&format!("batch-{i}.service"))).await
        }));
    }

    for (i, h) in handles.into_iter().enumerate() {
        let p = h.await.expect("Task must not panic");
        assert_eq!(p.unit_name, format!("batch-{i}.service"));
    }
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(10), "Took {elapsed:?}, expected < 10ms");
    assert_eq!(mock.calls.load(Ordering::SeqCst), 0, "No calls when Open");
}

#[tokio::test]
async fn test_half_open_probe_recovery_and_stampede() {
    let mock = Arc::new(ControllableMockProvider {
        delay: Duration::from_millis(20),
        calls: AtomicUsize::new(0),
        should_fail: false,
    });
    let config = AdaptiveTimeoutConfig::default()
        .with_failure_threshold(1)
        .with_base_cooldown(Duration::from_secs(30))
        .with_max_cooldown(Duration::from_secs(120));
    let engine = Arc::new(DiagnosticEngine::with_circuit(
        Some(mock.clone()),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    ));

    let breaker = engine.circuit();
    breaker.lock().unwrap().on_timeout();
    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::Open);
    breaker.lock().unwrap().set_tripped_at(Some(Instant::now() - Duration::from_secs(35)));

    let mut handles = Vec::new();
    for i in 0..11 {
        let eng = engine.clone();
        handles.push(tokio::spawn(async move {
            eng.diagnose(&sample_ctx(&format!("stampede-{i}.service"))).await
        }));
    }
    for h in handles {
        let _ = h.await.expect("No panic");
    }

    assert_eq!(mock.calls.load(Ordering::SeqCst), 1, "Exactly 1 probe executed");
    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::Closed);
    assert_eq!(breaker.lock().unwrap().current_cooldown(), Duration::from_secs(30));

    // Probe failure doubles cooldown
    breaker.lock().unwrap().on_error();
    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::Open);
    breaker.lock().unwrap().set_tripped_at(Some(Instant::now() - Duration::from_secs(35)));
    let _ = breaker.lock().unwrap().before_request();
    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::HalfOpen);

    breaker.lock().unwrap().on_timeout();
    assert_eq!(breaker.lock().unwrap().state(), ProviderCircuitState::Open);
    assert_eq!(breaker.lock().unwrap().current_cooldown(), Duration::from_secs(60));
}

#[tokio::test]
async fn test_rapid_bursts_and_thread_contention() {
    let mock = Arc::new(ControllableMockProvider {
        delay: Duration::from_millis(1),
        calls: AtomicUsize::new(0),
        should_fail: false,
    });
    let engine = Arc::new(DiagnosticEngine::new(Some(mock)));
    let mut tasks = Vec::new();
    for i in 0..150 {
        let eng = engine.clone();
        tasks.push(tokio::spawn(async move {
            eng.diagnose(&sample_ctx(&format!("burst-{i}.service"))).await
        }));
    }
    for t in tasks {
        let res = t.await.expect("Must not panic");
        assert!(!res.unit_name.is_empty());
    }
}

#[test]
fn test_extreme_and_zero_latencies_no_panic() {
    let mut tracker = LatencyTracker::new();
    let config = AdaptiveTimeoutConfig::default();
    for _ in 0..100 {
        tracker.record(Duration::ZERO, 0.20);
    }
    assert_eq!(tracker.calculate_timeout(&config), config.min_timeout);

    for _ in 0..100 {
        tracker.record(Duration::from_secs(100_000), 0.20);
    }
    assert_eq!(tracker.calculate_timeout(&config), config.max_timeout);
    assert!(tracker.p95_ms() > 0);
}
