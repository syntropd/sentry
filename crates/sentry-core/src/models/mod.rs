//! Domain models module re-exports.

pub mod agent;
pub mod event;
pub mod telemetry;
pub mod verdict;

pub use event::coredump;
pub use event::driver_event;
pub use event::incident_context;
pub use telemetry::cgroup;
pub use telemetry::psi;
pub use telemetry::system_state;
pub use verdict::diagnostic;
pub use verdict::remediation;
pub use verdict::severity;

pub use agent::{AgentAction, AgentPlan, AgentVerdict, StepRecord};
pub use event::coredump::{CoredumpRecord, CoredumpXattrs};
pub use event::driver_event::{DriverEvent, JournalEntryDetails, UnitFailedDetails};
pub use event::incident_context::IncidentContext;
pub use telemetry::cgroup::{
    CgroupCpuStats, CgroupIoDeviceStats, CgroupMemoryStats, CgroupTelemetry, CpuStat,
    IoDeviceMetrics, MemoryEvents,
};
pub use telemetry::psi::{PressureTelemetry, PsiLine, PsiRecord};
pub use telemetry::system_state::{
    DnsHealthState, NetworkOperationalState, PstorePanicReport, ShutdownState, TimeSyncStatus,
};
pub use verdict::diagnostic::{
    DiagnosticPayload, Evidence, ProposedRemediation, RiskLevel, RootCause,
};
pub use verdict::remediation::RemediationAction;
pub use verdict::severity::Severity;
pub use verdict::gang_triage::{
    triage_gang_crash, GangCrashContext, GangErrorCategory, GangTriageVerdict,
};
pub use verdict::decision::{DecisionTier, FaultClass, SystemOneVerdict};
pub use verdict::decision;
