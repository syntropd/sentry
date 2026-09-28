//! Errors originating from subsystem drivers.

use crate::error::coredump_error::CoredumpError;
use crate::error::journal_error::JournalError;
use crate::error::psi_error::PsiError;
use crate::error::telemetry_error::TelemetryError;
use thiserror::Error;

/// Subsystem driver errors.
#[derive(Debug, Error)]
pub enum DriverError {
    /// sd_notify communication error.
    #[error("sd_notify failed: {0}")]
    NotifyFailed(String),

    /// Socket activation parsing error.
    #[error("Socket activation failed: {0}")]
    SocketActivation(String),

    /// Pure Rust D-Bus communication error.
    #[error("D-Bus driver error: {0}")]
    DbusError(String),

    /// Journal stream parser error.
    #[error("Journal ingestion error: {0}")]
    Journal(#[from] JournalError),

    /// Kernel cgroups v2 telemetry read error.
    #[error("Telemetry extraction error: {0}")]
    Telemetry(#[from] TelemetryError),

    /// PSI pressure telemetry read error.
    #[error("PSI extraction error: {0}")]
    Psi(#[from] PsiError),

    /// Coredump parsing error.
    #[error("Coredump extraction error: {0}")]
    Coredump(#[from] CoredumpError),

    /// Logind session discovery error.
    #[error("Logind driver error: {0}")]
    Logind(String),

    /// Standard I/O error.
    #[error("Underlying I/O error: {0}")]
    Io(#[from] std::io::Error),
}
