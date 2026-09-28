//! Command: Reset circuit breaker lockout for a unit.

use crate::cli::exit_codes::{EX_NOPERM, EX_OK, EX_SOFTWARE, EX_UNAVAILABLE};
use crate::ipc::client::IpcClient;
use crate::ipc::protocol::{IpcRequest, IpcResponse};

/// Execute the `reset` subcommand.
pub async fn execute_reset(socket_path: &str, unit: &str) -> i32 {
    let mut client = match IpcClient::connect(socket_path).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Unable to connect to daemon at {}: {}", socket_path, e);
            return EX_UNAVAILABLE;
        }
    };

    match client.send_request(&IpcRequest::ResetCircuit { unit: unit.to_string() }).await {
        Ok(IpcResponse::Ok { .. }) => {
            println!("Circuit breaker for unit '{}' has been reset successfully.", unit);
            EX_OK
        }
        Ok(IpcResponse::Error { code: -32003, message }) => {
            eprintln!("Permission denied: {}", message);
            eprintln!("Hint: Try running with `sudo systemd-sentry reset {}`", unit);
            EX_NOPERM
        }
        Ok(IpcResponse::Error { code, message }) => {
            eprintln!("Error resetting unit (code {}): {}", code, message);
            EX_SOFTWARE
        }
        Err(e) => {
            eprintln!("Error communicating with daemon: {}", e);
            EX_UNAVAILABLE
        }
        _ => EX_UNAVAILABLE,
    }
}
