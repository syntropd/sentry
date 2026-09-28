//! Unit tests for LatencyTracker EMA, p95 calculation, and timeout clamping.

use sentry_diagnostic::circuit::{AdaptiveTimeoutConfig, LatencyTracker};
use std::time::Duration;

#[test]
fn test_tracker_initial_empty() {
    let tracker = LatencyTracker::new();
    assert!(tracker.is_empty());
    assert_eq!(tracker.count(), 0);
    assert_eq!(tracker.ema_ms(), None);
    assert_eq!(tracker.p95_ms(), 0);

    let config = AdaptiveTimeoutConfig::default();
    let timeout = tracker.calculate_timeout(&config);
    assert_eq!(timeout, config.initial_timeout);
}

#[test]
fn test_tracker_ema_and_p95() {
    let mut tracker = LatencyTracker::new();
    let alpha = 0.20;

    let samples = [100u32, 105, 110, 108, 500];
    for &sample in &samples {
        tracker.record_ms(sample, alpha);
    }

    assert_eq!(tracker.count(), 5);

    // Verify EMA mathematical progression
    let ema = tracker.ema_ms().expect("EMA should be present");
    assert!(
        (ema - 183.072).abs() < 1e-3,
        "Expected EMA ~183.072, got {ema}"
    );

    // Verify p95 correctly identifies the tail outlier (500ms)
    assert_eq!(tracker.p95_ms(), 500);
}

#[test]
fn test_tracker_timeout_clamping() {
    let config = AdaptiveTimeoutConfig::default()
        .with_min_timeout(Duration::from_millis(1500))
        .with_max_timeout(Duration::from_millis(15000))
        .with_initial_timeout(Duration::from_millis(5000));

    // Case 1: Low latency (< 1.5s) clamped to min_timeout
    let mut low_tracker = LatencyTracker::new();
    for _ in 0..10 {
        low_tracker.record_ms(50, 0.20);
    }
    let low_timeout = low_tracker.calculate_timeout(&config);
    assert_eq!(low_timeout, Duration::from_millis(1500));

    // Case 2: Moderate latency within [1.5s, 15.0s]
    let mut mod_tracker = LatencyTracker::new();
    for _ in 0..10 {
        mod_tracker.record_ms(2000, 0.20);
    }
    // EMA = 2000, p95 = 2000 -> max(2000*1.5, 2000*1.25) = 3000ms
    let mod_timeout = mod_tracker.calculate_timeout(&config);
    assert_eq!(mod_timeout, Duration::from_millis(3000));

    // Case 3: High latency (> 15.0s) clamped to max_timeout
    let mut high_tracker = LatencyTracker::new();
    for _ in 0..10 {
        high_tracker.record_ms(20000, 0.20);
    }
    let high_timeout = high_tracker.calculate_timeout(&config);
    assert_eq!(high_timeout, Duration::from_millis(15000));
}

#[test]
fn test_tracker_ring_buffer_wrap_around() {
    let mut tracker = LatencyTracker::new();
    for i in 1..=80 {
        tracker.record_ms(i * 10, 0.20);
    }

    // Bounded capacity invariant: count stays capped at 64
    assert_eq!(tracker.count(), 64);
    assert_eq!(tracker.samples_capacity(), 64);

    // p95 should reflect the top percentiles of the active 64 samples
    assert!(tracker.p95_ms() > 700);
}

#[test]
fn test_tracker_reset() {
    let mut tracker = LatencyTracker::new();
    tracker.record_ms(500, 0.20);
    assert_eq!(tracker.count(), 1);

    tracker.reset();
    assert_eq!(tracker.count(), 0);
    assert!(tracker.is_empty());
    assert_eq!(tracker.ema_ms(), None);
}

trait SamplesCap {
    fn samples_capacity(&self) -> usize;
}

impl SamplesCap for LatencyTracker {
    fn samples_capacity(&self) -> usize {
        sentry_diagnostic::circuit::latency_tracker::LATENCY_SAMPLE_CAPACITY
    }
}
