//! Errors originating from cgroups v2 telemetry extraction.

use std::path::PathBuf;
use thiserror::Error;

/// Telemetry and cgroups extraction errors.
#[derive(Debug, Error)]
pub enum TelemetryError {
    /// Cgroup directory not found for unit.
    #[error("Cgroup not found for unit '{unit}' at path: {path}")]
    CgroupNotFound {
        /// Associated unit name.
        unit: String,
        /// Expected directory path.
        path: PathBuf,
    },

    /// Path exists but is not a directory.
    #[error("Expected cgroup path is not a directory: {0}")]
    NotADirectory(PathBuf),

    /// I/O error reading telemetry file.
    #[error("Failed reading telemetry file '{0}': {1}")]
    Io(&'static str, #[source] std::io::Error),

    /// Integer parse error for telemetry counter.
    #[error("Failed to parse integer from '{0}': {1}")]
    ParseInt(&'static str, #[source] std::num::ParseIntError),

    /// General I/O error.
    #[error("General I/O error in telemetry: {0}")]
    GeneralIo(#[from] std::io::Error),
}
