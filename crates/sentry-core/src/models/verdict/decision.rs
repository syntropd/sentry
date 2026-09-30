//! System One fast-path classification verdict and tri-tier decision gating.

use serde::{Deserialize, Serialize};

/// Tri-tier decision gating based on calibrated confidence S.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionTier {
    /// S >= 0.90: Auto-remediate immediately + structured journal log (zero terminal noise).
    High,
    /// 0.60 <= S < 0.90: Stage in pending queue + update pending count.
    Medium,
    /// S < 0.60: Lock unit against restart loops; record forensic log.
    Low,
}

impl DecisionTier {
    /// Computes the gating tier from a calibrated confidence score S in [0.0, 1.0].
    pub fn from_confidence(s: f32) -> Self {
        if s >= 0.90 {
            Self::High
        } else if s >= 0.60 {
            Self::Medium
        } else {
            Self::Low
        }
    }

    /// String label representation of the decision tier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
        }
    }
}

/// Canonical 4-way fault taxonomy for System One fast triage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FaultClass {
    /// Transient glitch, segfault, or temporary error safely resolved by restarting.
    TransientRestart,
    /// Syntax error, permission denial, or drift in service configuration files.
    ConfigDrift,
    /// Failure of upstream socket, mount, or dependent system service.
    DependencyFailure,
    /// Complex or high-risk failure requiring operator manual intervention.
    ManualTriageRequired,
}

impl FaultClass {
    /// Returns the canonical taxonomy name as a string slice.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TransientRestart => "TransientRestart",
            Self::ConfigDrift => "ConfigDrift",
            Self::DependencyFailure => "DependencyFailure",
            Self::ManualTriageRequired => "ManualTriageRequired",
        }
    }

    /// Parses a single-token alias ("A", "B", "C", "D") or name into a fault class.
    pub fn parse_alias(token_or_name: &str) -> Option<Self> {
        let trimmed = token_or_name.trim();
        match trimmed {
            "A" | "TransientRestart" => Some(Self::TransientRestart),
            "B" | "ConfigDrift" => Some(Self::ConfigDrift),
            "C" | "DependencyFailure" => Some(Self::DependencyFailure),
            "D" | "ManualTriageRequired" => Some(Self::ManualTriageRequired),
            _ => None,
        }
    }

    /// Canonical single-token letter alias used in classification prompts.
    pub fn default_token(&self) -> &'static str {
        match self {
            Self::TransientRestart => "A",
            Self::ConfigDrift => "B",
            Self::DependencyFailure => "C",
            Self::ManualTriageRequired => "D",
        }
    }
}

/// Full System One verdict emitted by local engine fast classifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemOneVerdict {
    /// Canonical fault taxonomy classification.
    pub fault_class: FaultClass,
    /// Calibrated confidence score S in [0.0, 1.0].
    pub confidence: f32,
    /// Uncalibrated top softmax probability.
    pub raw_probability: f32,
    /// Top-to-second decision margin delta.
    pub margin: f32,
    /// Normalized Shannon entropy across candidate options.
    pub entropy: f32,
    /// Gating tier determined by confidence score.
    pub tier: DecisionTier,
    /// Human-readable explanation or rationale for the decision.
    pub explanation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_thresholds() {
        assert_eq!(DecisionTier::from_confidence(0.95), DecisionTier::High);
        assert_eq!(DecisionTier::from_confidence(0.90), DecisionTier::High);
        assert_eq!(DecisionTier::from_confidence(0.89), DecisionTier::Medium);
        assert_eq!(DecisionTier::from_confidence(0.60), DecisionTier::Medium);
        assert_eq!(DecisionTier::from_confidence(0.59), DecisionTier::Low);
        assert_eq!(DecisionTier::from_confidence(0.0), DecisionTier::Low);
    }

    #[test]
    fn test_fault_class_aliases() {
        assert_eq!(
            FaultClass::parse_alias("A"),
            Some(FaultClass::TransientRestart)
        );
        assert_eq!(
            FaultClass::parse_alias("TransientRestart"),
            Some(FaultClass::TransientRestart)
        );
        assert_eq!(FaultClass::parse_alias("B"), Some(FaultClass::ConfigDrift));
        assert_eq!(
            FaultClass::parse_alias("C"),
            Some(FaultClass::DependencyFailure)
        );
        assert_eq!(
            FaultClass::parse_alias("D"),
            Some(FaultClass::ManualTriageRequired)
        );
        assert_eq!(FaultClass::parse_alias("unknown"), None);
    }
}
