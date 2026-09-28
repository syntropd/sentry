//! Granular error types using `thiserror`.

pub mod collect;
pub mod decide;

pub use collect::coredump_error;
pub use collect::driver_error;
pub use collect::journal_error;
pub use collect::psi_error;
pub use collect::telemetry_error;
pub use decide::circuit_error;
pub use decide::config_error;
pub use decide::diagnostic_error;
pub use decide::safety_error;
pub use decide::sentry_error;

pub use collect::coredump_error::CoredumpError;
pub use collect::driver_error::DriverError;
pub use collect::journal_error::JournalError;
pub use collect::psi_error::PsiError;
pub use collect::telemetry_error::TelemetryError;
pub use decide::circuit_error::CircuitError;
pub use decide::config_error::ConfigError;
pub use decide::diagnostic_error::DiagnosticError;
pub use decide::safety_error::SafetyError;
pub use decide::sentry_error::SentryError;
