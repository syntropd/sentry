//! Automated fuzz harness asserting zero panics and invariant preservation.

use chrono::Utc;
use sentry_core::models::{DriverEvent, IncidentContext, UnitFailedDetails};
use sentry_diagnostic::engine::DiagnosticEngine;
use sentry_diagnostic::fallback::DeterministicFallbackEngine;
use sentry_diagnostic::provider::LlmProvider;
use sentry_diagnostic::sanitize::SanitizationPipeline;
use sentry_diagnostic::schema::{DiagnosticPrompt, ProviderHealth, RawLlmResponse};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

struct XorShift64(u64);

impl XorShift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn generate_fuzz_string(rng: &mut XorShift64) -> String {
    let tokens = [
        "{", "}", "[", "]", "\"", "\\", ":", ",", "\0", "\n", "\r", "\t",
        "```", "```json", "null", "true", "false", "0", "1.0", "-1", "999999",
        "unit_name", "incident_id", "root_cause", "proposed_remediation", "action",
        "RESTART", "CONFIDENCE", "summary", "detail", "evidence", "journal_lines",
        "💥", "🦀", "a", " ", "\\\"",
    ];
    let len = (rng.next_u64() % 40) as usize + 1;
    let mut s = String::new();
    for _ in 0..len {
        let idx = (rng.next_u64() as usize) % tokens.len();
        s.push_str(tokens[idx]);
    }
    s
}

struct FuzzMockProvider(String);

#[async_trait::async_trait]
impl LlmProvider for FuzzMockProvider {
    async fn ping(&self) -> Result<ProviderHealth, sentry_core::error::DiagnosticError> {
        Ok(ProviderHealth {
            available: true,
            provider_name: "fuzz".to_string(),
            model_name: "fuzz-v1".to_string(),
            latency_ms: 1,
            details: None,
        })
    }

    async fn complete(&self, _prompt: &DiagnosticPrompt) -> Result<RawLlmResponse, sentry_core::error::DiagnosticError> {
        Ok(RawLlmResponse {
            raw_text: self.0.clone(),
            model: "fuzz-v1".to_string(),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            latency: Duration::from_millis(1),
        })
    }

    fn id(&self) -> &'static str {
        "fuzz"
    }
}

#[test]
fn test_fuzz_sanitizer_never_panics() {
    let mut rng = XorShift64(0xDEAD_BEEF_CAFE_BABE);
    let pipeline = SanitizationPipeline::new();

    for _ in 0..1000 {
        let candidate = generate_fuzz_string(&mut rng);
        let res = catch_unwind(AssertUnwindSafe(|| {
            let _ = pipeline.process(&candidate);
        }));
        assert!(res.is_ok(), "SanitizationPipeline panicked on fuzzed input: {:?}", candidate);
    }
}

#[test]
fn test_fuzz_fallback_engine_never_panics() {
    let mut rng = XorShift64(0x1337_C0DE_F00D_BA5E);
    let fallback = DeterministicFallbackEngine::new();

    for i in 0..1000 {
        let unit = format!("fuzz-service-{}.service", rng.next_u64() % 100);
        let exit_code = if rng.next_u64() % 2 == 0 {
            Some((rng.next_u64() as i64 - 500) as i32)
        } else {
            None
        };
        let signal = if rng.next_u64() % 3 == 0 {
            Some(generate_fuzz_string(&mut rng))
        } else {
            None
        };

        let mut lines = Vec::new();
        let num_lines = (rng.next_u64() % 10) as usize;
        for _ in 0..num_lines {
            lines.push(generate_fuzz_string(&mut rng));
        }

        let ctx = IncidentContext {
            incident_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            unit: unit.clone(),
            failure_event: DriverEvent::UnitFailed(UnitFailedDetails {
                unit: unit.clone(),
                active_state: "failed".to_string(),
                sub_state: "failed".to_string(),
                result: signal,
                exec_status: exit_code,
                main_pid: Some((rng.next_u64() % 65535) as u32),
            }),
            journal_lines: lines,
            telemetry: None,
            cgroup: None,
            coredump: None,
        };

        let res = catch_unwind(AssertUnwindSafe(|| {
            let payload = fallback.triage(&ctx);
            assert_eq!(payload.unit_name, ctx.unit);
            assert_eq!(payload.incident_id, ctx.incident_id);
            assert!((0.0..=1.0).contains(&payload.proposed_remediation.confidence));
        }));

        assert!(res.is_ok(), "Fallback engine panicked on iteration {i}");
    }
}

#[tokio::test]
async fn test_fuzz_diagnostic_engine_end_to_end() {
    let mut rng = XorShift64(0xFEED_FACE_CAFE_0001);

    for i in 0..200 {
        let fuzz_output = generate_fuzz_string(&mut rng);
        let mock = Arc::new(FuzzMockProvider(fuzz_output));
        let engine = DiagnosticEngine::new(Some(mock));
        let unit_name = format!("test-unit-{i}.service");

        let ctx = IncidentContext::new(
            unit_name.clone(),
            DriverEvent::UnitFailed(UnitFailedDetails {
                unit: unit_name.clone(),
                active_state: "failed".to_string(),
                sub_state: "failed".to_string(),
                result: None,
                exec_status: Some(139),
                main_pid: Some(1234),
            }),
        );

        let expected_unit = ctx.unit.clone();
        let expected_id = ctx.incident_id;

        let task = tokio::spawn(async move {
            engine.diagnose(&ctx).await
        });

        let res = task.await;
        assert!(res.is_ok(), "DiagnosticEngine panicked during end-to-end fuzz on iteration {i}");
        let payload = res.unwrap();
        assert_eq!(payload.unit_name, expected_unit);
        assert_eq!(payload.incident_id, expected_id);
    }
}
