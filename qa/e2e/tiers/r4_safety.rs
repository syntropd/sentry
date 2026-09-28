//! Tier 1: R4 Zero-Trust Safety Engine & Circuit Breaker Tests
//!
//! Validates closed enum action gating, sliding window trip, backoff, and flap lockout.

use super::harness_circuit::{CircuitVerdict, UnitBreaker};
use super::harness_models::{RemediationAction, Severity};
use super::harness_policy::{PolicyEngine, PolicyVerdict};

#[test]
fn test_r4_advisory_llm_shell_execution_rejection() {
    let raw_llm_action = "rm -rf /tmp && systemctl restart foo";
    // Serde deserialization of RemediationAction must fail on arbitrary strings
    let parsed: Result<RemediationAction, _> =
        serde_json::from_str(&format!("\"{}\"", raw_llm_action));
    assert!(parsed.is_err(), "Arbitrary shell action was not rejected by closed enum");
}

#[test]
fn test_r4_sliding_window_rate_limiting() {
    let mut breaker = UnitBreaker::new(60, 3, 30);
    // 1st failure at t=10s
    assert_eq!(breaker.record_failure(10), CircuitVerdict::Allowed);
    // 2nd failure at t=20s
    assert_eq!(breaker.record_failure(20), CircuitVerdict::Allowed);
    // 3rd failure at t=30s -> Trips breaker
    match breaker.record_failure(30) {
        CircuitVerdict::Tripped { cooldown_secs } => {
            assert_eq!(cooldown_secs, 30);
        }
        other => panic!("Expected Tripped, got {:?}", other),
    }
}

#[test]
fn test_r4_circuit_breaker_exponential_backoff() {
    let mut breaker = UnitBreaker::new(60, 3, 30);

    // Trip 1
    breaker.record_failure(1);
    breaker.record_failure(2);
    match breaker.record_failure(3) {
        CircuitVerdict::Tripped { cooldown_secs } => assert_eq!(cooldown_secs, 30),
        _ => panic!("Expected trip 1 with 30s cooldown"),
    }

    // Cooldown expires at t=33s; simulate failure burst in next window
    breaker.record_failure(40);
    breaker.record_failure(41);
    match breaker.record_failure(42) {
        CircuitVerdict::Tripped { cooldown_secs } => assert_eq!(cooldown_secs, 60),
        _ => panic!("Expected trip 2 with 60s cooldown"),
    }
}

#[test]
fn test_r4_flap_lockout_after_3_trips() {
    let mut breaker = UnitBreaker::new(60, 2, 10);

    // Trip 1
    breaker.record_failure(10);
    assert!(matches!(breaker.record_failure(11), CircuitVerdict::Tripped { .. }));

    // Trip 2
    breaker.record_failure(30);
    assert!(matches!(breaker.record_failure(31), CircuitVerdict::Tripped { .. }));

    // Trip 3 -> Triggers Flap Lockout
    breaker.record_failure(60);
    assert_eq!(breaker.record_failure(61), CircuitVerdict::FlapLockout);
}

#[test]
fn test_r4_policy_blacklist_protects_critical_units() {
    let policy = PolicyEngine::default();
    let verdict = policy.evaluate(
        "dbus.service",
        RemediationAction::Restart,
        Severity::High,
    );
    assert_eq!(
        verdict,
        PolicyVerdict::DisallowedUnit("dbus.service".to_string())
    );

    let journal_verdict = policy.evaluate(
        "systemd-journald.service",
        RemediationAction::Restart,
        Severity::High,
    );
    assert_eq!(
        journal_verdict,
        PolicyVerdict::DisallowedUnit("systemd-journald.service".to_string())
    );
}
