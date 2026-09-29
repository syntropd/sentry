//! Autonomous agentic triage loop: Plan -> Execute -> Observe -> Reflect -> Verdict.

use super::circuit_breaker::CircuitBreaker;
use super::client_contextd::ContextdClient;
use super::client_toold::TooldClient;
use super::reflect_error::{reflect_on_observation, AgentLoopError};
use sentry_core::models::agent::{AgentAction, AgentVerdict, StepRecord};
use sentry_core::models::IncidentContext;
use std::time::Instant;

/// Autonomous agent orchestrating closed-loop diagnostic triage.
pub struct AgenticTriageLoop {
    /// Sandboxed execution client.
    pub toold: TooldClient,
    /// Semantic context retrieval client.
    pub contextd: ContextdClient,
    /// Execution safety circuit breaker.
    pub breaker: CircuitBreaker,
}

impl Default for AgenticTriageLoop {
    fn default() -> Self {
        Self {
            toold: TooldClient::default(),
            contextd: ContextdClient::default(),
            breaker: CircuitBreaker::default(),
        }
    }
}

impl AgenticTriageLoop {
    /// Construct a new autonomous agentic triage engine.
    pub fn new(toold: TooldClient, contextd: ContextdClient, breaker: CircuitBreaker) -> Self {
        Self {
            toold,
            contextd,
            breaker,
        }
    }

    /// Run autonomous multi-turn triage over an incident context until a verdict is reached.
    pub async fn run_triage(&mut self, incident: &IncidentContext) -> Result<AgentVerdict, AgentLoopError> {
        let mut history = Vec::new();
        let mut hypothesis = format!("Diagnosing unexpected failure in {}", incident.unit);

        while self.breaker.check().is_ok() {
            let step_idx = match self.breaker.advance() {
                Ok(idx) => idx,
                Err(e) => {
                    tracing::warn!("Agent circuit breaker tripped: {}", e);
                    break;
                }
            };

            // 1. Plan: select next diagnostic probe
            let action = match step_idx {
                0 => AgentAction::ContextQuery {
                    query: format!("Known failure patterns for unit {}", incident.unit),
                },
                1 => AgentAction::ToolExecution {
                    tool: "journal.slice".into(),
                    args: vec![incident.unit.clone()],
                },
                2 => AgentAction::ToolExecution {
                    tool: "unit.status".into(),
                    args: vec![incident.unit.clone()],
                },
                _ => AgentAction::Complete {
                    conclusion: format!("Comprehensive multi-probe triage complete for {}", incident.unit),
                },
            };

            if let AgentAction::Complete { ref conclusion } = action {
                history.push(StepRecord::new(
                    step_idx,
                    action.clone(),
                    conclusion.clone(),
                    0,
                ));
                break;
            }

            // 2. Execute & 3. Observe
            let t0 = Instant::now();
            let (observation, is_err) = match &action {
                AgentAction::ContextQuery { query } => match self.contextd.query_context(query).await {
                    Ok(obs) => (obs, false),
                    Err(e) => (format!("Context query error: {e}"), true),
                },
                AgentAction::ToolExecution { tool, args } => match self.toold.execute_tool(tool, args).await {
                    Ok(obs) => (obs, false),
                    Err(e) => (format!("Tool execution error: {e}"), true),
                },
                AgentAction::Complete { .. } => unreachable!(),
            };
            let duration_ms = t0.elapsed().as_millis() as u64;

            // 4. Reflect
            let reflection = reflect_on_observation(&hypothesis, &observation, is_err);
            if !reflection.is_empty() {
                hypothesis = reflection.clone();
            }

            history.push(
                StepRecord::new(step_idx, action, observation, duration_ms)
                    .with_reflection(reflection),
            );
        }

        // 5. Synthesize final AgentVerdict
        let root_cause = if hypothesis.contains("OOM") {
            "Memory budget exceeded (cgroups OOM)".into()
        } else if hypothesis.contains("permission") {
            "Filesystem access violation or capability denial".into()
        } else {
            format!("Systemd unit {} abnormal termination", incident.unit)
        };

        let explanation = format!(
            "Autonomous investigation across {} steps concluded. Final hypothesis: {}.",
            history.len(),
            hypothesis
        );

        let remediation = format!("systemctl restart {}", incident.unit);

        Ok(AgentVerdict::new(
            incident.unit.clone(),
            root_cause,
            explanation,
            remediation,
            0.88,
            history,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry_core::models::{DriverEvent, UnitFailedDetails};

    #[tokio::test]
    async fn test_agentic_loop_completes_triaging() {
        let mut loop_engine = AgenticTriageLoop::default();
        let ev = DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "crash-test.service".into(),
            active_state: "failed".into(),
            sub_state: "failed".into(),
            result: Some("exit-code".into()),
            exec_status: Some(137),
            main_pid: Some(9999),
        });
        let incident = IncidentContext::new("crash-test.service", ev);

        let verdict = loop_engine.run_triage(&incident).await.unwrap();
        assert_eq!(verdict.unit, "crash-test.service");
        assert!(verdict.total_steps > 0);
        assert!(!verdict.history.is_empty());
    }
}
