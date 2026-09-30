//! Asynchronous Varlink IPC client communicating with runtimed.

use sentry_core::error::DiagnosticError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::{timeout, Duration};

const DEFAULT_RUNTIMED_SOCK: &str = "/run/syntrop/io.syntrop.Runtime1";
const TIMEOUT: Duration = Duration::from_secs(5);

/// Candidate option for Varlink Decide call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionCandidate {
    /// Canonical name of the option.
    pub name: String,
    /// Single token alias (e.g., "A", "B").
    pub token: String,
}

/// Raw decision candidate score returned from runtimed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredCandidate {
    /// Option name.
    pub name: String,
    /// Token alias.
    pub token: String,
    /// Token ID.
    pub token_id: u32,
    /// Raw unnormalized logit.
    pub logit: f32,
    /// Softmax probability.
    pub probability: f32,
}

/// Calibrated decision result from runtimed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawDecisionResult {
    /// Winning candidate name.
    pub winner: String,
    /// Calibrated confidence score S in [0.0, 1.0].
    pub confidence: f32,
    /// Uncalibrated top softmax probability.
    pub raw_probability: f32,
    /// Margin between top and second candidate.
    pub margin: f32,
    /// Normalized Shannon entropy.
    pub entropy: f32,
    /// All evaluated candidates with scores.
    pub candidates: Vec<ScoredCandidate>,
}

/// Client for calling io.syntrop.Runtime1.Decide over Varlink.
#[derive(Debug, Clone)]
pub struct SystemOneClient {
    /// Socket path to runtimed.
    pub socket_path: PathBuf,
}

impl Default for SystemOneClient {
    fn default() -> Self {
        let path = std::env::var("SYNTROP_RUNTIME_SOCKET")
            .or_else(|_| std::env::var("RUNTIMED_SOCKET"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_RUNTIMED_SOCK));
        Self { socket_path: path }
    }
}

impl SystemOneClient {
    /// Creates a new SystemOneClient targeting a specific socket path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: path.into(),
        }
    }

    /// Evaluates candidate classification options against prompt.
    pub async fn decide(
        &self,
        model: &str,
        prompt: &str,
        candidates: &[DecisionCandidate],
        temperature: f32,
    ) -> Result<RawDecisionResult, DiagnosticError> {
        let mut stream = match timeout(TIMEOUT, UnixStream::connect(&self.socket_path)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(DiagnosticError::ProviderUnavailable(format!(
                    "failed to connect to runtimed at {}: {e}",
                    self.socket_path.display()
                )));
            }
            Err(_) => {
                return Err(DiagnosticError::Timeout(TIMEOUT.as_millis() as u64));
            }
        };

        let req = json!({
            "method": "io.syntrop.Runtime1.Decide",
            "parameters": {
                "model": model,
                "prompt": prompt,
                "candidates": candidates,
                "temperature": temperature,
            }
        });

        let mut payload = serde_json::to_vec(&req)
            .map_err(|e| DiagnosticError::MalformedJson(format!("serialize request: {e}")))?;
        payload.push(0);

        stream
            .write_all(&payload)
            .await
            .map_err(|e| DiagnosticError::ProviderUnavailable(format!("write error: {e}")))?;
        stream
            .flush()
            .await
            .map_err(|e| DiagnosticError::ProviderUnavailable(format!("flush error: {e}")))?;

        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        while let Ok(1) = stream.read(&mut byte).await {
            if byte[0] == 0 {
                break;
            }
            buf.push(byte[0]);
        }

        let reply: Value = serde_json::from_slice(&buf)
            .map_err(|e| DiagnosticError::MalformedJson(format!("deserialize reply: {e}")))?;

        if let Some(err) = reply.get("error").and_then(|e| e.as_str()) {
            return Err(DiagnosticError::ProviderUnavailable(format!(
                "runtimed error: {err}"
            )));
        }

        let result_val = reply
            .pointer("/parameters/result")
            .ok_or_else(|| DiagnosticError::MalformedJson("missing parameters/result".into()))?;

        serde_json::from_value(result_val.clone())
            .map_err(|e| DiagnosticError::MalformedJson(format!("parse result: {e}")))
    }
}
