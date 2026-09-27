//! Diagnostic Engine for systemd-sentry.
//!
//! Provides:
//! - Dual LLM providers: local llama.cpp, cloud OpenAI-compatible.
//! - Strict JSON schema generation and Serde models for `DiagnosticPayload`.
//! - Resilient multi-stage JSON sanitization and repair pipeline.
//! - Deterministic rule-based fallback triage for Linux signals, exit codes, and journal patterns.
//!
//! 100% pure Rust, zero unsafe code, zero dynamic C library dependencies.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod circuit;
pub mod engine;
pub mod fallback;
pub mod provider;
pub mod sanitize;
pub mod schema;

pub use circuit::{
    AdaptiveTimeoutConfig, LatencyTracker, ProviderAction, ProviderBreaker, ProviderCircuitState,
};
pub use engine::DiagnosticEngine;
pub use fallback::DeterministicFallbackEngine;
pub use provider::{
    create_provider, LlamaCppClient, LlmProvider, OpenAiClient, ProviderConfig,
    ProviderKind,
};
pub use sanitize::{DiagnosticSanitizer, SanitizationPipeline};
pub use schema::{
    diagnostic_payload_json_schema, openai_response_format, DiagnosticPrompt, ProviderHealth,
    RawLlmResponse,
};

