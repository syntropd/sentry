//! Decision-side error types: config, diagnosis, safety, circuits, top-level.

pub mod circuit_error;
pub mod config_error;
pub mod diagnostic_error;
pub mod safety_error;
pub mod sentry_error;

pub use circuit_error::CircuitError;
pub use config_error::ConfigError;
pub use diagnostic_error::DiagnosticError;
pub use safety_error::SafetyError;
pub use sentry_error::SentryError;
