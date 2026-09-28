//! Unified domain error for systemd-sentry.

use crate::error::circuit_error::CircuitError;
use crate::error::config_error::ConfigError;
use crate::error::coredump_error::CoredumpError;
use crate::error::diagnostic_error::DiagnosticError;
use crate::error::driver_error::DriverError;
use crate::error::journal_error::JournalError;
use crate::error::psi_error::PsiError;
use crate::error::safety_error::SafetyError;
use crate::error::telemetry_error::TelemetryError;
use thiserror::Error;

/// Unified top-level error type.
#[derive(Debug, Error)]
pub enum SentryError {
    /// Driver subsystem failure.
    #[error("Driver error: {0}")]
    Driver(#[from] DriverError),

    /// Journal parsing error.
    #[error("Journal error: {0}")]
    Journal(#[from] JournalError),

    /// Telemetry error.
    #[error("Telemetry error: {0}")]
    Telemetry(#[from] TelemetryError),

    /// PSI error.
    #[error("PSI error: {0}")]
    Psi(#[from] PsiError),

    /// Coredump error.
    #[error("Coredump error: {0}")]
    Coredump(#[from] CoredumpError),

    /// Diagnostic engine failure.
    #[error("Diagnostic error: {0}")]
    Diagnostic(#[from] DiagnosticError),

    /// Safety policy violation.
    #[error("Safety error: {0}")]
    Safety(#[from] SafetyError),

    /// Circuit breaker error.
    #[error("Circuit breaker error: {0}")]
    Circuit(#[from] CircuitError),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Standard I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
