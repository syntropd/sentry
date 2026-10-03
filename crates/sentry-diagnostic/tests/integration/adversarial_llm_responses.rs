//! Adversarial stress tests for malformed, truncated, and corrupted LLM responses.

use sentry_core::models::{DriverEvent, IncidentContext, UnitFailedDetails};
use sentry_diagnostic::engine::DiagnosticEngine;
use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::sanitize::repair::repair_json;
use sentry_diagnostic::sanitize::strip_markdown::strip_markdown_fences;
use sentry_diagnostic::sanitize::SanitizationPipeline;
use sentry_diagnostic::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use std::sync::Arc;
use std::time::Duration;

struct StaticMockProvider(String);

#[async_trait::async_trait]
impl LlmProvider for StaticMockProvider {
    async fn ping(&self) -> Result<ProviderHealth, sentry_core::error::DiagnosticError> {
        Ok(ProviderHealth {
            available: true,
            provider_name: "static-mock".to_string(),
            model_name: "mock-v1".to_string(),
            latency_ms: 1,
            details: None,
        })
    }

    async fn complete(&self, _prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, sentry_core::error::DiagnosticError> {
        Ok(RawLlmResponse {
            raw_text: self.0.clone(),
            model: "mock-v1".to_string(),
            prompt_tokens: Some(50),
            completion_tokens: Some(50),
            latency: Duration::from_millis(5),
        })
    }

    fn id(&self) -> &'static str {
        "static-mock"
    }
}

fn make_test_context() -> IncidentContext {
    IncidentContext::new(
        "adversarial-target.service",
        DriverEvent::UnitFailed(UnitFailedDetails {
            unit: "adversarial-target.service".to_string(),
            active_state: "failed".to_string(),
            sub_state: "failed".to_string(),
            result: Some("exit-code".to_string()),
            exec_status: Some(139),
            main_pid: Some(4242),
        }),
    )
}

#[test]
fn test_repair_handles_nested_unclosed_markdown_fences() {
    let inputs = [
        "````json\n```json\n{\"root_cause\": {\"summary\": \"test\"}}\n```\n````",
        "```markdown\n```json\n{\"root_cause\": {\"summary\": \"nested\"}}",
        "``````json\n{\"summary\": \"many backticks\"}\n``````",
        "```json\n{\"summary\": \"unclosed fence",
    ];

    for raw in inputs {
        let stripped = strip_markdown_fences(raw);
        let repaired = repair_json(stripped);
        assert!(!repaired.is_empty(), "Repaired output must never be empty");
    }
}

#[test]
fn test_repair_handles_broken_braces_and_mismatched_delimiters() {
    let broken = [
        "{{{{{{{{",
        "}}}}}}}}",
        "}{}{}{}{",
        "[{[{[}]}]",
        "{\"key\": [1, 2, {\"nested\": 3",
        "{\"unclosed_string\": \"some value with \\\"escaped\\\" quote",
        "{\"unclosed_with_trailing_slash\": \"abc\\\\",
    ];

    for input in broken {
        let repaired = repair_json(input);
        assert!(!repaired.is_empty());
    }
}

#[test]
fn test_repair_handles_extreme_trailing_commas() {
    let comma_inputs = [
        "{\"a\": 1,,,,,,,,}",
        "{\"items\": [1, 2, 3,,,,]}",
        "{\"a\": {\"b\": 2,},},",
        "{\"key\": \"val\",,,,,     \n\t  ",
    ];

    for input in comma_inputs {
        let repaired = repair_json(input);
        assert!(!repaired.is_empty());
        assert!(
            !repaired.ends_with(",}"),
            "clean_trailing_comma failed to strip consecutive trailing commas before brace in: {:?}",
            repaired
        );
        assert!(
            !repaired.ends_with(",]"),
            "clean_trailing_comma failed to strip consecutive trailing commas before bracket in: {:?}",
            repaired
        );
    }
}

#[test]
fn test_sanitization_handles_embedded_null_bytes() {
    let pipeline = SanitizationPipeline::new();
    let null_inputs = [
        "{\"unit_name\": \"test\0.service\"}",
        "\0\0{\"root_cause\": {\"summary\": \"nulls\0everywhere\"}}\0\0",
        "```json\n{\0\"a\": \"b\"\0}\n```",
    ];

    for input in null_inputs {
        let _ = pipeline.process(input);
    }
}

#[test]
fn test_deeply_nested_structures_do_not_stack_overflow() {
    let mut deep = String::from("{\"a\":");
    for _ in 0..120 {
        deep.push_str("{\"nested\":");
    }
    deep.push_str("\"bottom\"");
    for _ in 0..120 {
        deep.push('}');
    }
    deep.push('}');

    let pipeline = SanitizationPipeline::new();
    let _ = pipeline.process(&deep);
}

#[tokio::test]
async fn test_engine_never_panics_on_adversarial_llm_outputs() {
    let corrupt_payloads = [
        "",
        "   \t\n\r  ",
        "404 Not Found",
        "Internal Server Error: model crashed",
        "```json\n[{\"wrong\": \"type\"}]\n```",
        "{\"incident_id\": 12345, \"severity\": \"INVALID_SEVERITY\"}",
        "{\"proposed_remediation\": {\"action\": \"FORMAT_DISK\", \"confidence\": 999.0}}",
        "{\"unit_name\": null, \"root_cause\": null}",
        "NaN",
        "{\"a\": Infinity}",
        "Here is your analysis: ```json {\"summary\": \"ok\"} ``` Hope this helps!",
        "\x00\x01\x02\x03\x04\x05\x06\x07\x08\x0b\x0c\x0e\x0f",
        "{\"proposed_remediation\": {\"confidence\": -1.0}}",
        "{\"proposed_remediation\": {\"confidence\": 1.0001}}",
    ];

    let ctx = make_test_context();

    for payload in corrupt_payloads {
        let mock = Arc::new(StaticMockProvider(payload.to_string()));
        let engine = DiagnosticEngine::new(Some(mock));

        let diagnosis = engine.diagnose(&ctx).await;

        assert_eq!(diagnosis.unit_name, ctx.unit);
        assert_eq!(diagnosis.incident_id, ctx.incident_id);
    }
}
