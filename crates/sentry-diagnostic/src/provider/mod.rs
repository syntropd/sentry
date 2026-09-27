//! LLM provider interfaces and implementations.

pub mod bounded_body;
pub mod factory;
pub mod llama_cpp;
pub mod openai;

pub use bounded_body::{
    read_bounded_bytes, read_bounded_json, read_bounded_text, MAX_HTTP_RESPONSE_BYTES,
};
use crate::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use async_trait::async_trait;
pub use factory::{create_provider, ProviderConfig, ProviderKind};
pub use llama_cpp::LlamaCppClient;
pub use openai::OpenAiClient;
use sentry_core::error::DiagnosticError;

/// Asynchronous trait implemented by all LLM diagnostic inference providers.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Pings provider endpoint and verifies model readiness.
    async fn ping(&self) -> Result<ProviderHealth, DiagnosticError>;

    /// Dispatches telemetry prompt to model and retrieves raw completion.
    async fn complete(&self, prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError>;

    /// Returns the static provider identifier.
    fn id(&self) -> &'static str;
}
