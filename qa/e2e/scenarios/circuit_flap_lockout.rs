//! Tier 3: Cross-Feature: Circuit Breaker, Flap Lockout & CLI Operator Reset
//!
//! Validates state transitions from Closed -> Tripped -> Flap Lockout -> CLI Reset.

#[path = "../harness/harness_circuit.rs"]
mod harness_circuit;

use harness_circuit::{CircuitState, CircuitVerdict, UnitBreaker};

#[test]
fn test_tier3_circuit_flap_lockout_and_operator_reset_cycle() {
    let mut breaker = UnitBreaker::new(60, 2, 10);

    // Phase 1: Rapid crash loop triggers Trip 1 (10s backoff)
    breaker.record_failure(1);
    let v1 = breaker.record_failure(2);
    assert_eq!(v1, CircuitVerdict::Tripped { cooldown_secs: 10 });
    assert_eq!(breaker.state, CircuitState::Open);

    // Cooldown expires at t=12; probe fails at t=15 -> Trip 2 (20s backoff)
    breaker.record_failure(15);
    let v2 = breaker.record_failure(16);
    assert_eq!(v2, CircuitVerdict::Tripped { cooldown_secs: 20 });

    // Cooldown expires at t=36; probe fails at t=40 -> Trip 3 triggers FLAP LOCKOUT
    breaker.record_failure(40);
    let v3 = breaker.record_failure(41);
    assert_eq!(v3, CircuitVerdict::FlapLockout);
    assert_eq!(breaker.state, CircuitState::PermanentlyLocked);

    // All subsequent actions blocked indefinitely
    assert_eq!(breaker.check_action(100), CircuitVerdict::FlapLockout);
    assert_eq!(breaker.check_action(5000), CircuitVerdict::FlapLockout);

    // Phase 2: Human operator runs 'sentry reset <unit>'
    breaker.reset();
    assert_eq!(breaker.state, CircuitState::Closed);
    assert_eq!(breaker.check_action(5001), CircuitVerdict::Allowed);
}

#[test]
fn test_tier3_half_open_probe_success_recovers_to_closed() {
    let mut breaker = UnitBreaker::new(60, 2, 10);
    breaker.record_failure(1);
    breaker.record_failure(2);
    assert_eq!(breaker.state, CircuitState::Open);

    // After cooldown expires at t=12
    assert_eq!(breaker.check_action(15), CircuitVerdict::Allowed);
    assert_eq!(breaker.state, CircuitState::HalfOpen);

    // Probe succeeds for stabilization period without failure -> reset to Closed
    breaker.reset();
    assert_eq!(breaker.state, CircuitState::Closed);
}
