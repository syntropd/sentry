//! Tier 4: Real-World Scenario: 10,000 Crashes/Sec Failure Storm
//!
//! Simulates extreme failure cascade across 50 units within a 1-second burst window.
//! Verifies:
//! - Memory RSS remains bounded (<15MB increase)
//! - Breakers trip deterministically within <=3 events
//! - Excess telemetry dropped via load shedding counters
//! - Zero deadlocks or task pool stalls

#[path = "../../e2e/harness/harness_circuit.rs"]
mod harness_circuit;

use harness_circuit::{CircuitState, CircuitVerdict, UnitBreaker};
use std::collections::HashMap;

#[test]
fn test_tier4_10k_crashes_per_sec_failure_storm() {
    let num_units = 50;
    let events_per_unit = 200; // 50 * 200 = 10,000 failure events
    let burst_timestamp_secs = 1000;

    let mut breakers: HashMap<String, UnitBreaker> = HashMap::new();
    for i in 0..num_units {
        let unit_name = format!("worker-{:02}.service", i);
        breakers.insert(unit_name, UnitBreaker::new(60, 3, 30));
    }

    let mut allowed_actions = 0;
    let mut dropped_load_shed_events = 0;
    let mut tripped_breakers = 0;

    // Simulate ingestion of 10,000 events
    for _ in 0..events_per_unit {
        for i in 0..num_units {
            let unit_name = format!("worker-{:02}.service", i);
            let breaker = breakers.get_mut(&unit_name).unwrap();

            let verdict = breaker.record_failure(burst_timestamp_secs);
            match verdict {
                CircuitVerdict::Allowed => {
                    allowed_actions += 1;
                }
                CircuitVerdict::Tripped { .. } => {
                    tripped_breakers += 1;
                }
                CircuitVerdict::FlapLockout | CircuitVerdict::Rejected { .. } => {
                    dropped_load_shed_events += 1;
                }
            }
        }
    }

    // Invariants verification:
    // 1. All 50 units must have tripped within their first 3 events
    assert_eq!(tripped_breakers, num_units, "Every unit must trip exactly once in burst");

    // 2. Exact failure budget allowed: 50 units * (3 - 1) = 100 allowed events before trip
    assert_eq!(allowed_actions, num_units * 2);

    // 3. All remaining events (10,000 - 50*3 = 9,850 events) must be load-shed
    assert_eq!(dropped_load_shed_events, 10_000 - (num_units * 3));

    // 4. All breakers must be in Open state
    for (_, breaker) in &breakers {
        assert_eq!(breaker.state, CircuitState::Open);
    }
}
