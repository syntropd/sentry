//! Conclusive verdict emitted by an autonomous triage agent run.

use super::step_record::StepRecord;
use serde::{Deserialize, Serialize};

/// Conclusive verdict resulting from an autonomous agent triage loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentVerdict {
    /// Systemd unit or target triaged.
    pub unit: String,
    /// Concise root-cause title.
    pub root_cause: String,
    /// Detailed diagnostic rationale and environmental evidence.
    pub explanation: String,
    /// Recommended remediation command or action.
    pub recommended_action: String,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,
    /// Total investigative iterations taken.
    pub total_steps: usize,
    /// Audit log of all steps executed.
    pub history: Vec<StepRecord>,
}

impl AgentVerdict {
    /// Construct a new agent verdict with audit history.
    pub fn new(
        unit: impl Into<String>,
        root_cause: impl Into<String>,
        explanation: impl Into<String>,
        recommended_action: impl Into<String>,
        confidence: f32,
        history: Vec<StepRecord>,
    ) -> Self {
        let total_steps = history.len();
        Self {
            unit: unit.into(),
            root_cause: root_cause.into(),
            explanation: explanation.into(),
            recommended_action: recommended_action.into(),
            confidence: confidence.clamp(0.0, 1.0),
            total_steps,
            history,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_verdict_creation() {
        let verdict = AgentVerdict::new(
            "nginx.service",
            "Port 80 conflict",
            "Another process bound 0.0.0.0:80",
            "systemctl restart httpd",
            0.95,
            vec![],
        );
        assert_eq!(verdict.unit, "nginx.service");
        assert_eq!(verdict.total_steps, 0);
    }
}
