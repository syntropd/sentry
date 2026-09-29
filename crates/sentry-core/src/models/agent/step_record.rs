//! Execution step record for autonomous agentic triage loops.

use serde::{Deserialize, Serialize};

/// Action invoked by an autonomous agent during an investigation step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentAction {
    /// Sandboxed tool execution via toold.
    ToolExecution {
        /// Canonical tool name.
        tool: String,
        /// Positional tool arguments.
        args: Vec<String>,
    },
    /// Context semantic retrieval via contextd.
    ContextQuery {
        /// Natural language retrieval query.
        query: String,
    },
    /// Reached conclusive diagnosis or termination.
    Complete {
        /// Diagnostic conclusion statement.
        conclusion: String,
    },
}

/// A recorded execution step within the agentic triage trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRecord {
    /// Zero-indexed execution step counter.
    pub step_index: usize,
    /// Action taken in this step.
    pub action: AgentAction,
    /// Environmental observation received from tool or context query.
    pub observation: String,
    /// Agent reflection evaluating the observation against hypothesis.
    pub reflection: Option<String>,
    /// Step execution duration in milliseconds.
    pub duration_ms: u64,
}

impl StepRecord {
    /// Construct a new step record.
    pub fn new(step_index: usize, action: AgentAction, observation: String, duration_ms: u64) -> Self {
        Self {
            step_index,
            action,
            observation,
            reflection: None,
            duration_ms,
        }
    }

    /// Attach an evaluative reflection to this record.
    pub fn with_reflection(mut self, reflection: String) -> Self {
        self.reflection = Some(reflection);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_record_lifecycle() {
        let step = StepRecord::new(
            0,
            AgentAction::ContextQuery { query: "coredump".into() },
            "Found crash dump".into(),
            120,
        )
        .with_reflection("Crash was triggered by SIGSEGV".into());

        assert_eq!(step.step_index, 0);
        assert_eq!(step.reflection.as_deref(), Some("Crash was triggered by SIGSEGV"));
    }
}
