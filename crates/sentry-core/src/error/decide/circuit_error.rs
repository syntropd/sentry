//! Errors originating from sliding-window circuit breaker.

use thiserror::Error;

/// Circuit breaker and flap detector errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CircuitError {
    /// Circuit is OPEN; execution is locked out.
    #[error("Circuit breaker is OPEN for unit '{unit}'. Cooldown remaining: {cooldown_remaining_secs}s")]
    CircuitOpen {
        /// Target unit.
        unit: String,
        /// Cooldown remaining.
        cooldown_remaining_secs: u64,
    },

    /// Flapping service lockout triggered by excessive crash frequency.
    #[error("Service '{unit}' locked out due to flapping ({count} failures in {flap_window_secs}s)")]
    FlappingLockout {
        /// Target unit.
        unit: String,
        /// Flap monitoring window in seconds.
        flap_window_secs: u64,
        /// Recorded failure count.
        count: usize,
    },

    /// State machine invalid transition error.
    #[error("Invalid circuit transition from '{from}' to '{to}'")]
    InvalidStateTransition {
        /// Source state.
        from: String,
        /// Target state.
        to: String,
    },
}
