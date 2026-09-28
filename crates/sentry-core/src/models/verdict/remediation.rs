//! Bounded remediation actions permitted under zero-trust policy.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Bounded set of remediation actions evaluated by the safety gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RemediationAction {
    /// No action should be taken; observation only.
    NoAction,
    /// Immediate single restart of the service unit.
    Restart,
    /// Restart with exponential backoff delay.
    RestartWithBackoff,
    /// Reload configuration files (`systemctl reload`).
    Reload,
    /// Reset failed state markers (`systemctl reset-failed`).
    ResetFailed,
    /// Escalate directly to human administrator; lockout automatic action.
    EscalateToAdmin,
}

impl RemediationAction {
    /// Returns the static string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoAction => "NO_ACTION",
            Self::Restart => "RESTART",
            Self::RestartWithBackoff => "RESTART_WITH_BACKOFF",
            Self::Reload => "RELOAD",
            Self::ResetFailed => "RESET_FAILED",
            Self::EscalateToAdmin => "ESCALATE_TO_ADMIN",
        }
    }

    /// Returns true if the action performs an active modification to a service unit.
    pub const fn is_active_modification(&self) -> bool {
        matches!(
            self,
            Self::Restart | Self::RestartWithBackoff | Self::Reload | Self::ResetFailed
        )
    }

    /// Returns systemd job mode string (e.g. "replace") if applicable.
    pub const fn systemd_job_mode(&self) -> Option<&'static str> {
        match self {
            Self::Restart | Self::RestartWithBackoff | Self::Reload => Some("replace"),
            _ => None,
        }
    }
}

impl fmt::Display for RemediationAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RemediationAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().replace('-', "_").as_str() {
            "NO_ACTION" | "NONE" => Ok(Self::NoAction),
            "RESTART" => Ok(Self::Restart),
            "RESTART_WITH_BACKOFF" | "BACKOFF_RESTART" => Ok(Self::RestartWithBackoff),
            "RELOAD" => Ok(Self::Reload),
            "RESET_FAILED" => Ok(Self::ResetFailed),
            "ESCALATE_TO_ADMIN" | "ESCALATE" => Ok(Self::EscalateToAdmin),
            other => Err(format!("Unknown remediation action: '{other}'")),
        }
    }
}
