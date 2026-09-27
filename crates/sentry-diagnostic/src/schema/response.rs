//! Response and health status models for LLM inference providers.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Raw completion output and metadata received from an LLM provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLlmResponse {
    /// Text content returned by the model.
    pub raw_text: String,
    /// Identifier of the model that generated the completion.
    pub model: String,
    /// Prompt tokens evaluated, if reported.
    pub prompt_tokens: Option<u32>,
    /// Completion tokens generated, if reported.
    pub completion_tokens: Option<u32>,
    /// Total request turnaround duration.
    pub latency: Duration,
}

/// Provider health status reported during diagnostics or ping probes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderHealth {
    /// True if provider responded successfully to probe.
    pub available: bool,
    /// Name of provider ("llama.cpp", "openai").
    pub provider_name: String,
    /// Currently loaded or targeted model name.
    pub model_name: String,
    /// Ping round-trip latency in milliseconds.
    pub latency_ms: u64,
    /// Additional version or health details.
    pub details: Option<String>,
}
