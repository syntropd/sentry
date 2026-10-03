//! Protocol fuzzing test harness for MCP JSON-RPC 2.0 stdio transport.

use sentry_mcp::protocol::{
    INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR,
};
use sentry_mcp::server::run_stdio_stream;
use sentry_mcp::storage::McpState;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct TestDuplexHarness {
    writer: tokio::io::DuplexStream,
    reader: BufReader<tokio::io::DuplexStream>,
    server_handle: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

impl TestDuplexHarness {
    fn new(state: McpState) -> Self {
        let (client_write, server_read) = tokio::io::duplex(8192);
        let (server_write, client_read) = tokio::io::duplex(8192);

        let server_handle = tokio::spawn(async move {
            let reader = BufReader::new(server_read);
            run_stdio_stream(state, reader, server_write).await
        });

        Self {
            writer: client_write,
            reader: BufReader::new(client_read),
            server_handle,
        }
    }

    async fn send_and_receive(&mut self, request: &str) -> Value {
        self.writer
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("Failed to write request");
        self.writer.flush().await.expect("Failed to flush");

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .await
            .expect("Failed to read response");
        serde_json::from_str(&line).unwrap_or_else(|e| {
            panic!("Expected valid JSON response, got '{line}', err: {e}")
        })
    }

    async fn close(self) -> Result<(), std::io::Error> {
        drop(self.writer);
        self.server_handle.await.expect("Server task panicked")
    }
}

#[tokio::test]
async fn test_fuzz_malformed_json_frames() {
    let mut harness = TestDuplexHarness::new(McpState::new());

    // 1. Completely unparseable garbage
    let resp = harness.send_and_receive("NOT_JSON_AT_ALL").await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(resp["id"].is_null());
    assert_eq!(resp["error"]["code"], PARSE_ERROR);

    // 2. Truncated / unclosed JSON object
    let resp = harness.send_and_receive(r#"{"jsonrpc": "2.0", "id": 1, "method":"#).await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["error"]["code"], PARSE_ERROR);

    // 3. Missing required jsonrpc field
    let resp = harness.send_and_receive(r#"{"id": 2, "method": "ping"}"#).await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["error"]["code"], PARSE_ERROR);

    // 4. Invalid jsonrpc version
    let resp = harness
        .send_and_receive(r#"{"jsonrpc": "1.0", "id": 3, "method": "ping"}"#)
        .await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 3);
    assert_eq!(resp["error"]["code"], INVALID_REQUEST);

    // 5. Missing required params for tools/call
    let resp = harness
        .send_and_receive(r#"{"jsonrpc": "2.0", "id": 4, "method": "tools/call"}"#)
        .await;
    assert_eq!(resp["id"], 4);
    assert_eq!(resp["error"]["code"], INVALID_PARAMS);

    // 6. Missing required uri param for resources/read
    let resp = harness
        .send_and_receive(r#"{"jsonrpc": "2.0", "id": 5, "method": "resources/read"}"#)
        .await;
    assert_eq!(resp["id"], 5);
    assert_eq!(resp["error"]["code"], INVALID_PARAMS);

    // 7. Missing required name param for prompts/get
    let resp = harness
        .send_and_receive(r#"{"jsonrpc": "2.0", "id": 6, "method": "prompts/get"}"#)
        .await;
    assert_eq!(resp["id"], 6);
    assert_eq!(resp["error"]["code"], INVALID_PARAMS);

    assert!(harness.close().await.is_ok());
}

#[tokio::test]
async fn test_fuzz_unknown_methods() {
    let mut harness = TestDuplexHarness::new(McpState::new());

    for method in &["admin/reboot", "sys_exec", "eval", "invalid_call_42", ""] {
        let req = format!(r#"{{"jsonrpc": "2.0", "id": 100, "method": "{method}"}}"#);
        let resp = harness.send_and_receive(&req).await;
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 100);
        assert_eq!(resp["error"]["code"], METHOD_NOT_FOUND);
    }

    assert!(harness.close().await.is_ok());
}

#[tokio::test]
async fn test_fuzz_negative_and_extreme_request_ids() {
    let mut harness = TestDuplexHarness::new(McpState::new());

    // Negative ID in success ping
    let resp = harness
        .send_and_receive(r#"{"jsonrpc": "2.0", "id": -1, "method": "ping"}"#)
        .await;
    assert_eq!(resp["id"], -1);
    assert!(resp["result"].is_object());

    // Negative ID in error method not found
    let resp = harness
        .send_and_receive(r#"{"jsonrpc": "2.0", "id": -99999999, "method": "foo"}"#)
        .await;
    assert_eq!(resp["id"], -99999999);
    assert_eq!(resp["error"]["code"], METHOD_NOT_FOUND);

    // Extreme boundary i64::MIN (-9223372036854775808)
    let min_i64_req = format!(
        r#"{{"jsonrpc": "2.0", "id": {}, "method": "ping"}}"#,
        i64::MIN
    );
    let resp = harness.send_and_receive(&min_i64_req).await;
    assert_eq!(resp["id"], i64::MIN);
    assert!(resp["result"].is_object());

    assert!(harness.close().await.is_ok());
}

#[tokio::test]
async fn test_fuzz_oversized_batches_and_recovery() {
    let mut harness = TestDuplexHarness::new(McpState::new());

    // Single array batch
    let resp = harness
        .send_and_receive(r#"[{"jsonrpc": "2.0", "id": 1, "method": "ping"}]"#)
        .await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["error"]["code"], PARSE_ERROR);

    // Massive batch array of 500 items
    let mut batch_items = Vec::new();
    for i in 0..500 {
        batch_items.push(format!(r#"{{"jsonrpc": "2.0", "id": {i}, "method": "ping"}}"#));
    }
    let massive_batch = format!("[{}]", batch_items.join(","));

    let resp = harness.send_and_receive(&massive_batch).await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["error"]["code"], PARSE_ERROR);

    // Server must NOT deadlock or panic: verify immediate recovery on subsequent valid ping
    let ping_resp = harness
        .send_and_receive(r#"{"jsonrpc": "2.0", "id": 888, "method": "ping"}"#)
        .await;
    assert_eq!(ping_resp["id"], 888);
    assert!(ping_resp["result"].is_object());

    assert!(harness.close().await.is_ok());
}

#[tokio::test]
async fn test_fuzz_rapid_garbage_storm_never_deadlocks() {
    let mut harness = TestDuplexHarness::new(McpState::new());

    // Send 100 rapid malformed frames
    for i in 0..100 {
        let garbage = format!("{{garbage_frame_{i}: true,,}}");
        let resp = harness.send_and_receive(&garbage).await;
        assert_eq!(resp["error"]["code"], PARSE_ERROR);
    }

    // Verify server remains fully functional
    let ping_resp = harness
        .send_and_receive(r#"{"jsonrpc": "2.0", "id": 999, "method": "ping"}"#)
        .await;
    assert_eq!(ping_resp["id"], 999);

    assert!(harness.close().await.is_ok());
}
