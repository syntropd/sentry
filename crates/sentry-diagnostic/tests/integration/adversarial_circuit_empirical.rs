//! Empirical verification suite for Milestone HM3: Adaptive Timeout, Breaker, and Fallback.

use async_trait::async_trait;
use sentry_core::error::DiagnosticError;
use sentry_core::models::{
    DiagnosticPayload, DriverEvent, IncidentContext, RemediationAction, Severity,
    UnitFailedDetails,
};
use sentry_diagnostic::circuit::{
    AdaptiveTimeoutConfig, LatencyTracker, ProviderBreaker, ProviderCircuitState,
};
use sentry_diagnostic::engine::DiagnosticEngine;
use sentry_diagnostic::fallback::DeterministicFallbackEngine;
use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::sanitize::DiagnosticSanitizer;
use sentry_diagnostic::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct AllocCounter;
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static IS_TRACKING: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for AllocCounter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if IS_TRACKING.try_with(|t| t.get()).unwrap_or(false) {
            ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL_ALLOC: AllocCounter = AllocCounter;

struct MockHangingProvider {
    sleep_duration: Duration,
    call_count: AtomicUsize,
}

#[async_trait]
impl LlmProvider for MockHangingProvider {
    async fn ping(&self) -> Result<ProviderHealth, DiagnosticError> {
        Ok(ProviderHealth {
            available: true,
            provider_name: "mock-hang".to_string(),
            model_name: "hang-v1".to_string(),
            latency_ms: 0,
            details: None,
        })
    }

    async fn complete(&self, _prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.sleep_duration).await;
        Err(DiagnosticError::ProviderUnavailable("timed out".to_string()))
    }

    fn id(&self) -> &'static str {
        "mock-hang"
    }
}

fn make_context(unit: &str, exit_code: Option<i32>) -> IncidentContext {
    IncidentContext::new(
        unit,
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: unit.to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: None,
            exec_status: exit_code,
            main_pid: Some(1234),
        }),
    )
}

#[test]
fn test_latency_tracker_zero_heap_allocations_and_footprint() {
    assert!(
        std::mem::size_of::<LatencyTracker>() <= 384,
        "Footprint too large: {} bytes",
        std::mem::size_of::<LatencyTracker>()
    );

    let mut tracker = LatencyTracker::new();
    let config = AdaptiveTimeoutConfig::default();

    // Enable thread-isolated heap allocation tracking
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    IS_TRACKING.with(|t| t.set(true));

    for i in 1..=1000 {
        tracker.record(Duration::from_millis((i % 500) as u64), 0.20);
        let _ = tracker.calculate_timeout(&config);
        let _ = tracker.p95_ms();
    }
    tracker.reset();

    IS_TRACKING.with(|t| t.set(false));
    let total_allocs = ALLOC_COUNT.load(Ordering::SeqCst);

    assert_eq!(
        total_allocs, 0,
        "Expected strictly 0 heap allocations, detected: {total_allocs}"
    );
    println!("[EMPIRICAL] LatencyTracker footprint: {} bytes, allocations across 1000 ops: {total_allocs}", std::mem::size_of::<LatencyTracker>());
}

#[tokio::test]
async fn test_unresponsive_provider_timeout_execution_time() {
    let mock = Arc::new(MockHangingProvider {
        sleep_duration: Duration::from_secs(5),
        call_count: AtomicUsize::new(0),
    });

    let config = AdaptiveTimeoutConfig::default()
        .with_initial_timeout(Duration::from_millis(50))
        .with_min_timeout(Duration::from_millis(50))
        .with_max_timeout(Duration::from_millis(100))
        .with_failure_threshold(10);

    let engine = DiagnosticEngine::with_circuit(
        Some(mock.clone()),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    );

    let ctx = make_context("worker.service", Some(137));
    let mut durations = Vec::new();

    for _ in 0..5 {
        let t0 = Instant::now();
        let payload = engine.diagnose(&ctx).await;
        let elapsed = t0.elapsed();
        durations.push(elapsed);

        assert!(
            elapsed < Duration::from_millis(150),
            "Timeout abort exceeded 150ms SLA: took {elapsed:?}"
        );
        assert_eq!(payload.unit_name, "worker.service");
    }

    let avg_ms = durations.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / durations.len() as f64;
    println!("[EMPIRICAL] 50ms timeout abort latencies: avg={avg_ms:.2}ms, samples={durations:?}");
}

