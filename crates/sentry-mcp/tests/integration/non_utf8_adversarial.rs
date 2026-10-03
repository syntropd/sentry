//! Adversarial stress test demonstrating non-UTF8 byte stream resilience.

use sentry_mcp::server::run_stdio_stream;
use sentry_mcp::storage::McpState;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn test_adversarial_non_utf8_stream_termination() {
    let state = McpState::new();
    let (client_write, server_read) = tokio::io::duplex(4096);
    let (server_write, client_read) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        let reader = BufReader::new(server_read);
        run_stdio_stream(state, reader, server_write).await
    });

    let mut writer = client_write;
    let mut reader = BufReader::new(client_read);

    // 1. Initial healthy ping
    writer
        .write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"ping\"}\n")
        .await
        .expect("write ping");
    writer.flush().await.expect("flush ping");

    let mut line = String::new();
    reader.read_line(&mut line).await.expect("read ping response");
    assert!(line.contains("\"result\":{}"), "Initial ping must succeed");

    // 2. Inject non-UTF8 raw bytes
    let non_utf8_payload = vec![0xFF, 0xFE, 0xFD, 0x80, b'\n'];
    writer
        .write_all(&non_utf8_payload)
        .await
        .expect("write non-utf8");
    writer.flush().await.expect("flush non-utf8");

    // 3. Verify server returns JSON-RPC 2.0 PARSE_ERROR (-32700)
    line.clear();
    let bytes_read = reader.read_line(&mut line).await.expect("read after non-utf8");
    assert!(bytes_read > 0, "Server must not close stream on non-UTF8 byte sequence");
    assert!(
        line.contains("\"code\":-32700"),
        "Server must return JSON-RPC 2.0 PARSE_ERROR (-32700), got: {line}"
    );

    // 4. Verify server is still alive and processes subsequent requests
    writer
        .write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 2, \"method\": \"ping\"}\n")
        .await
        .expect("write second ping");
    writer.flush().await.expect("flush second ping");

    line.clear();
    reader.read_line(&mut line).await.expect("read second ping response");
    assert!(line.contains("\"result\":{}"), "Subsequent ping must succeed after non-UTF8 recovery");

    // 5. Clean EOF shutdown
    drop(writer);
    let server_result = server_handle.await.expect("join server");
    assert!(server_result.is_ok(), "Server must exit cleanly on stream EOF");
}
