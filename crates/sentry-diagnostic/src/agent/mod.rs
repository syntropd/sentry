//! Autonomous agentic triage and environmental feedback loop.

pub mod agent_loop;
pub mod circuit_breaker;
pub mod client_contextd;
pub mod client_toold;
pub mod reflect_error;

pub use agent_loop::AgenticTriageLoop;
pub use circuit_breaker::CircuitBreaker;
pub use client_contextd::ContextdClient;
pub use client_toold::TooldClient;
pub use reflect_error::{reflect_on_observation, AgentLoopError};