#[tokio::test]
async fn test_circuit_open_fallback_execution_time_and_zero_network() {
    let mock = Arc::new(MockHangingProvider {
        sleep_duration: Duration::from_secs(5),
        call_count: AtomicUsize::new(0),
    });

    let config = AdaptiveTimeoutConfig::default()
        .with_failure_threshold(2)
        .with_base_cooldown(Duration::from_secs(120));

    let mut breaker = ProviderBreaker::new(config.clone());
    breaker.on_error();
    breaker.on_error();
    assert_eq!(breaker.state(), ProviderCircuitState::Open);

    let engine = DiagnosticEngine::with_circuit(
        Some(mock.clone()),
        DeterministicFallbackEngine::new(),
        DiagnosticSanitizer::new(),
        config,
    );
    *engine.circuit().lock().unwrap() = breaker;

    let ctx = make_context("payment.service", Some(139));
    let initial_calls = mock.call_count.load(Ordering::SeqCst);

    let mut latencies_us = Vec::with_capacity(100);
    for _ in 0..100 {
        let t0 = Instant::now();
        let payload = engine.diagnose(&ctx).await;
        let elapsed = t0.elapsed();
        latencies_us.push(elapsed.as_micros());

        assert!(
            elapsed < Duration::from_millis(5),
            "Open circuit fallback exceeded 5ms SLA: {elapsed:?}"
        );
        assert_eq!(payload.unit_name, "payment.service");
        assert_eq!(payload.severity, Severity::Critical);
    }

    let final_calls = mock.call_count.load(Ordering::SeqCst);
    assert_eq!(
        initial_calls, final_calls,
        "Network calls initiated while circuit was Open!"
    );

    let avg_us = latencies_us.iter().sum::<u128>() as f64 / latencies_us.len() as f64;
    let max_us = latencies_us.iter().max().copied().unwrap_or(0);
    println!("[EMPIRICAL] Circuit Open fallback (100 runs): avg={avg_us:.2}µs, max={max_us}µs, network_calls=0");
}

#[tokio::test]
async fn test_deterministic_fallback_payload_completeness() {
    let fallback = DeterministicFallbackEngine::new();

    let test_matrix = [
        (Some(137), Severity::High, RemediationAction::RestartWithBackoff),
        (Some(139), Severity::Critical, RemediationAction::EscalateToAdmin),
        (Some(203), Severity::Critical, RemediationAction::NoAction),
        (None, Severity::Medium, RemediationAction::RestartWithBackoff),
    ];

    for (exit_code, expected_sev, expected_action) in test_matrix {
        let ctx = make_context("db.service", exit_code);
        let payload = fallback.triage(&ctx);

        assert_eq!(payload.unit_name, ctx.unit);
        assert_eq!(payload.incident_id, ctx.incident_id);
        assert!(!payload.root_cause.summary.trim().is_empty());
        assert!(!payload.root_cause.detail.trim().is_empty());
        assert!(!payload.proposed_remediation.rationale.trim().is_empty());
        assert!(payload.proposed_remediation.confidence > 0.0);
        assert!(payload.proposed_remediation.confidence <= 1.0);
        assert_eq!(payload.severity, expected_sev);
        assert_eq!(payload.proposed_remediation.action, expected_action);

        let serialized = serde_json::to_string(&payload).expect("Serialization failed");
        let deserialized: DiagnosticPayload = serde_json::from_str(&serialized).expect("Deserialization failed");
        assert_eq!(deserialized.unit_name, payload.unit_name);
        assert_eq!(deserialized.incident_id, payload.incident_id);
    }
}
