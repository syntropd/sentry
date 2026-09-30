//! Bounded registry managing per-unit circuit breakers with LRU/idle eviction.

use crate::circuit::config::CircuitConfig;
use crate::circuit::state::{CircuitState, CircuitStateSnapshot};
use crate::circuit::unit_breaker::UnitBreaker;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Maximum permissible number of concurrently tracked units in memory (prevents heap DoS).
pub const MAX_TRACKED_UNITS: usize = 512;

/// Default idle TTL for eviction of inactive, healthy units (30 minutes).
pub const DEFAULT_IDLE_TTL: Duration = Duration::from_secs(1800);

/// Bounded registry of unit circuit breakers with zero heap leak vulnerability.
#[derive(Debug, Clone)]
pub struct CircuitBreakerRegistry {
    breakers: HashMap<String, UnitBreaker>,
    config: CircuitConfig,
    max_units: usize,
    idle_ttl: Duration,
}

impl CircuitBreakerRegistry {
    /// Constructs a registry with default limits.
    pub fn new(config: CircuitConfig) -> Self {
        Self {
            breakers: HashMap::with_capacity(64),
            config,
            max_units: MAX_TRACKED_UNITS,
            idle_ttl: DEFAULT_IDLE_TTL,
        }
    }

    /// Evaluates unit state at monotonic timestamp.
    pub fn evaluate(&mut self, unit: &str, now: Instant) -> CircuitState {
        if let Some(breaker) = self.breakers.get_mut(unit) {
            breaker.evaluate_state(now).clone()
        } else {
            CircuitState::Closed
        }
    }

    /// Records failure for a unit, initializing its breaker if not present.
    pub fn record_failure(&mut self, unit: &str, now: Instant) -> CircuitState {
        self.ensure_capacity_or_evict(now);

        let config = self.config.clone();
        let breaker = self
            .breakers
            .entry(unit.to_string())
            .or_insert_with(|| UnitBreaker::new(unit, now));

        breaker.record_failure(now, &config).clone()
    }

    /// Records success for a unit.
    pub fn record_success(&mut self, unit: &str, now: Instant) {
        if let Some(breaker) = self.breakers.get_mut(unit) {
            breaker.record_success(now);
        }
    }

    /// Immediately permanently locks a unit against restart loops.
    pub fn lock_unit(&mut self, unit: &str, now: Instant) {
        self.ensure_capacity_or_evict(now);
        let breaker = self
            .breakers
            .entry(unit.to_string())
            .or_insert_with(|| UnitBreaker::new(unit, now));
        breaker.lock_permanently(now);
    }

    /// Manually resets a unit breaker (clearing lockouts and failure counts).
    pub fn reset_unit(&mut self, unit: &str, now: Instant) -> bool {
        if let Some(breaker) = self.breakers.get_mut(unit) {
            breaker.reset(now);
            true
        } else {
            false
        }
    }

    /// Generates snapshots of all currently tracked units.
    pub fn list_snapshots(&mut self, now: Instant) -> HashMap<String, CircuitStateSnapshot> {
        let mut map = HashMap::new();
        for (name, breaker) in &mut self.breakers {
            breaker.evaluate_state(now);
            map.insert(name.clone(), breaker.snapshot(now));
        }
        map
    }

    /// Retrieves snapshot for a specific unit if tracked.
    pub fn get_snapshot(&mut self, unit: &str, now: Instant) -> Option<CircuitStateSnapshot> {
        self.breakers.get_mut(unit).map(|b| {
            b.evaluate_state(now);
            b.snapshot(now)
        })
    }

    /// Returns count of tracked units.
    pub fn tracked_units_count(&self) -> usize {
        self.breakers.len()
    }

    /// Force eviction of idle healthy units whose TTL has expired.
    pub fn evict_idle_breakers(&mut self, now: Instant) {
        let ttl = self.idle_ttl;
        self.breakers.retain(|_, b| !b.is_idle(now, ttl));
    }

    fn ensure_capacity_or_evict(&mut self, now: Instant) {
        if self.breakers.len() < self.max_units {
            return;
        }

        // Phase 1: Evict idle, healthy units whose TTL has expired
        let ttl = self.idle_ttl;
        self.breakers.retain(|_, b| !b.is_idle(now, ttl));

        if self.breakers.len() < self.max_units {
            return;
        }

        // Phase 2: LRU eviction among Closed units with least recent activity
        let oldest_closed = self
            .breakers
            .iter()
            .filter(|(_, b)| b.is_closed())
            .min_by_key(|(_, b)| b.last_activity())
            .map(|(k, _)| k.clone());

        if let Some(key) = oldest_closed {
            self.breakers.remove(&key);
        }
    }
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new(CircuitConfig::default())
    }
}

