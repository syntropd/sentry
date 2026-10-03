//! Adversarial stress test: Flapping service engagement and persistent lockout.
//!
//! Validates:
//! 1. Tripping a service 3 times within 15 minutes triggers PERMANENTLY_LOCKED.
//! 2. `allows_remediation()` remains false even after extensive cooldown periods (hours/days).
//! 3. Operator `reset()` restores the breaker to CLOSED and allows remediation.
//! 4. Clean expiry of trips occurring outside the 15-minute rolling flap window.

use sentry_safety::circuit::{CircuitConfig, CircuitState, UnitBreaker};
use std::time::{Duration, Instant};

#[test]
fn test_flapping_lockout_three_trips_within_15_minutes() {
    let mut now = Instant::now();
    let config = CircuitConfig::default();
    let mut breaker = UnitBreaker::new("database-primary.service", now);

    assert_eq!(*breaker.evaluate_state(now), CircuitState::Closed);
    assert!(breaker.evaluate_state(now).allows_remediation());

    // --- Trip 1: 3 failures in sliding window ---
    for _ in 0..config.max_failures {
        now += Duration::from_millis(50);
        breaker.record_failure(now, &config);
    }
    let st = breaker.evaluate_state(now);
    assert!(matches!(st, CircuitState::Open { cooldown, failure_count: 3, .. } if *cooldown == Duration::from_secs(30)));
    assert!(!st.allows_remediation());

    // Advance 31s: transition to HalfOpen
    now += Duration::from_secs(31);
    assert_eq!(*breaker.evaluate_state(now), CircuitState::HalfOpen);
    assert!(breaker.evaluate_state(now).allows_remediation());

    // --- Trip 2: Failure during HalfOpen trial ---
    now += Duration::from_millis(100);
    let st = breaker.record_failure(now, &config);
    assert!(matches!(st, CircuitState::Open { cooldown, failure_count: 1, .. } if *cooldown == Duration::from_secs(60)));
    assert!(!st.allows_remediation());

    // Advance 61s: transition to HalfOpen
    now += Duration::from_secs(61);
    assert_eq!(*breaker.evaluate_state(now), CircuitState::HalfOpen);

    // --- Trip 3: Failure during second HalfOpen trial (Total elapsed: ~93s << 900s) ---
    now += Duration::from_millis(100);
    let st = breaker.record_failure(now, &config);
    assert!(
        matches!(st, CircuitState::PermanentlyLocked { flap_trips: 3, .. }),
        "Expected PERMANENTLY_LOCKED with 3 flap trips, got {:?}",
        st
    );
    assert_eq!(st.label(), "PERMANENTLY_LOCKED");
    assert!(!st.allows_remediation());

    // Verify snapshot reflects permanent lockout
    let snap = breaker.snapshot(now);
    assert_eq!(snap.state, "PERMANENTLY_LOCKED");
    assert!(snap.permanently_locked);
    assert_eq!(snap.cooldown_remaining_secs, 0);

    // Assert allows_remediation() is FALSE even after extensive time (1 hour, 24 hours, 7 days)
    for hours in [1, 24, 168] {
        now += Duration::from_secs(hours * 3600);
        let evaluated = breaker.evaluate_state(now);
        assert!(
            matches!(evaluated, CircuitState::PermanentlyLocked { .. }),
            "Lockout decayed after {} hours!",
            hours
        );
        assert!(
            !evaluated.allows_remediation(),
            "allows_remediation() must be false after {} hours",
            hours
        );
    }

    // Additional failures while locked must not alter state or panic
    let st = breaker.record_failure(now, &config);
    assert!(matches!(st, CircuitState::PermanentlyLocked { .. }));

    // Verify operator reset() restores it to CLOSED
    breaker.reset(now);
    let evaluated = breaker.evaluate_state(now);
    assert_eq!(*evaluated, CircuitState::Closed);
    assert!(evaluated.allows_remediation());

    let snap = breaker.snapshot(now);
    assert_eq!(snap.state, "CLOSED");
    assert!(!snap.permanently_locked);
    assert_eq!(snap.recent_failures, 0);
}

#[test]
fn test_flapping_lockout_across_temporary_recovery_to_closed() {
    let mut now = Instant::now();
    let config = CircuitConfig::default();
    let mut breaker = UnitBreaker::new("payment-gateway.service", now);

    // Trip 1
    for _ in 0..config.max_failures {
        now += Duration::from_millis(10);
        breaker.record_failure(now, &config);
    }
    // HalfOpen -> Success -> Closed
    now += Duration::from_secs(31);
    breaker.evaluate_state(now);
    breaker.record_success(now);
    assert_eq!(*breaker.evaluate_state(now), CircuitState::Closed);

    // Trip 2
    for _ in 0..config.max_failures {
        now += Duration::from_millis(10);
        breaker.record_failure(now, &config);
    }
    // HalfOpen -> Success -> Closed
    now += Duration::from_secs(61);
    breaker.evaluate_state(now);
    breaker.record_success(now);
    assert_eq!(*breaker.evaluate_state(now), CircuitState::Closed);

    // Trip 3 (within 15 minutes of Trip 1)
    for _ in 0..config.max_failures {
        now += Duration::from_millis(10);
        breaker.record_failure(now, &config);
    }
    let st = breaker.evaluate_state(now);
    assert!(
        matches!(st, CircuitState::PermanentlyLocked { .. }),
        "Expected PERMANENTLY_LOCKED despite temporary recoveries, got {:?}",
        st
    );
    assert!(!st.allows_remediation());
}

#[test]
fn test_flapping_window_expiry_prevents_unwarranted_lockout() {
    let mut now = Instant::now();
    let config = CircuitConfig::default();
    let mut breaker = UnitBreaker::new("batch-runner.service", now);

    // Trip 1 at t=0
    for _ in 0..config.max_failures {
        breaker.record_failure(now, &config);
    }
    now += Duration::from_secs(31);
    breaker.evaluate_state(now);

    // Trip 2 at t=60s
    now += Duration::from_secs(29);
    breaker.record_failure(now, &config);
    now += Duration::from_secs(61);
    breaker.evaluate_state(now);

    // Advance past flap_window (900s) from Trip 1: advance by 850s (total elapsed = 971s > 900s)
    now += Duration::from_secs(850);

    // Trip 3 occurs at t=971s (Trip 1 is now expired, only Trip 2 is within 900s)
    let st = breaker.record_failure(now, &config);
    assert!(
        matches!(st, CircuitState::Open { .. }),
        "Trip 1 should have expired from flap window, expected OPEN but got {:?}",
        st
    );
    assert!(!matches!(st, CircuitState::PermanentlyLocked { .. }));
}

#[test]
fn test_failure_burst_while_open_should_not_lockout_before_cooldown() {
    let mut now = Instant::now();
    let config = CircuitConfig::default(); // max_failures=3, flap_threshold=3, cooldown=30s
    let mut breaker = UnitBreaker::new("burst-crash.service", now);

    // 5 rapid failures in 500ms
    for _ in 0..5 {
        now += Duration::from_millis(100);
        breaker.record_failure(now, &config);
    }

    let st = breaker.evaluate_state(now);
    assert!(
        matches!(st, CircuitState::Open { .. }),
        "5 failures in 500ms should result in OPEN, but got {:?}",
        st
    );
}
