//! Asynchronous Varlink IPC client communicating with toold.

use super::reflect_error::AgentLoopError;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::{timeout, Duration};

const DEFAULT_TOOLD_SOCK: &str = "/run/syntrop/io.syntrop.Tool1";
const TIMEOUT: Duration = Duration::from_secs(5);

/// Client for invoking sandboxed diagnostic tools via toold.
#[derive(Debug, Clone)]
pub struct TooldClient {
    /// Filesystem path to the toold Unix domain socket.
    pub socket_path: PathBuf,
}

impl Default for TooldClient {
    fn default() -> Self {
        Self::new(DEFAULT_TOOLD_SOCK)
    }
}

impl TooldClient {
    /// Instantiate a toold client targeting the given socket path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: path.into(),
        }
    }

    /// Execute a tool via toold Varlink interface io.syntrop.Tool1.ExecuteTool.
    pub async fn execute_tool(&self, tool: &str, args: &[String]) -> Result<String, AgentLoopError> {
        if !self.socket_path.exists() {
            return Ok(format!(
                "[toold unavailable at {} - mock result for {} {:?}]",
                self.socket_path.display(),
                tool,
                args
            ));
        }

        let mut stream = match timeout(TIMEOUT, UnixStream::connect(&self.socket_path)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(AgentLoopError::ToolExecutionFailed(format!("socket error: {e}"))),
            Err(_) => return Err(AgentLoopError::ToolExecutionFailed("connection timed out".into())),
        };

        let req = json!({
            "method": "io.syntrop.Tool1.ExecuteTool",
            "parameters": {
                "name": tool,
                "args": args
            }
        });

        let mut payload = serde_json::to_vec(&req)
            .map_err(|e| AgentLoopError::ToolExecutionFailed(format!("serialize: {e}")))?;
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
            .map_err(|e| AgentLoopError::ToolExecutionFailed(format!("deserialize reply: {e}")))?;

        if let Some(stdout) = reply.pointer("/parameters/result/stdout").and_then(|v| v.as_str()) {
            Ok(stdout.to_string())
        } else if let Some(err) = reply.get("error").and_then(|e| e.as_str()) {
            Err(AgentLoopError::ToolExecutionFailed(err.to_string()))
        } else {
            Ok(reply.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_toold_client_fallback_when_offline() {
        let client = TooldClient::new("/nonexistent/toold.sock");
        let res = client.execute_tool("journal.slice", &["test".into()]).await;
        assert!(res.is_ok());
        assert!(res.unwrap().contains("mock result"));
    }
}
