//! Unit tests for ProviderBreaker state transitions, cooldown backoff, and probe throttling.

use sentry_diagnostic::circuit::{
    AdaptiveTimeoutConfig, ProviderAction, ProviderBreaker, ProviderCircuitState,
};
use std::time::{Duration, Instant};

#[test]
fn test_breaker_trip_on_consecutive_failures() {
    let config = AdaptiveTimeoutConfig::default()
        .with_failure_threshold(3)
        .with_base_cooldown(Duration::from_secs(30));
    let mut breaker = ProviderBreaker::new(config);

    assert_eq!(breaker.state(), ProviderCircuitState::Closed);
    assert_eq!(breaker.consecutive_failures(), 0);

    // First failure
    breaker.on_timeout();
    assert_eq!(breaker.state(), ProviderCircuitState::Closed);
    assert_eq!(breaker.consecutive_failures(), 1);

    // Second failure
    breaker.on_error();
    assert_eq!(breaker.state(), ProviderCircuitState::Closed);
    assert_eq!(breaker.consecutive_failures(), 2);

    // Third failure -> Tripped to Open!
    breaker.on_timeout();
    assert_eq!(breaker.state(), ProviderCircuitState::Open);
    assert_eq!(breaker.consecutive_failures(), 3);
    assert!(breaker.tripped_at().is_some());
    assert_eq!(breaker.current_cooldown(), Duration::from_secs(30));
}

#[test]
fn test_breaker_open_short_circuits_before_cooldown() {
    let config = AdaptiveTimeoutConfig::default()
        .with_failure_threshold(1)
        .with_base_cooldown(Duration::from_secs(30));
    let mut breaker = ProviderBreaker::new(config);

    breaker.on_error();
    assert_eq!(breaker.state(), ProviderCircuitState::Open);

    // Request before cooldown expires must short-circuit
    let action = breaker.before_request();
    assert_eq!(action, ProviderAction::ShortCircuit);
}

#[test]
fn test_breaker_half_open_probe_and_recovery() {
    let config = AdaptiveTimeoutConfig::default()
        .with_failure_threshold(1)
        .with_base_cooldown(Duration::from_secs(30));
    let mut breaker = ProviderBreaker::new(config);

    breaker.on_timeout();
    assert_eq!(breaker.state(), ProviderCircuitState::Open);

    // Simulate cooldown elapsed
    breaker.set_tripped_at(Some(Instant::now() - Duration::from_secs(35)));

    // First request after cooldown enters HalfOpen as a trial probe
    let action = breaker.before_request();
    assert!(action.is_proceed());
    assert_eq!(breaker.state(), ProviderCircuitState::HalfOpen);
    assert!(breaker.is_probe_in_flight());

    // Stampede prevention: concurrent requests while probe is active must short-circuit
    let concurrent_action = breaker.before_request();
    assert_eq!(concurrent_action, ProviderAction::ShortCircuit);

    // Probe succeeds!
    breaker.on_success(Duration::from_millis(200));
    assert_eq!(breaker.state(), ProviderCircuitState::Closed);
    assert_eq!(breaker.consecutive_failures(), 0);
    assert!(!breaker.is_probe_in_flight());
    assert_eq!(breaker.current_cooldown(), Duration::from_secs(30));

    // Next request proceeds normally
    let next_action = breaker.before_request();
    assert!(next_action.is_proceed());
}

#[test]
fn test_breaker_half_open_failure_exponential_backoff() {
    let config = AdaptiveTimeoutConfig::default()
        .with_failure_threshold(1)
        .with_base_cooldown(Duration::from_secs(30))
        .with_max_cooldown(Duration::from_secs(100));
    let mut breaker = ProviderBreaker::new(config);

    // Initial trip: cooldown = 30s
    breaker.on_error();
    assert_eq!(breaker.current_cooldown(), Duration::from_secs(30));

    // Simulate cooldown elapsed -> transition to HalfOpen
    breaker.set_tripped_at(Some(Instant::now() - Duration::from_secs(31)));
    let _ = breaker.before_request();
    assert_eq!(breaker.state(), ProviderCircuitState::HalfOpen);

    // Probe fails -> Re-trips with doubled cooldown (60s)
    breaker.on_timeout();
    assert_eq!(breaker.state(), ProviderCircuitState::Open);
    assert_eq!(breaker.current_cooldown(), Duration::from_secs(60));
    assert!(!breaker.is_probe_in_flight());

    // Advance 61s -> HalfOpen
    breaker.set_tripped_at(Some(Instant::now() - Duration::from_secs(61)));
    let _ = breaker.before_request();
    assert_eq!(breaker.state(), ProviderCircuitState::HalfOpen);

    // Second probe fails -> Doubled cooldown capped at max_cooldown (100s)
    breaker.on_error();
    assert_eq!(breaker.state(), ProviderCircuitState::Open);
    assert_eq!(breaker.current_cooldown(), Duration::from_secs(100));
}
