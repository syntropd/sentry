//! Unit tests for llama.cpp server compatibility provider.

use sentry_diagnostic::provider::LlamaCppClient;
use sentry_diagnostic::provider::LlmProvider;
use std::time::Duration;

#[test]
fn test_llama_cpp_client_construction() {
    let client = LlamaCppClient::new("http://127.0.0.1:8080", "default", Duration::from_secs(45));
    assert_eq!(client.timeout(), Duration::from_secs(45));
    assert_eq!(client.id(), "llama.cpp");
}

#[test]
fn test_llama_cpp_temperature_setting() {
    let client = LlamaCppClient::new("http://127.0.0.1:8080", "default", Duration::from_secs(15))
        .with_temperature(0.2);
    assert_eq!(client.id(), "llama.cpp");
}
