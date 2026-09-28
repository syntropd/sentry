//! Errors originating from zero-trust policy gatekeeper.

use thiserror::Error;

/// Safety policy errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SafetyError {
    /// Action is strictly prohibited for the given unit by policy.toml.
    #[error("Action '{action}' is disallowed for unit '{unit}': {reason}")]
    ActionDisallowed {
        /// Attempted remediation action.
        action: String,
        /// Unit target.
        unit: String,
        /// Disallow reason.
        reason: String,
    },

    /// Unit is explicitly marked unmanaged or blacklisted.
    #[error("Unit '{0}' is blacklisted from automatic remediation")]
    UnitBlacklisted(String),

    /// Action cooldown or rate limit has not elapsed.
    #[error("Action rate limit active for unit '{unit}'. Reset in {reset_in_secs}s")]
    RateLimitExceeded {
        /// Unit target.
        unit: String,
        /// Seconds until cooldown reset.
        reset_in_secs: u64,
    },

    /// LLM advisory violated safety boundary.
    #[error("Advisory rejected by zero-trust gate: {0}")]
    AdvisoryRejected(String),
}
