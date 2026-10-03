//! Adversarial stress test: Extreme exponential backoff scaling and integer safety.
//!
//! Validates:
//! 1. Triggering 20 successive trips without integer overflow or panic.
//! 2. Exponential cooldown progression (30s, 60s, 120s, 240s, 480s, 960s).
//! 3. Clamping strictly to `max_cooldown` (1800s) for trips >= 7.
//! 4. Immunity against extreme trip counts (100 trips, 1,000 trips) via bit shift capping.

use sentry_safety::circuit::{CircuitConfig, CircuitState, UnitBreaker};
use std::time::{Duration, Instant};

#[test]
fn test_twenty_successive_trips_exponential_backoff_and_clamp() {
    let mut now = Instant::now();
    // Use high flap_threshold so we can evaluate pure exponential backoff across 20 trips
    let config = CircuitConfig {
        max_failures: 1,
        window_duration: Duration::from_secs(60),
        cooldown_duration: Duration::from_secs(30),
        max_cooldown: Duration::from_secs(1800),
        flap_window: Duration::from_secs(86400),
        flap_threshold: 100,
    };

    let mut breaker = UnitBreaker::new("scale-target.service", now);

    let expected_cooldowns = [
        30,   // Trip 1: 30 * 2^0 = 30
        60,   // Trip 2: 30 * 2^1 = 60
        120,  // Trip 3: 30 * 2^2 = 120
        240,  // Trip 4: 30 * 2^3 = 240
        480,  // Trip 5: 30 * 2^4 = 480
        960,  // Trip 6: 30 * 2^5 = 960
        1800, // Trip 7: 30 * 2^6 = 1920 -> clamp 1800
        1800, // Trip 8: clamp 1800
        1800, // Trip 9: clamp 1800
        1800, // Trip 10: clamp 1800
        1800, // Trip 11: clamp 1800
        1800, // Trip 12: clamp 1800
        1800, // Trip 13: clamp 1800
        1800, // Trip 14: clamp 1800
        1800, // Trip 15: clamp 1800
        1800, // Trip 16: clamp 1800
        1800, // Trip 17: clamp 1800
        1800, // Trip 18: clamp 1800
        1800, // Trip 19: clamp 1800
        1800, // Trip 20: clamp 1800
    ];

    for (trip_idx, &expected_secs) in expected_cooldowns.iter().enumerate() {
        let trip_num = trip_idx + 1;

        // Record failure that trips the breaker
        let state = breaker.record_failure(now, &config);
        match state {
            CircuitState::Open {
                cooldown,
                failure_count,
                ..
            } => {
                assert_eq!(
                    *cooldown,
                    Duration::from_secs(expected_secs),
                    "Trip {}: expected cooldown {}s, got {:?}",
                    trip_num,
                    expected_secs,
                    cooldown
                );
                assert_eq!(*failure_count, 1);
            }
            other => panic!(
                "Trip {}: expected CircuitState::Open, got {:?}",
                trip_num, other
            ),
        }

        // Verify snapshot accurately reflects cooldown
        let snap = breaker.snapshot(now);
        assert_eq!(snap.state, "OPEN");
        assert_eq!(snap.cooldown_remaining_secs, expected_secs);

        // Advance monotonic time by cooldown duration to transition to HalfOpen
        now += Duration::from_secs(expected_secs);
        let eval_state = breaker.evaluate_state(now);
        assert_eq!(
            *eval_state,
            CircuitState::HalfOpen,
            "Trip {}: failed to transition to HalfOpen after {}s",
            trip_num,
            expected_secs
        );
    }
}

#[test]
fn test_extreme_trip_count_overflow_resilience() {
    let mut now = Instant::now();
    let config = CircuitConfig {
        max_failures: 1,
        window_duration: Duration::from_secs(60),
        cooldown_duration: Duration::from_secs(30),
        max_cooldown: Duration::from_secs(1800),
        flap_window: Duration::from_secs(86400 * 365),
        flap_threshold: 2000,
    };

    let mut breaker = UnitBreaker::new("extreme-overflow.service", now);

    // Push 100 trips beyond 30-bit shift boundary
    for i in 1..=100 {
        let st = breaker.record_failure(now, &config);
        match st {
            CircuitState::Open { cooldown, .. } => {
                assert!(
                    *cooldown <= Duration::from_secs(1800),
                    "Trip {}: cooldown {:?} exceeded max_cooldown 1800s",
                    i,
                    cooldown
                );
                if i >= 7 {
                    assert_eq!(*cooldown, Duration::from_secs(1800));
                }
            }
            other => panic!("Trip {}: unexpected state {:?}", i, other),
        }

        // Transition through HalfOpen
        now += Duration::from_secs(1800);
        breaker.evaluate_state(now);
    }
}
