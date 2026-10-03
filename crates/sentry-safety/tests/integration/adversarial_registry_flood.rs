//! Adversarial stress test: Registry flood with 10,000 distinct service unit failures.
//!
//! Validates:
//! 1. Memory bounding to <= 512 units (MAX_TRACKED_UNITS).
//! 2. Zero-panic resilience during high-volume failure storms.
//! 3. Guaranteed preservation of OPEN and PERMANENTLY_LOCKED units during eviction.
//! 4. Memory footprint stays flat (< 15MB RSS).

use sentry_safety::circuit::{
    CircuitBreakerRegistry, CircuitConfig, CircuitState, MAX_TRACKED_UNITS,
};
use std::fs;
use std::time::{Duration, Instant};

fn get_rss_bytes() -> usize {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<usize>() {
                        return kb * 1024;
                    }
                }
            }
        }
    }
    0
}

#[test]
fn test_registry_flood_10k_units_bounds_and_never_evicts_locked_or_open() {
    let mut now = Instant::now();
    let config = CircuitConfig::default();
    let mut registry = CircuitBreakerRegistry::new(config.clone());

    let initial_rss = get_rss_bytes();

    // 1. Seed with 5 OPEN units (trip each with 3 failures)
    let open_units = vec![
        "open-svc-alpha.service",
        "open-svc-bravo.service",
        "open-svc-charlie.service",
        "open-svc-delta.service",
        "open-svc-echo.service",
    ];
    for unit in &open_units {
        for _ in 0..config.max_failures {
            now += Duration::from_millis(1);
            registry.record_failure(unit, now);
        }
        let st = registry.evaluate(unit, now);
        assert!(
            matches!(st, CircuitState::Open { .. }),
            "Expected unit {} to be OPEN, got {:?}",
            unit,
            st
        );
    }

    // 2. Seed with 5 PERMANENTLY_LOCKED units
    let locked_units = vec![
        "locked-svc-1.service",
        "locked-svc-2.service",
        "locked-svc-3.service",
        "locked-svc-4.service",
        "locked-svc-5.service",
    ];
    for unit in &locked_units {
        // Trip 1
        for _ in 0..config.max_failures {
            now += Duration::from_millis(1);
            registry.record_failure(unit, now);
        }
        // Advance cooldown & fail in HalfOpen -> Trip 2
        now += Duration::from_secs(35);
        registry.evaluate(unit, now);
        registry.record_failure(unit, now);

        // Advance cooldown & fail in HalfOpen -> Trip 3 (Locked)
        now += Duration::from_secs(65);
        registry.evaluate(unit, now);
        let st = registry.record_failure(unit, now);
        assert!(
            matches!(st, CircuitState::PermanentlyLocked { .. }),
            "Expected unit {} to be PERMANENTLY_LOCKED, got {:?}",
            unit,
            st
        );
    }

    assert_eq!(registry.tracked_units_count(), 10);

    // 3. Flood registry with 10,000 distinct ephemeral service unit failures
    for i in 0..10_000 {
        let unit_name = format!("flood-ephemeral-{i:05}.service");
        now += Duration::from_millis(5);
        let state = registry.record_failure(&unit_name, now);
        assert_eq!(
            state,
            CircuitState::Closed,
            "Ephemeral unit should be Closed after 1 failure"
        );
    }

    // 4. Assert strict bounding to <= 512 units
    let count = registry.tracked_units_count();
    assert!(
        count <= MAX_TRACKED_UNITS,
        "Registry count {} exceeded MAX_TRACKED_UNITS {}",
        count,
        MAX_TRACKED_UNITS
    );
    assert_eq!(
        count, MAX_TRACKED_UNITS,
        "Registry should be capped at exactly MAX_TRACKED_UNITS"
    );

    // 5. Assert that none of the OPEN units were evicted
    for unit in &open_units {
        let snap = registry.get_snapshot(unit, now);
        assert!(
            snap.is_some(),
            "OPEN unit {} was improperly evicted during 10k flood",
            unit
        );
    }

    // 6. Assert that none of the PERMANENTLY_LOCKED units were evicted
    for unit in &locked_units {
        let snap = registry.get_snapshot(unit, now);
        assert!(
            snap.is_some(),
            "PERMANENTLY_LOCKED unit {} was improperly evicted during 10k flood",
            unit
        );
        let snap = snap.unwrap();
        assert_eq!(
            snap.state, "PERMANENTLY_LOCKED",
            "Unit {} state altered from locked",
            unit
        );
        assert!(snap.permanently_locked);
    }

    // 7. Verify memory RSS stayed flat (< 15MB)
    let final_rss = get_rss_bytes();
    let max_allowed_rss = 15 * 1024 * 1024; // 15MB
    println!(
        "Memory RSS: initial={} bytes, final={} bytes, max_allowed={} bytes",
        initial_rss, final_rss, max_allowed_rss
    );
    assert!(
        final_rss < max_allowed_rss,
        "RSS footprint {} bytes exceeded 15MB budget",
        final_rss
    );
}
