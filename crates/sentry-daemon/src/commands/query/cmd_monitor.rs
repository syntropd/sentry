//! Command: Stream live crash, trip, and remediation events from the daemon.

use crate::cli::exit_codes::{EX_OK, EX_UNAVAILABLE};
use crate::ipc::client::IpcClient;
use crate::ipc::protocol::{IpcRequest, IpcResponse};
use tokio::io::AsyncBufReadExt;

/// Execute the `monitor` subcommand.
pub async fn execute_monitor(socket_path: &str) -> i32 {
    let mut client = match IpcClient::connect(socket_path).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Unable to connect to daemon at {}: {}", socket_path, e);
            return EX_UNAVAILABLE;
        }
    };

    let sub_req = IpcRequest::SubscribeEvents;
    match client.send_request(&sub_req).await {
        Ok(IpcResponse::Ok { .. }) => {
            println!("Subscribed to systemd-sentry event stream. Press Ctrl+C to exit.\n");
        }
        Ok(IpcResponse::Error { code, message }) => {
            eprintln!("Failed to subscribe to daemon event stream (code {}): {}", code, message);
            return EX_UNAVAILABLE;
        }
        Err(e) => {
            eprintln!("Error communicating with daemon: {}", e);
            return EX_UNAVAILABLE;
        }
        _ => {
            eprintln!("Failed to subscribe to daemon event stream.");
            return EX_UNAVAILABLE;
        }
    }

    let mut reader = client.into_reader();
    let mut line = String::new();

    while let Ok(n) = reader.read_line(&mut line).await {
        if n == 0 {
            println!("\nDaemon closed connection.");
            break;
        }

        if let Ok(IpcResponse::Event { data }) = serde_json::from_str::<IpcResponse>(&line) {
            let unit = data.get("unit").and_then(|v| v.as_str()).unwrap_or("-");
            let severity = data.get("severity").and_then(|v| v.as_str()).unwrap_or("-");
            let cause = data.get("root_cause").and_then(|v| v.as_str()).unwrap_or("-");
            let action = data.get("action").and_then(|v| v.as_str()).unwrap_or("-");
            let circuit = data.get("circuit_state").and_then(|v| v.as_str()).unwrap_or("-");

            println!(
                "[{}] UNIT: {:<20} CAUSE: {:<30} ACTION: {:<15} CIRCUIT: {}",
                severity, unit, cause, action, circuit
            );
        }
        line.clear();
    }

    EX_OK
}
