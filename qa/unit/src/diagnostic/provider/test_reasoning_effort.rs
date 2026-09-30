//! Unit tests verifying reasoning effort configuration and defaults.

use sentry_diagnostic::provider::ProviderConfig;

#[test]
fn test_default_provider_config_sets_low_reasoning_effort() {
    let config = ProviderConfig::default();
    assert_eq!(config.reasoning_effort.as_deref(), Some("low"));
}

#[test]
fn test_provider_config_toml_roundtrip_reasoning_effort() {
    let toml_str = r#"
kind = "openai"
base_url = "http://127.0.0.1:32768/v1"
model = "fast"
timeout = { secs = 45, nanos = 0 }
temperature = 0.1
reasoning_effort = "medium"
"#;
    let config: ProviderConfig = toml::from_str(toml_str).expect("parse toml");
    assert_eq!(config.reasoning_effort.as_deref(), Some("medium"));
}

#[test]
fn test_provider_config_toml_omitted_reasoning_effort() {
    let toml_str = r#"
kind = "openai"
base_url = "http://127.0.0.1:32768/v1"
model = "fast"
timeout = { secs = 45, nanos = 0 }
temperature = 0.1
"#;
    let config: ProviderConfig = toml::from_str(toml_str).expect("parse toml");
    assert_eq!(config.reasoning_effort, None);
}
