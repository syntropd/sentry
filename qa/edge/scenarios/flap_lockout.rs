//! Tier 4: Real-World Scenario: Flap Lockout Enforcement
//!
//! Simulates flapper_service: short uptime (<50ms) followed by repeated exit(1).
//! Enforces:
//! - Sliding window rate tracking
//! - Exponential backoff progression
//! - Permanent lockout on flap frequency threshold
//! - Rejection of all automatic restarts

#[path = "../../e2e/harness/harness_circuit.rs"]
mod harness_circuit;

use harness_circuit::{CircuitState, CircuitVerdict, UnitBreaker};

#[test]
fn test_tier4_flapper_service_lockout_enforcement() {
    let mut breaker = UnitBreaker::new(60, 3, 15);

    // Round 1: Service starts, fails 3 times in 30ms -> Trip 1 (15s backoff)
    breaker.record_failure(10);
    breaker.record_failure(10);
    let v1 = breaker.record_failure(10);
    assert_eq!(v1, CircuitVerdict::Tripped { cooldown_secs: 15 });
    assert_eq!(breaker.state, CircuitState::Open);

    // At t=20s, action is rejected
    assert_eq!(breaker.check_action(20), CircuitVerdict::Rejected { remaining_secs: 5 });

    // Round 2: Cooldown expires at t=25s; probe restarted, crashes 3 times -> Trip 2 (30s backoff)
    assert_eq!(breaker.check_action(26), CircuitVerdict::Allowed);
    breaker.record_failure(26);
    breaker.record_failure(26);
    let v2 = breaker.record_failure(26);
    assert_eq!(v2, CircuitVerdict::Tripped { cooldown_secs: 30 });

    // Round 3: Cooldown expires at t=56s; probe restarted, crashes 3 times -> Trip 3 (FLAP LOCKOUT)
    assert_eq!(breaker.check_action(57), CircuitVerdict::Allowed);
    breaker.record_failure(57);
    breaker.record_failure(57);
    let v3 = breaker.record_failure(57);
    assert_eq!(v3, CircuitVerdict::FlapLockout);
    assert_eq!(breaker.state, CircuitState::PermanentlyLocked);

    // Permanent Lockout: Automatic restart attempts are completely blocked
    for t in (100..10_000).step_by(100) {
        assert_eq!(breaker.check_action(t), CircuitVerdict::FlapLockout);
    }
}
