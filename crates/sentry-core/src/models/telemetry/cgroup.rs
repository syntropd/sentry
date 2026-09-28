//! Linux cgroups v2 telemetry domain models.

use serde::{Deserialize, Serialize};

/// Cgroup v2 memory event counters (`memory.events`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MemoryEvents {
    /// Number of times processes entered low threshold.
    pub low: u64,
    /// Number of times processes crossed high threshold.
    pub high: u64,
    /// Number of times processes hit memory.max ceiling.
    pub max: u64,
    /// Number of OOM killer invocations.
    pub oom: u64,
    /// Number of processes killed by OOM killer.
    pub oom_kill: u64,
    /// Number of cgroup groups killed by OOM killer.
    pub oom_group_kill: u64,
}

/// Cgroup v2 memory statistics summary.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CgroupMemoryStats {
    /// Current memory usage in bytes.
    pub current: u64,
    /// Memory maximum ceiling in bytes (None if unlimited / "max").
    pub max: Option<u64>,
    /// Low memory event counter.
    pub low_events: u64,
    /// High memory event counter.
    pub high_events: u64,
    /// Max memory event counter.
    pub max_events: u64,
    /// OOM invocation event counter.
    pub oom_events: u64,
    /// OOM kill event counter.
    pub oom_kill_events: u64,
}

/// Cgroup v2 CPU usage statistics (`cpu.stat`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CpuStat {
    /// Total CPU time used in microseconds.
    pub usage_usec: u64,
    /// User CPU time in microseconds.
    pub user_usec: u64,
    /// System kernel CPU time in microseconds.
    pub system_usec: u64,
    /// Number of enforcement periods.
    pub nr_periods: u64,
    /// Number of throttled periods.
    pub nr_throttled: u64,
    /// Aggregate throttled duration in microseconds.
    pub throttled_usec: u64,
}

/// Alias for Cgroup CPU statistics.
pub type CgroupCpuStats = CpuStat;

/// IO throughput and operations per major:minor device (`io.stat`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IoDeviceMetrics {
    /// Device identifier formatted as "major:minor" (e.g. "259:0").
    pub device: String,
    /// Cumulative read bytes.
    pub rbytes: u64,
    /// Cumulative written bytes.
    pub wbytes: u64,
    /// Cumulative read IO operations.
    pub rios: u64,
    /// Cumulative write IO operations.
    pub wios: u64,
    /// Cumulative discarded bytes.
    pub dbytes: u64,
    /// Cumulative discard IO operations.
    pub dios: u64,
}

/// Alias for Cgroup IO device metrics.
pub type CgroupIoDeviceStats = IoDeviceMetrics;

/// Aggregate cgroup v2 telemetry snapshot for a unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CgroupTelemetry {
    /// Unit name associated with this cgroup.
    pub unit: Option<String>,
    /// Absolute cgroup hierarchy path (e.g. "/system.slice/nginx.service").
    pub cgroup_path: String,
    /// Current memory usage in bytes (`memory.current`).
    pub memory_current_bytes: Option<u64>,
    /// Configured memory ceiling (`memory.max`). `None` indicates unlimited.
    pub memory_max_bytes: Option<u64>,
    /// Memory event counters (`memory.events`).
    pub memory_events: MemoryEvents,
    /// CPU accounting statistics (`cpu.stat`).
    pub cpu_stat: CpuStat,
    /// Per-device IO statistics (`io.stat`).
    pub io_stats: Vec<IoDeviceMetrics>,
    /// Whether any processes currently reside in this cgroup.
    pub populated: Option<bool>,
    /// Whether the cgroup is currently frozen.
    pub frozen: Option<bool>,
    /// Telemetry sample timestamp in microseconds.
    pub timestamp_usec: u64,
    /// Whether this telemetry was synthetically derived or from procfs fallback.
    #[serde(default)]
    pub synthetic: bool,
}

impl CgroupTelemetry {
    /// Constructs a synthetic fallback `CgroupTelemetry` snapshot when cgroup v2 hierarchy is unavailable.
    pub fn synthetic_fallback(unit: Option<String>, path: Option<String>) -> Self {
        let cgroup_path = match path {
            Some(p) if p.contains("synthetic") => p,
            Some(p) => format!("{}/synthetic", p.trim_end_matches('/')),
            None => match &unit {
                Some(u) => format!("/sys/fs/cgroup/synthetic/{}", u),
                None => "/sys/fs/cgroup/synthetic".to_string(),
            },
        };
        let timestamp_usec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        Self {
            unit,
            cgroup_path,
            memory_current_bytes: None,
            memory_max_bytes: None,
            memory_events: MemoryEvents::default(),
            cpu_stat: CpuStat::default(),
            io_stats: Vec::new(),
            populated: None,
            frozen: None,
            timestamp_usec,
            synthetic: true,
        }
    }

    /// Returns true if this cgroup telemetry snapshot is synthetic or procfs fallback.
    pub fn is_synthetic(&self) -> bool {
        self.synthetic || self.cgroup_path.contains("synthetic") || self.cgroup_path.starts_with("/proc")
    }

    /// Checks if this cgroup suffered any OOM kills.
    pub fn had_oom_kill(&self) -> bool {
        self.memory_events.oom_kill > 0 || self.memory_events.oom > 0
    }

    /// Calculates memory utilization ratio (0.0 to 1.0) if a max ceiling is set.
    pub fn memory_utilization(&self) -> Option<f64> {
        match (self.memory_current_bytes, self.memory_max_bytes) {
            (Some(cur), Some(max)) if max > 0 => Some(cur as f64 / max as f64),
            _ => None,
        }
    }
}
