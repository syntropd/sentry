//! Unified D-Bus domain event definitions.

/// High-level event emitted by the passive systemd D-Bus listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbusEvent {
    /// New unit loaded into memory.
    UnitNew {
        /// Unit identifier name (e.g. "nginx.service").
        id: String,
        /// D-Bus object path.
        path: String,
    },
    /// Unit unloaded from memory.
    UnitRemoved {
        /// Unit identifier name.
        id: String,
        /// D-Bus object path.
        path: String,
    },
    /// New job queued in systemd manager.
    JobNew {
        /// Job numeric ID.
        id: u32,
        /// D-Bus job path.
        job_path: String,
        /// Unit target.
        unit: String,
    },
    /// Job completed and removed.
    JobRemoved {
        /// Job numeric ID.
        id: u32,
        /// D-Bus job path.
        job_path: String,
        /// Unit target.
        unit: String,
        /// Job result status (e.g. "done", "failed").
        result: String,
    },
    /// Properties on a unit changed.
    UnitStateChanged(UnitStateUpdate),
    /// Critical failure transition detected for a unit.
    UnitFailed(UnitFailedEvent),
    /// Unit files reloaded or modified on disk.
    UnitFilesChanged,
    /// Systemd manager is reloading configuration.
    Reloading(bool),
}

/// Snapshot of changed properties for a unit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnitStateUpdate {
    /// Target unit name.
    pub unit: String,
    /// ActiveState property (e.g. "active", "failed", "inactive").
    pub active_state: Option<String>,
    /// SubState property (e.g. "running", "dead", "failed").
    pub sub_state: Option<String>,
    /// Unit failure result (e.g. "exit-code", "core-dump").
    pub result: Option<String>,
    /// Exit code or signal termination code.
    pub exec_main_code: Option<i32>,
    /// Exit status code.
    pub exec_main_status: Option<i32>,
    /// Main service process PID.
    pub main_pid: Option<u32>,
    /// cgroup path associated with unit.
    pub cgroup: Option<String>,
}

/// Failure event descriptor for failed units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitFailedEvent {
    /// Unit identifier.
    pub unit: String,
    /// Active state at failure.
    pub active_state: String,
    /// Sub state at failure.
    pub sub_state: String,
    /// Failure result string.
    pub result: Option<String>,
    /// Exec termination code.
    pub exec_code: Option<i32>,
    /// Exec exit status.
    pub exec_status: Option<i32>,
}
