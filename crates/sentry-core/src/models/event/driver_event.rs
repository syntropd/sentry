//! Driver subsystem event models.

use crate::models::cgroup::CgroupTelemetry;
use crate::models::coredump::CoredumpRecord;
use crate::models::psi::PressureTelemetry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Details of a unit failure captured via D-Bus properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitFailedDetails {
    /// Unit identifier (e.g. "postgres.service").
    pub unit: String,
    /// D-Bus ActiveState property (e.g. "failed").
    pub active_state: String,
    /// D-Bus SubState property (e.g. "failed", "dead").
    pub sub_state: String,
    /// D-Bus Result property (e.g. "core-dump", "exit-code").
    pub result: Option<String>,
    /// Process exit status integer.
    pub exec_status: Option<i32>,
    /// Main service PID.
    pub main_pid: Option<u32>,
}

/// A structured journal log record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntryDetails {
    /// Associated unit name if identified.
    pub unit: Option<String>,
    /// Primary log message content.
    pub message: String,
    /// Syslog priority level (0 = Emergency .. 7 = Debug).
    pub priority: u8,
    /// Realtime timestamp in microseconds.
    pub timestamp_usec: u64,
    /// Auxiliary journal fields (__CURSOR, _PID, _COMM, etc.).
    pub extra: HashMap<String, String>,
}

/// Subsystem event stream emitted by host drivers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DriverEvent {
    /// A systemd service or unit failed.
    UnitFailed(UnitFailedDetails),
    /// A structured journal entry was ingested.
    JournalEntry(JournalEntryDetails),
    /// A process crashed and produced a coredump.
    Coredump(CoredumpRecord),
    /// Kernel PSI pressure reading.
    Pressure(PressureTelemetry),
    /// Cgroup v2 resource usage reading.
    Cgroup(CgroupTelemetry),
}

impl DriverEvent {
    /// Extracts the unit name associated with the event, if present.
    pub fn unit_name(&self) -> Option<&str> {
        match self {
            Self::UnitFailed(details) => Some(&details.unit),
            Self::JournalEntry(details) => details.unit.as_deref(),
            Self::Coredump(record) => Some(&record.unit),
            Self::Pressure(telemetry) => telemetry.unit.as_deref(),
            Self::Cgroup(telemetry) => telemetry.unit.as_deref(),
        }
    }
}
