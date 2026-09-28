//! Resilient JSON sanitization and deserialization pipeline.

use crate::sanitize::strip_markdown::slice_outermost_json;
use crate::sanitize::repair::repair_json;
use crate::sanitize::strip_markdown::strip_markdown_fences;
use sentry_core::error::DiagnosticError;
use sentry_core::models::DiagnosticPayload;

/// Multi-stage pipeline to sanitize and parse raw LLM output into `DiagnosticPayload`.
#[derive(Debug, Default, Clone)]
pub struct SanitizationPipeline;

/// Type alias for `SanitizationPipeline`.
pub type DiagnosticSanitizer = SanitizationPipeline;


impl SanitizationPipeline {
    /// Constructs a new sanitization pipeline.
    pub fn new() -> Self {
        Self
    }

    /// Executes the 3-stage sanitization and validation workflow.
    ///
    /// - Stage 1: Strip markdown fences and slice outermost `{ ... }` brackets.
    /// - Stage 2: Serde deserialization and semantic validation (confidence 0..=1).
    /// - Stage 3: Heuristic JSON repair for truncated tokens and retry deserialization.
    pub fn process(&self, raw_input: &str) -> Result<DiagnosticPayload, DiagnosticError> {
        // Stage 1: Strip markdown fences and slice outermost brackets
        let stripped = strip_markdown_fences(raw_input);
        let sliced = slice_outermost_json(stripped).unwrap_or(stripped);

        // Stage 2: Attempt standard Serde deserialization
        match Self::deserialize_and_validate(sliced) {
            Ok(payload) => Ok(payload),
            Err(stage2_err) => {
                // Stage 3: Attempt heuristic JSON repair
                let repaired = repair_json(sliced);
                match Self::deserialize_and_validate(&repaired) {
                    Ok(payload) => Ok(payload),
                    Err(_) => Err(stage2_err),
                }
            }
        }
    }

    fn deserialize_and_validate(json_str: &str) -> Result<DiagnosticPayload, DiagnosticError> {
        let payload: DiagnosticPayload = serde_json::from_str(json_str)
            .map_err(|e| DiagnosticError::MalformedJson(e.to_string()))?;

        // Semantic validation: confidence score must be in range 0.0..=1.0
        let conf = payload.proposed_remediation.confidence;
        if !(0.0..=1.0).contains(&conf) {
            return Err(DiagnosticError::SchemaValidation(format!(
                "Confidence score {conf} out of bounds (must be 0.0 to 1.0)"
            )));
        }

        Ok(payload)
    }
}
