//! Command: Query and display supervisor status and circuit states.

use crate::cli::exit_codes::{EX_OK, EX_UNAVAILABLE};
use crate::ipc::client::IpcClient;
use crate::ipc::protocol::{IpcRequest, IpcResponse};

/// Execute the `status` subcommand.
pub async fn execute_status(socket_path: &str, json: bool) -> i32 {
    let mut client = match IpcClient::connect(socket_path).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Error: Unable to connect to systemd-sentry daemon at {}: {}",
                socket_path, e
            );
            eprintln!("Ensure the daemon is running (`systemctl status systemd-sentry`).");
            return EX_UNAVAILABLE;
        }
    };

    match client.send_request(&IpcRequest::Status).await {
        Ok(IpcResponse::Ok { data }) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&data).unwrap_or_default());
            } else {
                print_status_table(&data);
            }
            EX_OK
        }
        Ok(IpcResponse::Error { code, message }) => {
            eprintln!("Daemon returned error (code {}): {}", code, message);
            code
        }
        Err(e) => {
            eprintln!("Error communicating with daemon: {}", e);
            EX_UNAVAILABLE
        }
        _ => EX_UNAVAILABLE,
    }
}

fn print_status_table(data: &serde_json::Value) {
    let uptime = data.get("uptime_seconds").and_then(|v| v.as_u64()).unwrap_or(0);
    let rss_mb = data.get("rss_mb").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let degraded = data.get("degraded_mode").and_then(|v| v.as_bool()).unwrap_or(false);
    let dropped = data.get("dropped_events").and_then(|v| v.as_u64()).unwrap_or(0);
    let tracked = data.get("tracked_units_count").and_then(|v| v.as_u64()).unwrap_or(0);

    println!("● systemd-sentry supervisor");
    println!("   Uptime:         {}s", uptime);
    println!("   Memory RSS:     {:.2} MB", rss_mb);
    println!("   State:          {}", if degraded { "DEGRADED (Load Shedding Active)" } else { "HEALTHY" });
    println!("   Dropped Events: {}", dropped);
    println!("   Tracked Units:  {}", tracked);

    if let Some(breakers_obj) = data.get("circuit_breakers").and_then(|v| v.as_object()) {
        if !breakers_obj.is_empty() {
            println!("\nCIRCUIT BREAKERS:");
            println!("{:<32} {:<12} {:<10} {:<8}", "UNIT", "STATE", "FAILURES", "LOCKED");
            println!("{:-<66}", "");
            let mut entries: Vec<_> = breakers_obj.iter().collect();
            entries.sort_by_key(|(k, _)| *k);
            for (unit, b) in entries {
                let state = b.get("state").and_then(|v| v.as_str()).unwrap_or("-");
                let fails = b
                    .get("recent_failures")
                    .or_else(|| b.get("failure_count"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let locked = if b.get("permanently_locked").and_then(|v| v.as_bool()).unwrap_or(false) {
                    "YES"
                } else if b.get("flap_count").and_then(|v| v.as_u64()).unwrap_or(0) > 0 {
                    "YES"
                } else {
                    "NO"
                };
                println!("{:<32} {:<12} {:<10} {:<8}", unit, state, fails, locked);
            }
        }
    } else if let Some(breakers) = data.get("circuit_breakers").and_then(|v| v.as_array()) {
        if !breakers.is_empty() {
            println!("\nCIRCUIT BREAKERS:");
            println!("{:<32} {:<12} {:<10} {:<8}", "UNIT", "STATE", "FAILURES", "LOCKED");
            println!("{:-<66}", "");
            for b in breakers {
                let unit = b
                    .get("unit_name")
                    .or_else(|| b.get("unit"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let state = b.get("state").and_then(|v| v.as_str()).unwrap_or("-");
                let fails = b
                    .get("recent_failures")
                    .or_else(|| b.get("failure_count"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let locked = if b.get("permanently_locked").and_then(|v| v.as_bool()).unwrap_or(false) {
                    "YES"
                } else if b.get("flap_count").and_then(|v| v.as_u64()).unwrap_or(0) > 0 {
                    "YES"
                } else {
                    "NO"
                };
                println!("{:<32} {:<12} {:<10} {:<8}", unit, state, fails, locked);
            }
        }
    }
}
