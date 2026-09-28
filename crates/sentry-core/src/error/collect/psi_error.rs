//! Errors originating from Pressure Stall Information (PSI) parsing.

use thiserror::Error;

/// PSI parsing errors.
#[derive(Debug, Error)]
pub enum PsiError {
    /// Invalid line format encountered in PSI file.
    #[error("Invalid PSI line format: '{0}'")]
    InvalidLineFormat(String),

    /// Unknown line prefix encountered (expected 'some' or 'full').
    #[error("Unknown PSI line prefix: '{0}' (expected 'some' or 'full')")]
    UnknownLinePrefix(String),

    /// Missing expected metric field in line.
    #[error("Missing PSI field '{0}' in line: '{1}'")]
    MissingField(&'static str, String),

    /// Record does not contain required 'some' line.
    #[error("Missing required 'some' line in PSI record")]
    MissingSomeLine,

    /// Underling I/O error reading PSI proc/cgroup file.
    #[error("I/O error reading PSI: {0}")]
    Io(#[from] std::io::Error),
}
