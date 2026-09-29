//! Foundational domain models, telemetry primitives, and error definitions
//! for systemd-sentry.
//!
//! 100% pure Rust, zero unsafe code, and zero dynamic C library dependencies.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod error;
pub mod models;

pub use error::{
    CircuitError, ConfigError, CoredumpError, DiagnosticError, DriverError, JournalError,
    PsiError, SafetyError, SentryError, TelemetryError,
};
pub use models::{
    CgroupCpuStats, CgroupIoDeviceStats, CgroupMemoryStats, CgroupTelemetry, CoredumpRecord,
    CoredumpXattrs, CpuStat, DiagnosticPayload, DriverEvent, Evidence, IncidentContext,
    IoDeviceMetrics, JournalEntryDetails, MemoryEvents, PressureTelemetry, ProposedRemediation,
    PsiLine, PsiRecord, RemediationAction, RiskLevel, RootCause, Severity, UnitFailedDetails,
    DnsHealthState, NetworkOperationalState, PstorePanicReport, ShutdownState, TimeSyncStatus,
    GangCrashContext, GangErrorCategory, GangTriageVerdict, triage_gang_crash,
};
