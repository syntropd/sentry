//! Reflection logic and error classifications for agentic loops.

use thiserror::Error;

/// Error categories during autonomous agentic loop execution.
#[derive(Debug, Error)]
pub enum AgentLoopError {
    /// Circuit breaker tripped due to iteration count or timeout.
    #[error("Circuit breaker tripped: {0}")]
    CircuitBreakerTripped(String),

    /// Invocation of sandboxed tool failed.
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    /// Context semantic retrieval query failed.
    #[error("Context retrieval failed: {0}")]
    ContextRetrievalFailed(String),

    /// LLM provider reasoning query failed.
    #[error("LLM reasoning engine error: {0}")]
    ReasoningFailed(String),

    /// Underling I/O transport failure.
    #[error("I/O error during triage: {0}")]
    Io(#[from] std::io::Error),
}

/// Analyze an environmental observation against current hypothesis to formulate a reflection.
pub fn reflect_on_observation(hypothesis: &str, observation: &str, is_error: bool) -> String {
    if is_error {
        return format!(
            "Tool execution failed while verifying '{}'. Error: {}",
            hypothesis, observation
        );
    }

    let obs_lower = observation.to_lowercase();
    if obs_lower.contains("out of memory") || obs_lower.contains("oom-kill") {
        "Observation strongly indicates kernel OOM killer termination.".into()
    } else if obs_lower.contains("permission denied") || obs_lower.contains("eacces") {
        "Observation confirms filesystem permission or capability restriction.".into()
    } else if obs_lower.contains("connection refused") || obs_lower.contains("econnrefused") {
        "Observation confirms socket endpoint unavailability or dependency failure.".into()
    } else if obs_lower.contains("sigsegv") || obs_lower.contains("segmentation fault") {
        "Observation reveals memory corruption or illegal memory access.".into()
    } else if obs_lower.is_empty() {
        "Empty observation received; cannot confirm hypothesis without further probes.".into()
    } else {
        format!(
            "Reflected on observation for hypothesis '{}': findings consistent with partial failure.",
            hypothesis
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflect_oom() {
        let r = reflect_on_observation("test", "kernel: oom-killer invoked", false);
        assert!(r.contains("OOM killer"));
    }

    #[test]
    fn test_reflect_error() {
        let r = reflect_on_observation("test", "cannot open file", true);
        assert!(r.contains("failed"));
    }
}
