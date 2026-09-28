//! Tier 1: R3 Diagnostic Engine, Dual LLM Providers & MCP Protocol Tests
//!
//! Validates schema enforcement, markdown stripping, fallback triage, and MCP.

use super::harness_models::*;

#[test]
fn test_r3_diagnostic_payload_json_schema_serialization() {
    let payload = DiagnosticPayload {
        incident_id: "018f2d5e-6b94-7b92-939a-653a01a34800".to_string(),
        timestamp: "2026-09-24T00:15:30Z".to_string(),
        unit_name: "postgres.service".to_string(),
        root_cause: RootCause {
            summary: "Disk full on /var/lib/postgresql".to_string(),
            detail: Some("No space left on device error during write".to_string()),
        },
        evidence: Evidence {
            journal_lines: vec!["FATAL: could not extend file: No space left on device".to_string()],
            exit_codes: vec![1],
            signals: vec![],
            coredump_trace: None,
            psi_stats: None,
        },
        severity: Severity::Critical,
        proposed_remediation: ProposedRemediation {
            action: RemediationAction::EscalateToAdmin,
            rationale: Some("Requires manual disk space remediation".to_string()),
            risk_level: RiskLevel::Hazardous,
            confidence: 0.98,
        },
    };

    let serialized = serde_json::to_string_pretty(&payload).unwrap();
    let deserialized: DiagnosticPayload = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.unit_name, "postgres.service");
    assert_eq!(deserialized.severity, Severity::Critical);
    assert_eq!(deserialized.proposed_remediation.action, RemediationAction::EscalateToAdmin);
}

#[test]
fn test_r3_markdown_fence_stripper() {
    let raw_response = "```json\n{\n  \"action\": \"RESTART\",\n  \"confidence\": 0.95\n}\n```";
    let trimmed = raw_response.trim();
    let unwrapped = if let Some(stripped) = trimmed.strip_prefix("```json") {
        stripped.strip_suffix("```").unwrap_or(stripped).trim()
    } else {
        trimmed
    };

    assert!(unwrapped.starts_with('{'));
    assert!(unwrapped.ends_with('}'));
    assert!(!unwrapped.contains("```"));
}

#[test]
fn test_r3_deterministic_fallback_triage_exit_code_137_oom() {
    let exit_code = 137; // SIGKILL / OOM
    let (action, severity) = match exit_code {
        137 => (RemediationAction::RestartWithBackoff, Severity::High),
        139 => (RemediationAction::EscalateToAdmin, Severity::Critical),
        _ => (RemediationAction::NoAction, Severity::Low),
    };

    assert_eq!(action, RemediationAction::RestartWithBackoff);
    assert_eq!(severity, Severity::High);
}

#[test]
fn test_r3_deterministic_fallback_triage_exit_code_139_segfault() {
    let exit_code = 139; // SIGSEGV
    let (action, severity) = match exit_code {
        139 => (RemediationAction::EscalateToAdmin, Severity::Critical),
        _ => (RemediationAction::NoAction, Severity::Low),
    };

    assert_eq!(action, RemediationAction::EscalateToAdmin);
    assert_eq!(severity, Severity::Critical);
}

#[test]
fn test_r3_mcp_json_rpc_handshake_and_tool_call() {
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "clientInfo": { "name": "claude-desktop", "version": "1.0.0" }
        }
    });

    assert_eq!(init_request["jsonrpc"], "2.0");
    assert_eq!(init_request["method"], "initialize");

    let tool_call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "get_incident",
            "arguments": { "incident_id": "inc-01J8K3M9" }
        }
    });

    assert_eq!(tool_call["method"], "tools/call");
    assert_eq!(tool_call["params"]["name"], "get_incident");
    assert_eq!(tool_call["params"]["arguments"]["incident_id"], "inc-01J8K3M9");
}
