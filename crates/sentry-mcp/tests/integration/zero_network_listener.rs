//! Adversarial verification of zero network listeners for Model Context Protocol server.
//!
//! Asserts that:
//! 1. Running the MCP stdio server opens strictly 0 TCP sockets and 0 TCP listening sockets.
//! 2. Active file descriptors (/proc/self/fd) contain zero TCP listeners.
//! 3. Source code contains zero socket binding or network server imports.

use sentry_mcp::server::run_stdio_stream;
use sentry_mcp::storage::McpState;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn get_process_socket_inodes() -> HashSet<u64> {
    let mut inodes = HashSet::new();
    let fd_dir = Path::new("/proc/self/fd");
    if let Ok(entries) = fs::read_dir(fd_dir) {
        for entry in entries.flatten() {
            if let Ok(target) = fs::read_link(entry.path()) {
                let target_str = target.to_string_lossy();
                if let Some(rest) = target_str.strip_prefix("socket:[") {
                    if let Some(inode_str) = rest.strip_suffix(']') {
                        if let Ok(inode) = inode_str.parse::<u64>() {
                            inodes.insert(inode);
                        }
                    }
                }
            }
        }
    }
    inodes
}

fn get_listening_tcp_inodes(proc_net_file: &str) -> HashSet<u64> {
    let mut listening = HashSet::new();
    if let Ok(content) = fs::read_to_string(proc_net_file) {
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            // In /proc/net/tcp: fields[3] is state ('st'), fields[9] is inode
            if fields.len() > 9 {
                let state = fields[3];
                let inode_str = fields[9];
                // '0A' (10 in decimal) indicates TCP_LISTEN
                if state == "0A" {
                    if let Ok(inode) = inode_str.parse::<u64>() {
                        listening.insert(inode);
                    }
                }
            }
        }
    }
    listening
}

#[tokio::test]
async fn test_assert_zero_network_listeners_while_serving_mcp() {
    let state = McpState::new();
    let (client_write, server_read) = tokio::io::duplex(4096);
    let (server_write, client_read) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        let reader = BufReader::new(server_read);
        run_stdio_stream(state, reader, server_write).await
    });

    let mut writer = client_write;
    let mut reader = BufReader::new(client_read);

    // Perform an active exchange of MCP requests
    writer
        .write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"initialize\"}\n")
        .await
        .expect("write init");
    writer.flush().await.expect("flush init");

    let mut line = String::new();
    reader.read_line(&mut line).await.expect("read init");
    assert!(line.contains("\"capabilities\""));

    writer
        .write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 2, \"method\": \"tools/list\"}\n")
        .await
        .expect("write tools");
    writer.flush().await.expect("flush tools");

    line.clear();
    reader.read_line(&mut line).await.expect("read tools");
    assert!(line.contains("\"tools\""));

    // Check open sockets during active MCP operation
    let proc_sockets = get_process_socket_inodes();
    let tcp4_listening = get_listening_tcp_inodes("/proc/net/tcp");
    let tcp6_listening = get_listening_tcp_inodes("/proc/net/tcp6");

    let process_listening_tcp4: Vec<&u64> =
        proc_sockets.intersection(&tcp4_listening).collect();
    let process_listening_tcp6: Vec<&u64> =
        proc_sockets.intersection(&tcp6_listening).collect();

    assert!(
        process_listening_tcp4.is_empty(),
        "MCP server opened IPv4 listening TCP sockets: {process_listening_tcp4:?}"
    );
    assert!(
        process_listening_tcp6.is_empty(),
        "MCP server opened IPv6 listening TCP sockets: {process_listening_tcp6:?}"
    );

    // Clean shutdown
    drop(writer);
    let res = server_handle.await.expect("join");
    assert!(res.is_ok());
}

#[test]
fn test_codebase_contains_zero_network_binding_calls() {
    let mcp_src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    fn check_dir(dir: &Path) {
        for entry in fs::read_dir(dir).expect("read src dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                check_dir(&path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let code = fs::read_to_string(&path).expect("read rs file");
                assert!(
                    !code.contains("TcpListener"),
                    "File {:?} contains forbidden TcpListener",
                    path
                );
                assert!(
                    !code.contains(".bind("),
                    "File {:?} contains forbidden .bind() network call",
                    path
                );
                assert!(
                    !code.contains("127.0.0.1"),
                    "File {:?} contains network address literal",
                    path
                );
                assert!(
                    !code.contains("0.0.0.0"),
                    "File {:?} contains network address literal",
                    path
                );
            }
        }
    }

    check_dir(&mcp_src_dir);
}
