#![allow(dead_code)]
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
    PermanentlyLocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitVerdict {
    Allowed,
    Tripped { cooldown_secs: u64 },
    FlapLockout,
    Rejected { remaining_secs: u64 },
}

pub struct UnitBreaker {
    pub state: CircuitState,
    pub window_duration_secs: u64,
    pub max_failures: usize,
    pub base_cooldown_secs: u64,
    pub max_cooldown_secs: u64,
    pub flap_window_secs: u64,
    pub flap_trip_threshold: usize,
    pub consecutive_trips: u32,
    pub lockout_until: u64,
    pub failures: VecDeque<u64>,
    pub trip_timestamps: VecDeque<u64>,
}

impl UnitBreaker {
    pub fn new(window_secs: u64, max_failures: usize, base_cooldown_secs: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            window_duration_secs: window_secs,
            max_failures,
            base_cooldown_secs,
            max_cooldown_secs: 1800,
            flap_window_secs: 900,
            flap_trip_threshold: 3,
            consecutive_trips: 0,
            lockout_until: 0,
            failures: VecDeque::new(),
            trip_timestamps: VecDeque::new(),
        }
    }

    pub fn record_failure(&mut self, now_secs: u64) -> CircuitVerdict {
        if self.state == CircuitState::PermanentlyLocked {
            return CircuitVerdict::FlapLockout;
        }

        if self.state == CircuitState::Open {
            if now_secs < self.lockout_until {
                return CircuitVerdict::Rejected {
                    remaining_secs: self.lockout_until - now_secs,
                };
            } else {
                self.state = CircuitState::HalfOpen;
            }
        }

        // Evict failures outside the sliding window
        let cutoff = now_secs.saturating_sub(self.window_duration_secs);
        while let Some(&t) = self.failures.front() {
            if t < cutoff {
                self.failures.pop_front();
            } else {
                break;
            }
        }
        self.failures.push_back(now_secs);

        // Check if breaker trips
        if self.failures.len() >= self.max_failures {
            self.consecutive_trips += 1;
            self.failures.clear();
            // Record trip timestamp in flap window
            let flap_cutoff = now_secs.saturating_sub(self.flap_window_secs);
            while let Some(&t) = self.trip_timestamps.front() {
                if t < flap_cutoff {
                    self.trip_timestamps.pop_front();
                } else {
                    break;
                }
            }
            self.trip_timestamps.push_back(now_secs);

            // Flap detection check
            if self.trip_timestamps.len() >= self.flap_trip_threshold {
                self.state = CircuitState::PermanentlyLocked;
                return CircuitVerdict::FlapLockout;
            }

            // Exponential backoff
            let factor = 2u64.pow(self.consecutive_trips.saturating_sub(1).min(6));
            let cooldown = (self.base_cooldown_secs * factor).min(self.max_cooldown_secs);
            self.state = CircuitState::Open;
            self.lockout_until = now_secs + cooldown;
            return CircuitVerdict::Tripped { cooldown_secs: cooldown };
        }

        CircuitVerdict::Allowed
    }

    pub fn check_action(&mut self, now_secs: u64) -> CircuitVerdict {
        match self.state {
            CircuitState::PermanentlyLocked => CircuitVerdict::FlapLockout,
            CircuitState::Closed => CircuitVerdict::Allowed,
            CircuitState::HalfOpen => CircuitVerdict::Allowed,
            CircuitState::Open => {
                if now_secs >= self.lockout_until {
                    self.state = CircuitState::HalfOpen;
                    CircuitVerdict::Allowed
                } else {
                    CircuitVerdict::Rejected {
                        remaining_secs: self.lockout_until - now_secs,
                    }
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.consecutive_trips = 0;
        self.lockout_until = 0;
        self.failures.clear();
        self.trip_timestamps.clear();
    }
}
