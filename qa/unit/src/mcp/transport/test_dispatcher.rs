//! Unit tests for McpDispatcher message routing.

use sentry_mcp::protocol::{INVALID_PARAMS, METHOD_NOT_FOUND, PARSE_ERROR};
use sentry_mcp::server::McpDispatcher;
use sentry_mcp::storage::McpState;

#[test]
fn test_dispatcher_parse_error() {
    let state = McpState::new();
    let resp = McpDispatcher::handle_message(&state, "not json at all").expect("Response for error");
    assert_eq!(resp.error.as_ref().unwrap().code, PARSE_ERROR);
}

#[test]
fn test_dispatcher_method_not_found() {
    let state = McpState::new();
    let req = r#"{"jsonrpc": "2.0", "id": 1, "method": "unknown_function"}"#;
    let resp = McpDispatcher::handle_message(&state, req).expect("Response");
    assert_eq!(resp.error.as_ref().unwrap().code, METHOD_NOT_FOUND);
}

#[test]
fn test_dispatcher_notification_returns_none() {
    let state = McpState::new();
    let req = r#"{"jsonrpc": "2.0", "method": "notifications/initialized"}"#;
    assert!(McpDispatcher::handle_message(&state, req).is_none());
}

#[test]
fn test_dispatcher_initialize_and_tools_list() {
    let state = McpState::new();

    let init_req = r#"{"jsonrpc": "2.0", "id": 1, "method": "initialize"}"#;
    let resp = McpDispatcher::handle_message(&state, init_req).expect("Response");
    assert!(resp.result.is_some());

    let tools_req = r#"{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}"#;
    let resp = McpDispatcher::handle_message(&state, tools_req).expect("Response");
    let result = resp.result.unwrap();
    assert!(result["tools"].is_array());
}

#[test]
fn test_dispatcher_tools_call_missing_name() {
    let state = McpState::new();
    let req = r#"{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {}}"#;
    let resp = McpDispatcher::handle_message(&state, req).expect("Response");
    assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
}
