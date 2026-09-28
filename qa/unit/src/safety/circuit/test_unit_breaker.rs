//! Unit tests for individual UnitBreaker logic.

use sentry_safety::circuit::{CircuitConfig, CircuitState, UnitBreaker};
use std::time::{Duration, Instant};

#[test]
fn test_unit_breaker_initial_state() {
    let now = Instant::now();
    let breaker = UnitBreaker::new("web.service", now);
    assert_eq!(breaker.unit(), "web.service");
    let snap = breaker.snapshot(now);
    assert_eq!(snap.state, "CLOSED");
    assert_eq!(snap.recent_failures, 0);
    assert!(!snap.permanently_locked);
}

#[test]
fn test_unit_breaker_trip_and_cooldown_to_half_open() {
    let mut now = Instant::now();
    let config = CircuitConfig {
        max_failures: 2,
        window_duration: Duration::from_secs(60),
        cooldown_duration: Duration::from_secs(10),
        max_cooldown: Duration::from_secs(1800),
        flap_window: Duration::from_secs(300),
        flap_threshold: 3,
    };

    let mut breaker = UnitBreaker::new("api.service", now);

    // 1st failure: still CLOSED
    let st = breaker.record_failure(now, &config);
    assert_eq!(*st, CircuitState::Closed);

    // 2nd failure: trips OPEN
    let st = breaker.record_failure(now, &config);
    assert!(matches!(st, CircuitState::Open { failure_count: 2, .. }));

    // Before cooldown elapses: still OPEN
    now += Duration::from_secs(5);
    let st = breaker.evaluate_state(now);
    assert!(matches!(st, CircuitState::Open { .. }));

    // After cooldown: transitions to HALF_OPEN
    now += Duration::from_secs(6);
    let st = breaker.evaluate_state(now);
    assert_eq!(*st, CircuitState::HalfOpen);

    // Success in HalfOpen: returns to CLOSED
    breaker.record_success(now);
    let snap = breaker.snapshot(now);
    assert_eq!(snap.state, "CLOSED");
    assert_eq!(snap.recent_failures, 0);
}

#[test]
fn test_unit_breaker_flapping_lockout_and_operator_reset() {
    let mut now = Instant::now();
    let config = CircuitConfig {
        max_failures: 1,
        window_duration: Duration::from_secs(60),
        cooldown_duration: Duration::from_secs(5),
        max_cooldown: Duration::from_secs(1800),
        flap_window: Duration::from_secs(60),
        flap_threshold: 3,
    };

    let mut breaker = UnitBreaker::new("flapper.service", now);

    // Trip 1 (cooldown = 5s * 2^0 = 5s)
    breaker.record_failure(now, &config);
    now += Duration::from_secs(6);
    breaker.evaluate_state(now); // HalfOpen

    // Trip 2 (cooldown = 5s * 2^1 = 10s)
    breaker.record_failure(now, &config);
    now += Duration::from_secs(11);
    breaker.evaluate_state(now); // HalfOpen

    // Trip 3 -> PermanentlyLocked!
    let st = breaker.record_failure(now, &config);
    assert!(matches!(st, CircuitState::PermanentlyLocked { flap_trips: 3, .. }));

    // Additional failures do not panic and remain locked
    let st = breaker.record_failure(now, &config);
    assert!(matches!(st, CircuitState::PermanentlyLocked { .. }));

    // Operator reset restores CLOSED
    breaker.reset(now);
    let snap = breaker.snapshot(now);
    assert_eq!(snap.state, "CLOSED");
    assert!(!snap.permanently_locked);
}

#[test]
fn test_unit_breaker_exponential_backoff() {
    let mut now = Instant::now();
    let config = CircuitConfig {
        max_failures: 1,
        window_duration: Duration::from_secs(60),
        cooldown_duration: Duration::from_secs(10), // base cooldown = 10s
        max_cooldown: Duration::from_secs(50),      // max cooldown = 50s
        flap_window: Duration::from_secs(600),
        flap_threshold: 10,                         // high threshold to observe multiple backoffs
    };

    let mut breaker = UnitBreaker::new("scaling.service", now);

    // Trip 1: 10s * 2^0 = 10s
    let st = breaker.record_failure(now, &config);
    assert_eq!(
        *st,
        CircuitState::Open {
            tripped_at: now,
            cooldown: Duration::from_secs(10),
            failure_count: 1,
        }
    );

    // Transition to HalfOpen after 10s
    now += Duration::from_secs(10);
    assert_eq!(*breaker.evaluate_state(now), CircuitState::HalfOpen);

    // Trip 2: 10s * 2^1 = 20s
    let st = breaker.record_failure(now, &config);
    assert_eq!(
        *st,
        CircuitState::Open {
            tripped_at: now,
            cooldown: Duration::from_secs(20),
            failure_count: 1,
        }
    );

    // Transition to HalfOpen after 20s
    now += Duration::from_secs(20);
    assert_eq!(*breaker.evaluate_state(now), CircuitState::HalfOpen);

    // Trip 3: 10s * 2^2 = 40s
    let st = breaker.record_failure(now, &config);
    assert_eq!(
        *st,
        CircuitState::Open {
            tripped_at: now,
            cooldown: Duration::from_secs(40),
            failure_count: 1,
        }
    );

    // Transition to HalfOpen after 40s
    now += Duration::from_secs(40);
    assert_eq!(*breaker.evaluate_state(now), CircuitState::HalfOpen);

    // Trip 4: 10s * 2^3 = 80s -> clamped to max_cooldown (50s)
    let st = breaker.record_failure(now, &config);
    assert_eq!(
        *st,
        CircuitState::Open {
            tripped_at: now,
            cooldown: Duration::from_secs(50),
            failure_count: 1,
        }
    );
}
