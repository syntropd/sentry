//! Unit tests for OpenAI-compatible provider client.

use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::provider::OpenAiClient;
use std::time::Duration;

#[test]
fn test_openai_client_construction() {
    let client = OpenAiClient::new(
        "https://api.openai.com/v1",
        Some("test-key".to_string()),
        "gpt-4o-mini",
        Duration::from_secs(60),
    );
    assert_eq!(client.id(), "openai");
}

#[test]
fn test_openai_client_with_options() {
    let client = OpenAiClient::new(
        "https://api.groq.com/openai/v1",
        None,
        "llama-3.1-70b-versatile",
        Duration::from_secs(30),
    )
    .with_temperature(0.0)
    .with_max_retries(3);

    assert_eq!(client.id(), "openai");
}
