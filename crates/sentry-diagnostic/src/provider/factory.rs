//! Factory for instantiating LLM providers from configuration.

use crate::provider::llama_cpp::LlamaCppClient;
use crate::provider::openai::OpenAiClient;
use crate::provider::LlmProvider;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Supported LLM provider types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    /// Local or remote llama.cpp server.
    LlamaCpp,
    /// Cloud OpenAI or OpenAI-compatible endpoint.
    OpenAi,
}

/// Unified provider configuration parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider kind.
    pub kind: ProviderKind,
    /// Base URL (e.g. "http://127.0.0.1:32768/v1" or "https://api.openai.com/v1").
    pub base_url: String,
    /// Model name.
    pub model: String,
    /// Optional API key for cloud providers.
    pub api_key: Option<String>,
    /// Request timeout.
    pub timeout: Duration,
    /// Sampling temperature (default 0.1).
    pub temperature: f32,
    /// Adaptive timeout and circuit breaker configuration.
    #[serde(default)]
    pub adaptive: crate::circuit::AdaptiveTimeoutConfig,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::OpenAi,
            base_url: "http://127.0.0.1:32768/v1".to_string(),
            model: "fast".to_string(),
            api_key: None,
            timeout: Duration::from_secs(45),
            temperature: 0.1,
            adaptive: crate::circuit::AdaptiveTimeoutConfig::default(),
        }
    }
}

/// Constructs a boxed provider matching the given configuration.
pub fn create_provider(config: &ProviderConfig) -> Box<dyn LlmProvider> {
    match config.kind {
        ProviderKind::LlamaCpp => {
            let client = LlamaCppClient::new(&config.base_url, &config.model, config.timeout)
                .with_temperature(config.temperature);
            Box::new(client)
        }
        ProviderKind::OpenAi => {
            let client = OpenAiClient::new(
                &config.base_url,
                config.api_key.clone(),
                &config.model,
                config.timeout,
            )
            .with_temperature(config.temperature);
            Box::new(client)
        }
    }
}
