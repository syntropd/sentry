//! Unit tests for stdio streaming execution over duplex async channels.

use sentry_mcp::server::run_stdio_stream;
use sentry_mcp::storage::McpState;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn test_stdio_streaming_full_session() {
    let state = McpState::new();

    // Setup duplex channels simulating stdin and stdout
    let (client_write, server_read) = tokio::io::duplex(4096);
    let (server_write, client_read) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        let reader = BufReader::new(server_read);
        run_stdio_stream(state, reader, server_write).await
    });

    let mut client_writer = client_write;
    let mut client_reader = BufReader::new(client_read);

    // 1. Send initialize
    client_writer
        .write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"initialize\"}\n")
        .await
        .unwrap();

    let mut line = String::new();
    client_reader.read_line(&mut line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["id"], 1);
    assert!(resp["result"]["capabilities"].is_object());

    // 2. Send initialized notification (no response expected)
    client_writer
        .write_all(b"{\"jsonrpc\": \"2.0\", \"method\": \"notifications/initialized\"}\n")
        .await
        .unwrap();

    // 3. Send ping
    client_writer
        .write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 2, \"method\": \"ping\"}\n")
        .await
        .unwrap();

    line.clear();
    client_reader.read_line(&mut line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["id"], 2);
    assert_eq!(resp["result"], serde_json::json!({}));

    // 4. Close client writer to trigger EOF and shutdown server
    drop(client_writer);
    let server_res = server_handle.await.unwrap();
    assert!(server_res.is_ok());
}
