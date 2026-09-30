//! Asynchronous Varlink IPC client communicating with contextd.

use super::reflect_error::AgentLoopError;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::{timeout, Duration};

const DEFAULT_CONTEXTD_SOCK: &str = "/run/syntrop/io.syntrop.Context1";
const TIMEOUT: Duration = Duration::from_secs(5);

/// Client for semantic incident and historical context retrieval via contextd.
#[derive(Debug, Clone)]
pub struct ContextdClient {
    /// Filesystem path to contextd Unix domain socket.
    pub socket_path: PathBuf,
}

impl Default for ContextdClient {
    fn default() -> Self {
        Self::new(DEFAULT_CONTEXTD_SOCK)
    }
}

impl ContextdClient {
    /// Instantiate a contextd client targeting the given socket path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: path.into(),
        }
    }

    /// Query relevant historical incidents or documentation matches from contextd.
    pub async fn query_context(&self, query: &str) -> Result<String, AgentLoopError> {
        if !self.socket_path.exists() {
            return Ok(format!(
                "[contextd offline at {} - mock context for '{}']",
                self.socket_path.display(),
                query
            ));
        }

        let mut stream = match timeout(TIMEOUT, UnixStream::connect(&self.socket_path)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(AgentLoopError::ContextRetrievalFailed(format!("socket error: {e}"))),
            Err(_) => return Err(AgentLoopError::ContextRetrievalFailed("connection timed out".into())),
        };

        let unit = if let Some(pos) = query.find("unit ") {
            query[pos + 5..].split_whitespace().next().unwrap_or(query)
        } else if let Some(w) = query.split_whitespace().find(|w| w.contains('.')) {
            w
        } else {
            query.trim()
        };

        let req = json!({
            "method": "io.syntrop.Context1.GetUnitContext",
            "parameters": {
                "unit": unit,
                "since_seconds": 3600
            }
        });

        let mut payload = serde_json::to_vec(&req)
            .map_err(|e| AgentLoopError::ContextRetrievalFailed(format!("serialize: {e}")))?;
        payload.push(0);

        stream.write_all(&payload).await?;
        stream.flush().await?;

        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        while let Ok(1) = stream.read(&mut byte).await {
            if byte[0] == 0 {
                break;
            }
            buf.push(byte[0]);
        }

        let reply: Value = serde_json::from_slice(&buf)
            .map_err(|e| AgentLoopError::ContextRetrievalFailed(format!("deserialize reply: {e}")))?;

        if let Some(context) = reply.pointer("/parameters/context") {
            let mut parts = Vec::new();
            if let Some(summary) = context.get("summary").and_then(|s| s.as_str()) {
                parts.push(format!("Summary: {}", summary));
            }
            if let Some(diffs) = context.get("config_diffs").and_then(|d| d.as_array()) {
                if !diffs.is_empty() {
                    parts.push(format!("Configuration changes: {}", diffs.len()));
                }
            }
            if let Some(pkgs) = context.get("package_upgrades").and_then(|p| p.as_array()) {
                if !pkgs.is_empty() {
                    parts.push(format!("Package upgrades: {}", pkgs.len()));
                }
            }
            if parts.is_empty() {
                Ok(context.to_string())
            } else {
                Ok(parts.join("\n"))
            }
        } else if let Some(err) = reply.get("error").and_then(|e| e.as_str()) {
            Err(AgentLoopError::ContextRetrievalFailed(err.to_string()))
        } else {
            Ok(reply.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_contextd_client_fallback_when_offline() {
        let client = ContextdClient::new("/nonexistent/contextd.sock");
        let res = client.query_context("OOM memory leak").await;
        assert!(res.is_ok());
        assert!(res.unwrap().contains("mock context"));
    }
}
