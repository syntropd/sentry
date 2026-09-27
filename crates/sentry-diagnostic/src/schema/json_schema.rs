//! Formal JSON Schema generator for DiagnosticPayload.

use serde_json::{json, Value};

/// Generates the strict JSON Schema for `DiagnosticPayload`.
///
/// Compatible with OpenAI Structured Outputs (`strict: true`).
pub fn diagnostic_payload_json_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "incident_id": {
                "type": "string",
                "description": "Unique incident UUIDv4"
            },
            "timestamp": {
                "type": "string",
                "description": "RFC 3339 UTC timestamp"
            },
            "unit_name": {
                "type": "string",
                "description": "Systemd unit name, e.g. 'nginx.service'"
            },
            "root_cause": {
                "type": "object",
                "properties": {
                    "summary": { "type": "string" },
                    "detail": { "type": "string" }
                },
                "required": ["summary", "detail"],
                "additionalProperties": false
            },
            "evidence": {
                "type": "object",
                "properties": {
                    "journal_lines": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "exit_code": {
                        "type": ["integer", "null"]
                    },
                    "signal": {
                        "type": ["string", "null"]
                    },
                    "coredump": {
                        "type": ["string", "null"]
                    },
                    "psi": {
                        "type": ["object", "null"]
                    },
                    "cgroup": {
                        "type": ["object", "null"]
                    }
                },
                "required": ["journal_lines"],
                "additionalProperties": false
            },
            "severity": {
                "type": "string",
                "enum": ["LOW", "MEDIUM", "HIGH", "CRITICAL"]
            },
            "proposed_remediation": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": [
                            "NO_ACTION",
                            "RESTART",
                            "RESTART_WITH_BACKOFF",
                            "RELOAD",
                            "RESET_FAILED",
                            "ESCALATE_TO_ADMIN"
                        ]
                    },
                    "rationale": { "type": "string" },
                    "risk_level": {
                        "type": "string",
                        "enum": ["LOW", "MEDIUM", "HIGH"]
                    },
                    "confidence": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0
                    }
                },
                "required": ["action", "rationale", "risk_level", "confidence"],
                "additionalProperties": false
            }
        },
        "required": [
            "incident_id",
            "timestamp",
            "unit_name",
            "root_cause",
            "evidence",
            "severity",
            "proposed_remediation"
        ],
        "additionalProperties": false
    })
}

/// Builds OpenAI `response_format` configuration with strict schema enforcement.
pub fn openai_response_format() -> Value {
    json!({
        "type": "json_schema",
        "json_schema": {
            "name": "systemd_incident_diagnostic",
            "strict": true,
            "schema": diagnostic_payload_json_schema()
        }
    })
}
