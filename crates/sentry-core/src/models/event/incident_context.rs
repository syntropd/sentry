//! Incident context bundle passed into diagnostic engine.

use crate::models::cgroup::CgroupTelemetry;
use crate::models::coredump::CoredumpRecord;
use crate::models::driver_event::DriverEvent;
use crate::models::psi::PressureTelemetry;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Comprehensive incident context collected across host subsystems.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncidentContext {
    /// Unique incident tracking identifier.
    pub incident_id: Uuid,
    /// Creation timestamp in UTC.
    pub timestamp: DateTime<Utc>,
    /// Target service unit name.
    pub unit: String,
    /// Triggering subsystem failure event.
    pub failure_event: DriverEvent,
    /// Recent journal logs relevant to unit.
    pub journal_lines: Vec<String>,
    /// Subsystem pressure telemetry at time of incident.
    pub telemetry: Option<PressureTelemetry>,
    /// Cgroup v2 resource accounting at time of incident.
    pub cgroup: Option<CgroupTelemetry>,
    /// Associated coredump record if crash occurred.
    pub coredump: Option<CoredumpRecord>,
}

impl IncidentContext {
    /// Initializes a fresh incident context with auto-generated UUID and UTC timestamp.
    pub fn new(unit: impl Into<String>, failure_event: DriverEvent) -> Self {
        Self {
            incident_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            unit: unit.into(),
            failure_event,
            journal_lines: Vec::new(),
            telemetry: None,
            cgroup: None,
            coredump: None,
        }
    }

    /// Appends journal lines.
    pub fn with_journal_lines(mut self, lines: Vec<String>) -> Self {
        self.journal_lines = lines;
        self
    }

    /// Attaches pressure telemetry.
    pub fn with_telemetry(mut self, telemetry: PressureTelemetry) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    /// Attaches cgroup telemetry.
    pub fn with_cgroup(mut self, cgroup: CgroupTelemetry) -> Self {
        self.cgroup = Some(cgroup);
        self
    }

    /// Attaches crash record.
    pub fn with_coredump(mut self, coredump: CoredumpRecord) -> Self {
        self.coredump = Some(coredump);
        self
    }
}
