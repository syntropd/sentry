//! Tier 2: R4 Circuit Breaker Boundary & Corner Case Tests
//!
//! Tests edge conditions: 0-second window, 1-failure limit, max cooldown clamp.

#[path = "../../e2e/harness/harness_circuit.rs"]
mod harness_circuit;

use harness_circuit::{CircuitVerdict, UnitBreaker};

#[test]
fn test_r4_boundary_zero_second_sliding_window() {
    let mut breaker = UnitBreaker::new(0, 3, 30);
    // At window=0, failures in same second accumulate
    breaker.record_failure(10);
    breaker.record_failure(10);
    let verdict = breaker.record_failure(10);
    assert!(matches!(verdict, CircuitVerdict::Tripped { .. }));
}

#[test]
fn test_r4_boundary_single_failure_budget() {
    let mut breaker = UnitBreaker::new(60, 1, 30);
    // Failure threshold = 1 trips on very first failure
    let verdict = breaker.record_failure(5);
    assert_eq!(verdict, CircuitVerdict::Tripped { cooldown_secs: 30 });
}

#[test]
fn test_r4_boundary_exponential_backoff_max_ceiling() {
    let mut breaker = UnitBreaker::new(60, 1, 30);
    breaker.max_cooldown_secs = 120; // Set ceiling to 120s

    // Trip 1: 30s
    let v1 = breaker.record_failure(1);
    assert_eq!(v1, CircuitVerdict::Tripped { cooldown_secs: 30 });

    // Trip 2: 60s
    let v2 = breaker.record_failure(35);
    assert_eq!(v2, CircuitVerdict::Tripped { cooldown_secs: 60 });

    // Trip 3: 120s (hitting max ceiling instead of 120)
    let v3 = breaker.record_failure(100);
    assert_eq!(v3, CircuitVerdict::FlapLockout); // hits flap lockout threshold of 3
}

#[test]
fn test_r4_boundary_timestamps_moving_backwards() {
    let mut breaker = UnitBreaker::new(60, 3, 30);
    breaker.record_failure(100);
    // Monotonic clock skew backwards to 50
    let verdict = breaker.record_failure(50);
    assert_eq!(verdict, CircuitVerdict::Allowed);
}

#[test]
fn test_r4_boundary_exact_lockout_expiration_second() {
    let mut breaker = UnitBreaker::new(60, 1, 30);
    breaker.record_failure(10); // Tripped until 10 + 30 = 40

    // At t=39 -> still rejected (1 sec remaining)
    assert_eq!(
        breaker.check_action(39),
        CircuitVerdict::Rejected { remaining_secs: 1 }
    );

    // At t=40 -> exact boundary expiration -> Allowed (HalfOpen)
    assert_eq!(breaker.check_action(40), CircuitVerdict::Allowed);

    // At t=41 -> Allowed
    assert_eq!(breaker.check_action(41), CircuitVerdict::Allowed);
}
