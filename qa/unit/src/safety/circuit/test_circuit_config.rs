//! Unit tests for circuit breaker configuration.

use sentry_safety::circuit::CircuitConfig;
use std::time::Duration;

#[test]
fn test_circuit_config_defaults() {
    let cfg = CircuitConfig::default();
    assert_eq!(cfg.max_failures, 3);
    assert_eq!(cfg.window_duration, Duration::from_secs(60));
    assert_eq!(cfg.cooldown_duration, Duration::from_secs(30));
    assert_eq!(cfg.base_cooldown(), Duration::from_secs(30));
    assert_eq!(cfg.max_cooldown, Duration::from_secs(1800));
    assert_eq!(cfg.flap_window, Duration::from_secs(900));
    assert_eq!(cfg.flap_threshold, 3);
}

#[test]
fn test_circuit_config_custom_construction() {
    let cfg = CircuitConfig::new(
        5,
        Duration::from_secs(60),
        Duration::from_secs(120),
        Duration::from_secs(600),
        4,
    )
    .with_max_cooldown(Duration::from_secs(3600));
    assert_eq!(cfg.max_failures, 5);
    assert_eq!(cfg.window_duration, Duration::from_secs(60));
    assert_eq!(cfg.cooldown_duration, Duration::from_secs(120));
    assert_eq!(cfg.max_cooldown, Duration::from_secs(3600));
    assert_eq!(cfg.flap_window, Duration::from_secs(600));
    assert_eq!(cfg.flap_threshold, 4);
}

#[test]
fn test_circuit_config_clamping_minimums() {
    let cfg = CircuitConfig::new(0, Duration::ZERO, Duration::ZERO, Duration::ZERO, 0);
    assert_eq!(cfg.max_failures, 1);
    assert_eq!(cfg.flap_threshold, 1);
    assert_eq!(cfg.max_cooldown, Duration::from_secs(1800));
}
