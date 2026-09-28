//! Errors originating from crash and coredump extraction.

use std::path::PathBuf;
use thiserror::Error;

/// Coredump extraction and analysis errors.
#[derive(Debug, Error)]
pub enum CoredumpError {
    /// Coredump file not found.
    #[error("Coredump file not found: {0}")]
    FileNotFound(PathBuf),

    /// Parsing failure on coredump data or attributes.
    #[error("Failed to parse coredump data: {0}")]
    ParseError(String),

    /// Underlying standard I/O error.
    #[error("I/O error during coredump extraction: {0}")]
    Io(#[from] std::io::Error),
}
