//! Autonomous triage agent hypothesis and forward planning model.

use serde::{Deserialize, Serialize};

/// Triage investigation plan formulated by the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentPlan {
    /// Diagnostic hypothesis currently being tested.
    pub hypothesis: String,
    /// Next planned investigative actions.
    pub planned_actions: Vec<String>,
    /// Confidence score in current diagnostic direction (0.0 - 1.0).
    pub confidence: f32,
}

impl AgentPlan {
    /// Constructs a new agent plan.
    pub fn new(hypothesis: impl Into<String>, planned_actions: Vec<String>, confidence: f32) -> Self {
        Self {
            hypothesis: hypothesis.into(),
            planned_actions,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_plan_creation() {
        let plan = AgentPlan::new(
            "OOM killer terminated process",
            vec!["inspect memory cgroup".into(), "check journal".into()],
            0.85,
        );
        assert_eq!(plan.planned_actions.len(), 2);
        assert!(plan.confidence > 0.8);
    }
}
