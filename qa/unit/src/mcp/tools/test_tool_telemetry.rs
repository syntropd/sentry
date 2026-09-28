//! Unit tests for `get_unit_telemetry` tool execution.

use sentry_mcp::storage::McpState;
use sentry_mcp::tools::execute_get_telemetry;
use serde_json::json;

#[test]
fn test_get_telemetry_for_healthy_unit() {
    let state = McpState::new();
    let args = json!({ "unit_name": "postgres.service" });
    let res = execute_get_telemetry(&state, Some(&args));

    assert_eq!(res["isError"], false);
    let text = res["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("postgres.service"));
    assert!(text.contains("active"));
    assert!(text.contains("CLOSED"));
}

#[test]
fn test_get_telemetry_missing_unit_name() {
    let state = McpState::new();
    let res = execute_get_telemetry(&state, None);

    assert_eq!(res["isError"], true);
    let text = res["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Missing required argument"));
}
