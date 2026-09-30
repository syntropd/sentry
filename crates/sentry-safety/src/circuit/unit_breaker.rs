//! Sliding-window circuit breaker instance for an individual systemd unit.

use crate::circuit::config::CircuitConfig;
use crate::circuit::state::{CircuitState, CircuitStateSnapshot};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Per-unit circuit breaker and flap tracking state machine.
#[derive(Debug, Clone)]
pub struct UnitBreaker {
    unit: String,
    state: CircuitState,
    failure_timestamps: VecDeque<Instant>,
    trip_timestamps: VecDeque<Instant>,
    last_activity: Instant,
}

impl UnitBreaker {
    /// Creates a new circuit breaker in `Closed` state for the named unit.
    pub fn new(unit: impl Into<String>, now: Instant) -> Self {
        Self {
            unit: unit.into(),
            state: CircuitState::Closed,
            failure_timestamps: VecDeque::with_capacity(16),
            trip_timestamps: VecDeque::with_capacity(8),
            last_activity: now,
        }
    }

    /// Evaluates current state based on monotonic time and updates cooldowns.
    pub fn evaluate_state(&mut self, now: Instant) -> &CircuitState {
        self.last_activity = now;
        if let CircuitState::Open {
            tripped_at,
            cooldown,
            ..
        } = self.state
        {
            if now.saturating_duration_since(tripped_at) >= cooldown {
                self.state = CircuitState::HalfOpen;
            }
        }
        &self.state
    }

    /// Records a unit failure event, updating sliding windows and trip logic.
    pub fn record_failure(&mut self, now: Instant, config: &CircuitConfig) -> &CircuitState {
        self.last_activity = now;
        self.evaluate_state(now);

        if matches!(self.state, CircuitState::PermanentlyLocked { .. }) {
            return &self.state;
        }

        if matches!(self.state, CircuitState::Open { .. }) {
            return &self.state;
        }

        if matches!(self.state, CircuitState::HalfOpen) {
            // A failure during trial immediately re-trips or locks out
            return self.trigger_trip(now, config, 1);
        }

        // Evict expired failure timestamps outside sliding window
        if let Some(cutoff) = now.checked_sub(config.window_duration) {
            while let Some(&front) = self.failure_timestamps.front() {
                if front < cutoff {
                    self.failure_timestamps.pop_front();
                } else {
                    break;
                }
            }
        }

        // Bounded capacity: limit retained failure timestamps
        if self.failure_timestamps.len() >= config.max_failures * 2 {
            self.failure_timestamps.pop_front();
        }
        self.failure_timestamps.push_back(now);

        if self.failure_timestamps.len() >= config.max_failures {
            self.trigger_trip(now, config, self.failure_timestamps.len())
        } else {
            &self.state
        }
    }

    fn trigger_trip(&mut self, now: Instant, config: &CircuitConfig, count: usize) -> &CircuitState {
        self.failure_timestamps.clear();

        // Evict expired trip records outside flap window
        if let Some(flap_cutoff) = now.checked_sub(config.flap_window) {
            while let Some(&front) = self.trip_timestamps.front() {
                if front < flap_cutoff {
                    self.trip_timestamps.pop_front();
                } else {
                    break;
                }
            }
        }

        if self.trip_timestamps.len() >= config.flap_threshold * 2 {
            self.trip_timestamps.pop_front();
        }
        self.trip_timestamps.push_back(now);

        if self.trip_timestamps.len() >= config.flap_threshold {
            self.state = CircuitState::PermanentlyLocked {
                locked_at: now,
                flap_trips: self.trip_timestamps.len(),
            };
        } else {
            let trip_count = self.trip_timestamps.len().max(1);
            let exponent = (trip_count - 1).min(30) as u32;
            let multiplier = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
            let cooldown = config
                .cooldown_duration
                .checked_mul(multiplier)
                .map(|d| d.min(config.max_cooldown))
                .unwrap_or(config.max_cooldown);

            self.state = CircuitState::Open {
                tripped_at: now,
                cooldown,
                failure_count: count,
            };
        }

        &self.state
    }

    /// Records successful operation during HalfOpen recovery.
    pub fn record_success(&mut self, now: Instant) {
        self.last_activity = now;
        if self.state == CircuitState::HalfOpen {
            self.state = CircuitState::Closed;
            self.failure_timestamps.clear();
        }
    }

    /// Operator-initiated manual reset (clears all failure counts and lockouts).
    pub fn reset(&mut self, now: Instant) {
        self.last_activity = now;
        self.state = CircuitState::Closed;
        self.failure_timestamps.clear();
        self.trip_timestamps.clear();
    }

    /// Immediately permanently locks this breaker against restart loops.
    pub fn lock_permanently(&mut self, now: Instant) {
        self.last_activity = now;
        self.state = CircuitState::PermanentlyLocked {
            locked_at: now,
            flap_trips: self.trip_timestamps.len().max(1),
        };
    }

    /// Returns unit name.
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Returns last activity timestamp.
    pub fn last_activity(&self) -> Instant {
        self.last_activity
    }

    /// Returns current circuit state.
    pub fn state(&self) -> &CircuitState {
        &self.state
    }

    /// Returns true if the circuit is currently Closed.
    pub fn is_closed(&self) -> bool {
        self.state == CircuitState::Closed
    }

    /// Returns whether this breaker is idle and safe to evict.
    pub fn is_idle(&self, now: Instant, ttl: Duration) -> bool {
        self.state == CircuitState::Closed
            && self.failure_timestamps.is_empty()
            && now.saturating_duration_since(self.last_activity) >= ttl
    }

    /// Generates a telemetry snapshot.
    pub fn snapshot(&self, now: Instant) -> CircuitStateSnapshot {
        let (cooldown_remaining_secs, permanently_locked) = match self.state {
            CircuitState::Open {
                tripped_at,
                cooldown,
                ..
            } => {
                let elapsed = now.saturating_duration_since(tripped_at);
                let remaining = cooldown.saturating_sub(elapsed).as_secs();
                (remaining, false)
            }
            CircuitState::PermanentlyLocked { .. } => (0, true),
            _ => (0, false),
        };

        CircuitStateSnapshot {
            state: self.state.label().to_string(),
            cooldown_remaining_secs,
            recent_failures: self.failure_timestamps.len(),
            permanently_locked,
        }
    }
}
