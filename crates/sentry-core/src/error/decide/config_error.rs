//! Errors originating from configuration loading and parsing.

use std::path::PathBuf;
use thiserror::Error;

/// Configuration errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Configuration file not found.
    #[error("Configuration file not found: {0}")]
    FileNotFound(PathBuf),

    /// Syntax or schema parse error in policy.toml.
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    /// Missing or invalid configuration value.
    #[error("Invalid setting '{key}': {reason}")]
    InvalidSetting {
        /// Configuration key name.
        key: String,
        /// Rationale.
        reason: String,
    },

    /// Underlying filesystem I/O error.
    #[error("I/O error reading configuration: {0}")]
    Io(#[from] std::io::Error),
}
