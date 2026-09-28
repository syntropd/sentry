//! Unit tests for bounded HTTP response body reader.

use reqwest::Client;
use sentry_diagnostic::provider::bounded_body::{
    read_bounded_bytes, read_bounded_json, read_bounded_text, MAX_HTTP_RESPONSE_BYTES,
};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct TestMessage {
    status: String,
}

#[tokio::test]
async fn test_bounded_bytes_and_json_success() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let body = r#"{"status":"ok"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = socket.write_all(response.as_bytes()).await;
    });

    let client = Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}"))
        .send()
        .await
        .unwrap();

    let parsed: TestMessage = read_bounded_json(resp, MAX_HTTP_RESPONSE_BYTES)
        .await
        .unwrap();
    assert_eq!(
        parsed,
        TestMessage {
            status: "ok".to_string()
        }
    );
}

#[tokio::test]
async fn test_bounded_bytes_exceeds_content_length() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nhello";
        let _ = socket.write_all(response.as_bytes()).await;
    });

    let client = Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}"))
        .send()
        .await
        .unwrap();

    let res = read_bounded_bytes(resp, 10).await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Content-Length"));
}

#[tokio::test]
async fn test_bounded_text_success() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let body = "error occurred on remote service";
        let response = format!(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = socket.write_all(response.as_bytes()).await;
    });

    let client = Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}"))
        .send()
        .await
        .unwrap();

    let text = read_bounded_text(resp, MAX_HTTP_RESPONSE_BYTES)
        .await
        .unwrap();
    assert_eq!(text, "error occurred on remote service");
}
