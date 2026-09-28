//! Command: Inspect full telemetry and diagnostic for a specific incident.

use crate::cli::exit_codes::{EX_DATAERR, EX_OK, EX_SOFTWARE, EX_UNAVAILABLE};
use crate::ipc::client::IpcClient;
use crate::ipc::protocol::{IpcRequest, IpcResponse};

/// Execute the `inspect` subcommand.
pub async fn execute_inspect(socket_path: &str, id: &str, json: bool) -> i32 {
    let mut client = match IpcClient::connect(socket_path).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Unable to connect to daemon at {}: {}", socket_path, e);
            return EX_UNAVAILABLE;
        }
    };

    match client.send_request(&IpcRequest::InspectIncident { id: id.to_string() }).await {
        Ok(IpcResponse::Ok { data }) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&data).unwrap_or_default());
            } else {
                println!("============================================================");
                println!(" INCIDENT INSPECTION: {}", id);
                println!("============================================================");
                let unit = data
                    .get("unit_name")
                    .or_else(|| data.get("context").and_then(|c| c.get("unit_name")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let timestamp = data
                    .get("timestamp")
                    .or_else(|| data.get("context").and_then(|c| c.get("failed_at")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let diag = data.get("diagnostic").unwrap_or(&data);
                let severity = diag.get("severity").and_then(|v| v.as_str()).unwrap_or("-");

                println!("Unit:       {}", unit);
                println!("Timestamp:  {}", timestamp);
                println!("Severity:   {}", severity);

                if let Some(rc) = diag.get("root_cause") {
                    let summary = rc.get("summary").and_then(|v| v.as_str()).unwrap_or("-");
                    let detail = rc
                        .get("detail")
                        .or_else(|| rc.get("description"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
                    println!("Root Cause: {}", summary);
                    println!("Details:    {}", detail);
                }

                if let Some(rem) = diag.get("proposed_remediation") {
                    println!("\nProposed Remediation:");
                    let action = rem.get("action").and_then(|v| v.as_str()).unwrap_or("-");
                    let rationale = rem.get("rationale").and_then(|v| v.as_str()).unwrap_or("-");
                    let conf = rem
                        .get("confidence")
                        .and_then(|v| v.as_f64())
                        .map(|c| format!("{:.2}", c))
                        .unwrap_or_else(|| "-".to_string());
                    println!("  Action:     {}", action);
                    println!("  Confidence: {}", conf);
                    println!("  Rationale:  {}", rationale);
                }

                if let Some(ev) = diag.get("evidence") {
                    println!("\nEvidence Collected:");
                    if let Some(code) = ev.get("exit_code").and_then(|v| v.as_i64()) {
                        println!("  Exit Code:  {}", code);
                    }
                    if let Some(sig) = ev.get("signal").and_then(|v| v.as_str()) {
                        println!("  Signal:     {}", sig);
                    }
                    if let Some(core) = ev.get("coredump").and_then(|v| v.as_str()) {
                        println!("  Coredump:   {}", core);
                    }
                }
                println!("============================================================");
            }
            EX_OK
        }
        Ok(IpcResponse::Error { code: 404, message }) => {
            eprintln!("Not found: {}", message);
            EX_DATAERR
        }
        Ok(IpcResponse::Error { code, message }) => {
            eprintln!("Error (code {}): {}", code, message);
            EX_SOFTWARE
        }
        Err(e) => {
            eprintln!("Error communicating with daemon: {}", e);
            EX_UNAVAILABLE
        }
        _ => EX_UNAVAILABLE,
    }
}
