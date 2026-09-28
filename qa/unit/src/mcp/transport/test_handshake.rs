//! Unit tests for MCP handshake initialization and ping.

use sentry_mcp::protocol::{handle_initialize, handle_ping, RequestId, MCP_PROTOCOL_VERSION};

#[test]
fn test_handle_initialize_response_format() {
    let resp = handle_initialize(Some(RequestId::Number(1)));
    assert_eq!(resp.id, Some(RequestId::Number(1)));

    let result = resp.result.expect("Result present");
    assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
    assert_eq!(result["serverInfo"]["name"], "systemd-sentry-mcp");
    assert_eq!(result["serverInfo"]["version"], "0.1.0");
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"]["resources"].is_object());
    assert!(result["capabilities"]["prompts"].is_object());
}

#[test]
fn test_handle_ping_response() {
    let resp = handle_ping(Some(RequestId::Number(2)));
    assert_eq!(resp.id, Some(RequestId::Number(2)));
    assert_eq!(resp.result, Some(serde_json::json!({})));
}
