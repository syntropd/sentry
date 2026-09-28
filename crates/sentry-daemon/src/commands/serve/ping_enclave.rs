//! Ping inferenced's emergency triage enclave before diagnosing.
//!
//! The enclave holds one protected compute slice for crash diagnosis so
//! triage survives even a wedged GPU. The ping carries the incident id;
//! inferenced isolates the slice (preempting squatters) and answers
//! `accepted` with the assigned plane. When the enclave is absent the
//! diagnosis proceeds unprotected (fail-open): triage must work
//! standalone, since it runs while the system is already broken.

use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::timeout;

/// Live enclave socket (matches inferenced's own daemon default).
pub const DEFAULT_TRIAGE_SOCKET: &str = "/run/syntrop/sentry.sock";
/// Override for tests and non-standard installs.
pub const TRIAGE_SOCKET_ENV: &str = "SYNTROP_TRIAGE_SOCKET";
const RPC_TIMEOUT: Duration = Duration::from_millis(500);

/// Outcome of the pre-diagnosis enclave ping.
pub enum EnclaveStatus {
    /// Enclave accepted and isolated compute on `plane`.
    Protected {
        /// Plane the enclave assigned to this incident.
        plane: String,
    },
    /// No protection (`reason`); diagnosis proceeds standalone.
    Unprotected {
        /// Why no protection was granted.
        reason: String,
    },
}

/// Resolve the enclave socket from the env override or the default path.
pub fn triage_socket_path() -> PathBuf {
    std::env::var(TRIAGE_SOCKET_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TRIAGE_SOCKET))
}

/// Ping the enclave; never fails, reports protection status instead.
pub async fn ping_triage_enclave(incident_id: &str, unit: &str) -> EnclaveStatus {
    let socket = triage_socket_path();
    if !socket.exists() {
        tracing::info!("triage enclave absent; diagnosing unprotected");
        return EnclaveStatus::Unprotected { reason: "enclave socket absent".into() };
    }
    match ping_once(&socket, incident_id, unit).await {
        Ok(plane) => {
            tracing::info!(plane = %plane, "triage enclave accepted; compute isolated");
            EnclaveStatus::Protected { plane }
        }
        Err(reason) => {
            tracing::warn!("triage enclave unreachable ({reason}); diagnosing unprotected");
            EnclaveStatus::Unprotected { reason }
        }
    }
}

async fn ping_once(socket: &PathBuf, incident_id: &str, unit: &str) -> Result<String, String> {
    let mut stream = timeout(RPC_TIMEOUT, UnixStream::connect(socket))
        .await
        .map_err(|_| "connect timed out".to_string())?
        .map_err(|e| e.to_string())?;
    let req = serde_json::to_vec(&json!({
        "action": "triage_ping",
        "incident_id": incident_id,
        "unit_name": unit,
    }))
    .map_err(|e| e.to_string())?;
    timeout(RPC_TIMEOUT, stream.write_all(&req))
        .await
        .map_err(|_| "write timed out".to_string())?
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 65536];
    let n = timeout(RPC_TIMEOUT, stream.read(&mut buf))
        .await
        .map_err(|_| "read timed out".to_string())?
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("enclave closed connection".to_string());
    }
    let reply: serde_json::Value =
        serde_json::from_slice(&buf[..n]).map_err(|e| format!("bad reply: {e}"))?;
    if reply.get("status").and_then(|v| v.as_str()) != Some("accepted") {
        return Err(format!("enclave declined: {}", reply.get("status").and_then(|v| v.as_str()).unwrap_or("?")));
    }
    reply
        .get("allocated_plane")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| "reply carried no plane".to_string())
}
