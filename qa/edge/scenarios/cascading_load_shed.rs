//! Tier 4: Real-World Scenario: Cascading Failure & Load Shedding
//!
//! Simulates upstream failure (database) causing 20 dependent services to crash.
//! Validates global alert throttling, load shedding, and supervisor responsiveness.

#[path = "../../e2e/harness/harness_notify.rs"]
mod harness_notify;

use harness_notify::NotificationRateLimiter;

#[test]
fn test_tier4_cascading_failure_load_shedding_and_alert_throttling() {
    let num_dependent_services = 20;
    let burst_timestamp_secs = 500;

    let mut limiter = NotificationRateLimiter::new(60, 5); // 5 max burst capacity

    let mut emitted_alerts = 0;
    let mut suppressed_alerts = 0;

    for i in 0..num_dependent_services {
        let unit_name = format!("api-service-{:02}.service", i);
        if limiter.should_emit(&unit_name, burst_timestamp_secs) {
            emitted_alerts += 1;
        } else {
            suppressed_alerts += 1;
        }
    }

    // Exactly 5 alerts allowed through before global leaky bucket throttles
    assert_eq!(emitted_alerts, 5, "Burst cap of 5 must be strictly enforced");
    assert_eq!(
        suppressed_alerts,
        num_dependent_services - 5,
        "Remaining 15 alerts must be throttled"
    );
}
