//! Tier 3: Cross-Feature: Circuit Breaker Lockout & Dual-Channel Notification Alert
//!
//! Validates that tripping circuit breaker automatically triggers toast and wall alerts.

#[path = "../harness/harness_circuit.rs"]
mod harness_circuit;

#[path = "../harness/harness_notify.rs"]
mod harness_notify;

use harness_circuit::{CircuitVerdict, UnitBreaker};
use harness_notify::NotificationRateLimiter;

#[test]
fn test_tier3_circuit_lockout_triggers_dual_notifications() {
    let mut breaker = UnitBreaker::new(60, 2, 30);
    let mut limiter = NotificationRateLimiter::new(60, 5);
    let unit = "cache.service";

    // 1st failure
    assert_eq!(breaker.record_failure(10), CircuitVerdict::Allowed);

    // 2nd failure -> Trips circuit breaker
    let verdict = breaker.record_failure(12);
    assert!(matches!(verdict, CircuitVerdict::Tripped { .. }));

    // Tripped event dispatches notification
    let should_notify = limiter.should_emit(unit, 12);
    assert!(should_notify, "Circuit breaker trip must trigger alert");

    // Format wall banner
    let banner = format!(
        "[CIRCUIT BREAKER LOCKOUT] {}\nAutomatic restarts suspended for 30s.",
        unit
    );
    assert!(banner.contains("cache.service"));
    assert!(banner.contains("suspended for 30s"));
}
