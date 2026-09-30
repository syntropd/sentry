//! System One fast classification and candidate scoring.
//!
//! Submodules provide:
//! - `client`: Varlink client communicating with `runtimed`.
//! - `prompt_builder`: Formats incident telemetry into candidate classification prompts.
//! - `classifier`: System One decision coordinator.

pub mod classifier;
pub mod client;
pub mod prompt_builder;

pub use classifier::SystemOneClassifier;
pub use client::DecisionCandidate;
pub use client::RawDecisionResult;
pub use client::ScoredCandidate;
pub use client::SystemOneClient;
pub use prompt_builder::build_decision_prompt;
