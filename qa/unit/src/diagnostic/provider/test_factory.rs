//! Unit tests for provider factory.

use sentry_diagnostic::provider::{create_provider, ProviderConfig, ProviderKind};
use std::time::Duration;

#[test]
fn test_create_llama_cpp_provider() {
    let config = ProviderConfig {
        kind: ProviderKind::LlamaCpp,
        base_url: "http://127.0.0.1:8080".to_string(),
        model: "qwen2.5".to_string(),
        api_key: None,
        timeout: Duration::from_secs(30),
        temperature: 0.1,
        adaptive: Default::default(),
    };

    let provider = create_provider(&config);
    assert_eq!(provider.id(), "llama.cpp");
}

#[test]
fn test_create_openai_provider() {
    let config = ProviderConfig {
        kind: ProviderKind::OpenAi,
        base_url: "https://api.openai.com/v1".to_string(),
        model: "gpt-4o-mini".to_string(),
        api_key: Some("secret".to_string()),
        timeout: Duration::from_secs(30),
        temperature: 0.1,
        adaptive: Default::default(),
    };

    let provider = create_provider(&config);
    assert_eq!(provider.id(), "openai");
}

#[test]
fn test_default_provider_config() {
    let config = ProviderConfig::default();
    assert_eq!(config.kind, ProviderKind::OpenAi);
    assert_eq!(config.base_url, "http://127.0.0.1:32768/v1");
    assert_eq!(config.model, "fast");

    let provider = create_provider(&config);
    assert_eq!(provider.id(), "openai");
}

#[test]
fn test_provider_kind_serde_aliases() {
    let parse = |s: &str| serde_json::from_str::<ProviderKind>(s).expect("valid enum string");
    assert_eq!(parse(r#""Ollama""#), ProviderKind::OpenAi);
    assert_eq!(parse(r#""ollama""#), ProviderKind::OpenAi);
    assert_eq!(parse(r#""openai""#), ProviderKind::OpenAi);
    assert_eq!(parse(r#""llama_cpp""#), ProviderKind::LlamaCpp);
    assert_eq!(parse(r#""llamacpp""#), ProviderKind::LlamaCpp);
    assert_eq!(parse(r#""Llama_Cpp""#), ProviderKind::LlamaCpp);
}

#[test]
fn test_provider_kind_toml_aliases() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        kind: ProviderKind,
    }
    let parse_toml = |s: &str| toml::from_str::<Wrapper>(s).expect("valid toml").kind;
    assert_eq!(parse_toml("kind = \"Ollama\""), ProviderKind::OpenAi);
    assert_eq!(parse_toml("kind = \"ollama\""), ProviderKind::OpenAi);
    assert_eq!(parse_toml("kind = \"llama_cpp\""), ProviderKind::LlamaCpp);
    assert_eq!(parse_toml("kind = \"openai\""), ProviderKind::OpenAi);
}

