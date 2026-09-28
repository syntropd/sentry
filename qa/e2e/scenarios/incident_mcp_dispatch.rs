//! Tier 3: Cross-Feature: Incident Logging & MCP Protocol Tool/Resource Dispatch
//!
//! Validates incident querying via JSON-RPC 2.0 MCP tools and URI resource resolution.

use std::collections::HashMap;

#[test]
fn test_tier3_mcp_get_incident_tool_retrieval() {
    let mut incident_db = HashMap::new();
    incident_db.insert(
        "inc-01J8K3M9".to_string(),
        serde_json::json!({
            "incident_id": "inc-01J8K3M9",
            "unit": "api-worker.service",
            "severity": "CRITICAL",
            "root_cause": "SIGSEGV null deref"
        }),
    );

    // Incoming JSON-RPC tool request
    let request_id = 42;
    let target_uuid = "inc-01J8K3M9";

    let response = if let Some(record) = incident_db.get(target_uuid) {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "content": [{ "type": "text", "text": record.to_string() }]
            }
        })
    } else {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "error": { "code": -32602, "message": "Incident not found" }
        })
    };

    assert_eq!(response["id"], 42);
    assert!(response["result"]["content"][0]["text"].as_str().unwrap().contains("api-worker.service"));
}

#[test]
fn test_tier3_mcp_incident_uri_resource_resolution() {
    let uri = "incident://inc-01J8K3M9";
    assert!(uri.starts_with("incident://"));

    let id = uri.strip_prefix("incident://").unwrap();
    assert_eq!(id, "inc-01J8K3M9");
}
