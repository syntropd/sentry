//! Unit tests for CircuitBreakerRegistry bounding and eviction.

use sentry_safety::circuit::{CircuitBreakerRegistry, CircuitConfig, CircuitState, MAX_TRACKED_UNITS};
use std::time::{Duration, Instant};

#[test]
fn test_circuit_registry_basic_tracking() {
    let now = Instant::now();
    let config = CircuitConfig {
        max_failures: 2,
        ..Default::default()
    };
    let mut registry = CircuitBreakerRegistry::new(config);

    assert_eq!(registry.evaluate("nginx.service", now), CircuitState::Closed);
    registry.record_failure("nginx.service", now);
    assert_eq!(registry.evaluate("nginx.service", now), CircuitState::Closed);

    let state = registry.record_failure("nginx.service", now);
    assert!(matches!(state, CircuitState::Open { .. }));

    let snap = registry.get_snapshot("nginx.service", now).unwrap();
    assert_eq!(snap.state, "OPEN");

    assert!(registry.reset_unit("nginx.service", now));
    assert_eq!(registry.evaluate("nginx.service", now), CircuitState::Closed);
}

#[test]
fn test_circuit_registry_bounded_eviction_on_storm() {
    let mut now = Instant::now();
    let config = CircuitConfig::default();
    let mut registry = CircuitBreakerRegistry::new(config);

    // Flood with distinct ephemeral unit failures
    for i in 0..(MAX_TRACKED_UNITS + 50) {
        let unit_name = format!("ephemeral-{i}.service");
        now += Duration::from_millis(10);
        registry.record_failure(&unit_name, now);
    }

    // Must be strictly bounded at or below MAX_TRACKED_UNITS
    assert!(
        registry.tracked_units_count() <= MAX_TRACKED_UNITS,
        "Tracked units {} exceeds limit {}",
        registry.tracked_units_count(),
        MAX_TRACKED_UNITS
    );
}

#[test]
fn test_circuit_registry_never_evicts_locked_units() {
    let mut now = Instant::now();
    let config = CircuitConfig {
        max_failures: 1,
        cooldown_duration: Duration::from_secs(1),
        flap_window: Duration::from_secs(60),
        flap_threshold: 2,
        ..Default::default()
    };
    let mut registry = CircuitBreakerRegistry::new(config);

    // Trip "critical-flapper.service" twice to trigger PermanentlyLocked
    registry.record_failure("critical-flapper.service", now);
    now += Duration::from_secs(2);
    let locked_state = registry.record_failure("critical-flapper.service", now);
    assert!(matches!(locked_state, CircuitState::PermanentlyLocked { .. }));

    // Now flood with other units to trigger capacity evictions
    for i in 0..(MAX_TRACKED_UNITS + 50) {
        let unit_name = format!("other-worker-{i}.service");
        now += Duration::from_millis(10);
        registry.record_failure(&unit_name, now);
    }

    // Critical flapper must still be preserved in registry
    let snap = registry.get_snapshot("critical-flapper.service", now);
    assert!(snap.is_some(), "Locked unit must not be evicted during flood");
    assert_eq!(snap.unwrap().state, "PERMANENTLY_LOCKED");
}
