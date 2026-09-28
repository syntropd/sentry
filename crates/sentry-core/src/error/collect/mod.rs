//! Telemetry-collection error types: drivers, journal, coredump, PSI.

pub mod coredump_error;
pub mod driver_error;
pub mod journal_error;
pub mod psi_error;
pub mod telemetry_error;

pub use coredump_error::CoredumpError;
pub use driver_error::DriverError;
pub use journal_error::JournalError;
pub use psi_error::PsiError;
pub use telemetry_error::TelemetryError;
