//! Command: List recent incidents recorded by the supervisor.

use crate::cli::exit_codes::{EX_OK, EX_SOFTWARE, EX_UNAVAILABLE};
use crate::ipc::client::IpcClient;
use crate::ipc::protocol::{IpcRequest, IpcResponse};

/// Execute the `incidents` subcommand.
pub async fn execute_incidents(socket_path: &str, limit: usize, json: bool) -> i32 {
    let mut client = match IpcClient::connect(socket_path).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Unable to connect to daemon at {}: {}", socket_path, e);
            return EX_UNAVAILABLE;
        }
    };

    match client.send_request(&IpcRequest::ListIncidents { limit }).await {
        Ok(IpcResponse::Ok { data }) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&data).unwrap_or_default());
            } else if let Some(items) = data.as_array() {
                if items.is_empty() {
                    println!("No incidents recorded.");
                } else {
                    println!("{:<36} {:<24} {:<10} {:<30}", "INCIDENT ID", "UNIT", "SEVERITY", "ROOT CAUSE");
                    println!("{:-<104}", "");
                    for item in items {
                        let id = item.get("incident_id").and_then(|v| v.as_str()).unwrap_or("-");
                        let unit = item.get("unit_name").and_then(|v| v.as_str()).unwrap_or("-");
                        let sev = item.get("severity").and_then(|v| v.as_str()).unwrap_or("-");
                        let cause = item
                            .get("root_cause")
                            .and_then(|v| {
                                v.as_str()
                                    .or_else(|| v.get("summary").and_then(|s| s.as_str()))
                            })
                            .unwrap_or("-");
                        println!("{:<36} {:<24} {:<10} {:<30}", id, unit, sev, cause);
                    }
                }
            }
            EX_OK
        }
        Ok(IpcResponse::Error { code, message }) => {
            eprintln!("Daemon returned error (code {}): {}", code, message);
            EX_SOFTWARE
        }
        Err(e) => {
            eprintln!("Error communicating with daemon: {}", e);
            EX_UNAVAILABLE
        }
        _ => EX_UNAVAILABLE,
    }
}
