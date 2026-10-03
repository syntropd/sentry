//! Adversarial stress test: Clock skew and monotonic time edges.
//!
//! Validates:
//! 1. Rapid successive failures within microseconds do not panic or miss trip transitions.
//! 2. Backward monotonic time steps (clock skew / NTP jumps) do not panic or cause underflow.
//! 3. System boot / window underflow behavior when window duration exceeds system uptime.

use sentry_safety::circuit::{CircuitConfig, CircuitState, UnitBreaker};
use std::time::{Duration, Instant};

#[test]
fn test_rapid_failures_within_microseconds() {
    let base = Instant::now();
    let config = CircuitConfig {
        max_failures: 3,
        window_duration: Duration::from_secs(60),
        cooldown_duration: Duration::from_secs(30),
        max_cooldown: Duration::from_secs(1800),
        flap_window: Duration::from_secs(900),
        flap_threshold: 3,
    };

    let mut breaker = UnitBreaker::new("rapid-burst.service", base);

    // Three rapid failures occurring 1 microsecond apart
    let t1 = base;
    let t2 = base + Duration::from_micros(1);
    let t3 = base + Duration::from_micros(2);

    assert_eq!(*breaker.record_failure(t1, &config), CircuitState::Closed);
    assert_eq!(*breaker.record_failure(t2, &config), CircuitState::Closed);
    let st = breaker.record_failure(t3, &config);

    assert!(
        matches!(st, CircuitState::Open { failure_count: 3, .. }),
        "Microsecond burst failed to trip OPEN: {:?}",
        st
    );
    assert!(!st.allows_remediation());
}

#[test]
fn test_rapid_trips_within_microseconds_locks_out() {
    let base = Instant::now();
    let config = CircuitConfig {
        max_failures: 1, // Trip on every failure
        window_duration: Duration::from_secs(60),
        cooldown_duration: Duration::ZERO,
        max_cooldown: Duration::from_secs(1800),
        flap_window: Duration::from_secs(900),
        flap_threshold: 3,
    };

    let mut breaker = UnitBreaker::new("rapid-tripper.service", base);

    // Trip 1 at base
    breaker.record_failure(base, &config);
    // Trip 2 at base + 1 microsecond
    let t2 = base + Duration::from_micros(1);
    breaker.record_failure(t2, &config);
    // Trip 3 at base + 2 microseconds
    let t3 = base + Duration::from_micros(2);
    let st = breaker.record_failure(t3, &config);

    assert!(
        matches!(st, CircuitState::PermanentlyLocked { flap_trips: 3, .. }),
        "Rapid microsecond trips failed to lock out: {:?}",
        st
    );
    assert!(!st.allows_remediation());
}

#[test]
fn test_clock_skew_backwards_timestamps() {
    let now = Instant::now();
    let config = CircuitConfig::default();
    let mut breaker = UnitBreaker::new("skewed.service", now);

    // Initial failure at t=100s relative
    breaker.record_failure(now, &config);

    // Skew backwards: failure reported 50s earlier
    let earlier = now.checked_sub(Duration::from_secs(50)).unwrap_or(now);
    let st = breaker.record_failure(earlier, &config);
    assert_eq!(*st, CircuitState::Closed);

    // Snapshot with backwards time must not panic on duration_since
    let snap = breaker.snapshot(earlier);
    assert_eq!(snap.state, "CLOSED");

    // Evaluation with backwards time must not panic
    let eval = breaker.evaluate_state(earlier);
    assert_eq!(*eval, CircuitState::Closed);
}

#[test]
fn test_boot_time_checked_sub_window_underflow() {
    let now = Instant::now();
    // Emulate system boot scenario or long window where window exceeds system uptime
    // System uptime is ~15 hours; window of 100,000s (>27 hours) forces checked_sub to return None.
    let oversized_window = Duration::from_secs(100_000);
    let config = CircuitConfig {
        max_failures: 3,
        window_duration: oversized_window,
        cooldown_duration: Duration::from_secs(30),
        max_cooldown: Duration::from_secs(1800),
        flap_window: oversized_window,
        flap_threshold: 3,
    };

    let mut breaker = UnitBreaker::new("early-boot.service", now);

    let t1 = now;
    let t2 = now + Duration::from_secs(1);
    let t3 = now + Duration::from_secs(2);

    breaker.record_failure(t1, &config);
    breaker.record_failure(t2, &config);
    let st = breaker.record_failure(t3, &config);

    // An oversized sliding window should retain recent failures occurring within seconds.
    // If checked_sub underflow sets cutoff = now, failures at t1 and t2 are prematurely evicted!
    assert!(
        matches!(st, CircuitState::Open { failure_count: 3, .. }),
        "Failure accumulation broken when window_duration exceeds system uptime: {:?}",
        st
    );
}

#[test]
fn test_duration_max_checked_sub_edge() {
    let now = Instant::now();
    // When flap_window is Duration::MAX, checked_sub returns None.
    // unwrap_or(now) sets flap_cutoff = now, which evicts all prior trips!
    let config = CircuitConfig {
        max_failures: 1,
        window_duration: Duration::from_secs(60),
        cooldown_duration: Duration::from_secs(1),
        max_cooldown: Duration::from_secs(1800),
        flap_window: Duration::MAX,
        flap_threshold: 3,
    };

    let mut breaker = UnitBreaker::new("max-flap-duration.service", now);
    let t1 = now;
    let t2 = now + Duration::from_secs(2);
    let t3 = now + Duration::from_secs(4);

    breaker.record_failure(t1, &config);
    breaker.record_failure(t2, &config);
    let st = breaker.record_failure(t3, &config);

    assert!(
        matches!(st, CircuitState::PermanentlyLocked { .. }),
        "Flap lockout failed when flap_window is Duration::MAX: unwrap_or(now) evicted past trips! Got: {:?}",
        st
    );
}

#[test]
fn test_window_duration_max_checked_sub_edge() {
    let now = Instant::now();
    // When window_duration is Duration::MAX, checked_sub returns None.
    // unwrap_or(now) sets cutoff = now, which evicts all prior failure timestamps!
    let config = CircuitConfig {
        max_failures: 3,
        window_duration: Duration::MAX,
        cooldown_duration: Duration::from_secs(30),
        max_cooldown: Duration::from_secs(1800),
        flap_window: Duration::from_secs(900),
        flap_threshold: 3,
    };

    let mut breaker = UnitBreaker::new("max-window-duration.service", now);
    let t1 = now;
    let t2 = now + Duration::from_secs(1);
    let t3 = now + Duration::from_secs(2);

    breaker.record_failure(t1, &config);
    breaker.record_failure(t2, &config);
    let st = breaker.record_failure(t3, &config);

    assert!(
        matches!(st, CircuitState::Open { failure_count: 3, .. }),
        "Circuit failed to trip OPEN when window_duration is Duration::MAX: unwrap_or(now) evicted past failures! Got: {:?}",
        st
    );
}
