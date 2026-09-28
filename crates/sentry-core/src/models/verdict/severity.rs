//! Incident severity level classification.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Incident urgency and impact classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    /// Informational or low-impact anomalies.
    Low = 0,
    /// Degraded performance or non-critical service disruption.
    Medium = 1,
    /// Critical service failure requiring prompt recovery.
    High = 2,
    /// Severe system-wide failure, crash loops, or kernel panics.
    Critical = 3,
}

impl Severity {
    /// Returns the static string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }

    /// Returns true if this severity is High or Critical.
    pub const fn is_urgent(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().as_str() {
            "LOW" => Ok(Self::Low),
            "MEDIUM" | "MED" => Ok(Self::Medium),
            "HIGH" => Ok(Self::High),
            "CRITICAL" | "CRIT" => Ok(Self::Critical),
            other => Err(format!("Unknown severity value: '{other}'")),
        }
    }
}
