//! Errors originating from LLM and fallback triage engines.

use thiserror::Error;

/// Diagnostic engine errors.
#[derive(Debug, Error)]
pub enum DiagnosticError {
    /// LLM endpoint (llama.cpp / OpenAI) unreachable.
    #[error("Diagnostic provider unreachable: {0}")]
    ProviderUnavailable(String),

    /// Diagnostic request timed out.
    #[error("Diagnostic query timed out after {0} ms")]
    Timeout(u64),

    /// Model output could not be sanitized or parsed as JSON.
    #[error("Malformed JSON response from model: {0}")]
    MalformedJson(String),

    /// Schema validation failed against DiagnosticPayload.
    #[error("Diagnostic schema validation failed: {0}")]
    SchemaValidation(String),

    /// Input context exceeded model prompt token limit.
    #[error("Context token limit exceeded: {0} tokens")]
    TokenLimitExceeded(usize),

    /// No safe remediation was proposed.
    #[error("No remediation action could be determined")]
    NoRemediationProposed,
}
