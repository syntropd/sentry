//! Unit tests for MCP JSON-RPC 2.0 types and serialization.

use sentry_mcp::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, RequestId};
use serde_json::json;

#[test]
fn test_request_id_number_and_string() {
    let num_id = RequestId::Number(42);
    let str_id = RequestId::String("req-1".to_string());

    assert_eq!(serde_json::to_string(&num_id).unwrap(), "42");
    assert_eq!(serde_json::to_string(&str_id).unwrap(), "\"req-1\"");
}

#[test]
fn test_jsonrpc_request_deserialization() {
    let raw = r#"{"jsonrpc": "2.0", "id": 1, "method": "ping"}"#;
    let req: JsonRpcRequest = serde_json::from_str(raw).expect("Valid request");
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, Some(RequestId::Number(1)));
    assert_eq!(req.method, "ping");
}

#[test]
fn test_jsonrpc_success_response() {
    let resp = JsonRpcResponse::success(Some(RequestId::Number(1)), json!({"status": "ok"}));
    let serialized = serde_json::to_string(&resp).unwrap();
    assert!(serialized.contains("\"jsonrpc\":\"2.0\""));
    assert!(serialized.contains("\"id\":1"));
    assert!(serialized.contains("\"result\":{\"status\":\"ok\"}"));
    assert!(!serialized.contains("\"error\""));
}

#[test]
fn test_jsonrpc_error_response() {
    let err = JsonRpcError::new(-32601, "Method not found");
    let resp = JsonRpcResponse::error(Some(RequestId::String("abc".to_string())), err);
    let serialized = serde_json::to_string(&resp).unwrap();
    assert!(serialized.contains("\"code\":-32601"));
    assert!(serialized.contains("\"message\":\"Method not found\""));
    assert!(!serialized.contains("\"result\""));
}
