//! Cloud OpenAI-compatible LLM inference provider.

use crate::provider::bounded_body::{read_bounded_json, read_bounded_text, MAX_HTTP_RESPONSE_BYTES};
use crate::provider::LlmProvider;
use crate::schema::{openai_response_format, DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde_json::{json, Value};
use sentry_core::error::DiagnosticError;
use std::time::{Duration, Instant};

/// OpenAI-compatible provider client supporting structured outputs and custom base URLs.
#[derive(Debug, Clone)]
pub struct OpenAiClient {
    base_url: String,
    api_key: Option<String>,
    model: String,
    client: Client,
    temperature: f32,
    max_retries: usize,
    reasoning_effort: Option<String>,
}

impl OpenAiClient {
    /// Constructs a new OpenAI-compatible client.
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
        timeout: Duration,
    ) -> Self {
        let base = base_url.into().trim_end_matches('/').to_string();
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            base_url: base,
            api_key,
            model: model.into(),
            client,
            temperature: 0.1,
            max_retries: 2,
            reasoning_effort: None,
        }
    }

    /// Sets temperature.
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    /// Sets reasoning effort.
    pub fn with_reasoning_effort(mut self, effort: Option<String>) -> Self {
        self.reasoning_effort = effort;
        self
    }

    /// Sets max retries on rate limit.
    pub fn with_max_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }

    fn endpoint_url(&self) -> String {
        if self.base_url.ends_with("/v1") {
            format!("{}/chat/completions", self.base_url)
        } else {
            format!("{}/v1/chat/completions", self.base_url)
        }
    }

    fn build_headers(&self) -> Result<HeaderMap, DiagnosticError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(key) = &self.api_key {
            let auth_val = format!("Bearer {key}");
            let header_val = HeaderValue::from_str(&auth_val).map_err(|e| {
                DiagnosticError::ProviderUnavailable(format!("Invalid API key characters: {e}"))
            })?;
            headers.insert(AUTHORIZATION, header_val);
        }

        Ok(headers)
    }

    /// Executes chat completion with retries and structured outputs.
    pub async fn chat_completions(&self, prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError> {
        let start = Instant::now();
        let url = self.endpoint_url();
        let headers = self.build_headers()?;

        let mut with_response_format = !self.base_url.contains("minimax");
        let mut attempts = 0;
        loop {
            attempts += 1;
            let mut body = json!({
                "model": self.model,
                "messages": [
                    { "role": "system", "content": DiagnosticPrompt::SYSTEM_PROMPT },
                    { "role": "user", "content": prompt.to_prompt_json() }
                ],
                "temperature": self.temperature,
                "max_completion_tokens": 2048
            });
            if let Some(effort) = &self.reasoning_effort {
                body["reasoning_effort"] = json!(effort);
            }
            if with_response_format {
                body["response_format"] = openai_response_format();
            }

            let req = self.client.post(&url).headers(headers.clone()).json(&body);

            let resp = match req.send().await {
                Ok(r) => r,
                Err(_e) if attempts <= self.max_retries => {
                    tokio::time::sleep(Duration::from_millis(500 * (1 << attempts))).await;
                    continue;
                }
                Err(e) => {
                    return Err(DiagnosticError::ProviderUnavailable(format!(
                        "OpenAI endpoint request failed: {e}"
                    )));
                }
            };

            let status = resp.status();
            if status == reqwest::StatusCode::BAD_REQUEST && with_response_format {
                with_response_format = false;
                continue;
            }

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(DiagnosticError::ProviderUnavailable(
                    "OpenAI API authentication failed (HTTP 401 Unauthorized)".to_string(),
                ));
            }

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempts <= self.max_retries {
                let delay = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(2 * (attempts as u64));

                tokio::time::sleep(Duration::from_secs(delay.clamp(1, 10))).await;
                continue;
            }

            if !status.is_success() {
                let text = read_bounded_text(resp, MAX_HTTP_RESPONSE_BYTES)
                    .await
                    .unwrap_or_default();
                return Err(DiagnosticError::ProviderUnavailable(format!(
                    "OpenAI API returned status {status}: {text}"
                )));
            }

            let resp_json: Value = read_bounded_json(resp, MAX_HTTP_RESPONSE_BYTES).await?;

            let raw_text = resp_json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();

            let prompt_tokens = resp_json["usage"]["prompt_tokens"].as_u64().map(|v| v as u32);
            let completion_tokens = resp_json["usage"]["completion_tokens"]
                .as_u64()
                .map(|v| v as u32);

            return Ok(RawLlmResponse {
                raw_text,
                model: self.model.clone(),
                prompt_tokens,
                completion_tokens,
                latency: start.elapsed(),
            });
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiClient {
    async fn ping(&self) -> Result<ProviderHealth, DiagnosticError> {
        let start = Instant::now();
        // A minimal prompt to test connectivity
        let test_prompt = DiagnosticPrompt {
            unit_name: "ping.service".to_string(),
            exit_code: Some(0),
            signal: None,
            journal_lines: vec![],
            coredump_trace: None,
            psi_stats: None,
            cgroup_stats: None,
            active_state: "active".to_string(),
            sub_state: "running".to_string(),
            timestamp: chrono::Utc::now(),
        };

        match self.chat_completions(&test_prompt).await {
            Ok(resp) => Ok(ProviderHealth {
                available: true,
                provider_name: "openai".to_string(),
                model_name: self.model.clone(),
                latency_ms: start.elapsed().as_millis() as u64,
                details: Some(format!("tokens: {:?}", resp.completion_tokens)),
            }),
            Err(e) => Err(e),
        }
    }

    async fn complete(&self, prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, DiagnosticError> {
        self.chat_completions(prompt).await
    }

    fn id(&self) -> &'static str {
        "openai"
    }
}
