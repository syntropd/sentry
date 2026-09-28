//! Pressure Stall Information (PSI) telemetry models.

use serde::{Deserialize, Serialize};

/// Metrics for a single PSI line (`some` or `full`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PsiLine {
    /// Percentage of time stalled over the last 10 seconds (0.00 .. 100.00).
    pub avg10: f64,
    /// Percentage of time stalled over the last 60 seconds (0.00 .. 100.00).
    pub avg60: f64,
    /// Percentage of time stalled over the last 300 seconds (0.00 .. 100.00).
    pub avg300: f64,
    /// Total accumulated stall time in microseconds.
    pub total_usec: u64,
}

impl PsiLine {
    /// Constructs a new `PsiLine`.
    pub const fn new(avg10: f64, avg60: f64, avg300: f64, total_usec: u64) -> Self {
        Self { avg10, avg60, avg300, total_usec }
    }

    /// Constructs a zeroed `PsiLine`.
    pub const fn zero() -> Self {
        Self {
            avg10: 0.0,
            avg60: 0.0,
            avg300: 0.0,
            total_usec: 0,
        }
    }

    /// Checks if the short-term pressure (avg10) exceeds a percentage threshold.
    pub fn is_stalled(&self, threshold_pct: f64) -> bool {
        self.avg10 >= threshold_pct
    }
}

/// A PSI record containing `some` and optional `full` stall lines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PsiRecord {
    /// Partial stall metrics (at least one task waiting).
    pub some: PsiLine,
    /// Complete stall metrics (all tasks waiting). Optional for CPU on Linux < 6.9.
    pub full: Option<PsiLine>,
}

impl PsiRecord {
    /// Constructs a new `PsiRecord`.
    pub const fn new(some: PsiLine, full: Option<PsiLine>) -> Self {
        Self { some, full }
    }

    /// Constructs a zeroed `PsiRecord`.
    pub const fn zero() -> Self {
        Self {
            some: PsiLine::zero(),
            full: Some(PsiLine::zero()),
        }
    }

    /// Checks if full stall is currently detected.
    pub fn is_fully_stalled(&self, threshold_pct: f64) -> bool {
        self.full.as_ref().map_or(false, |f| f.avg10 >= threshold_pct)
    }
}

/// Pressure telemetry bundle across CPU, Memory, and IO subsystems.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PressureTelemetry {
    /// Associated systemd unit name, if recorded for a specific cgroup.
    pub unit: Option<String>,
    /// CPU pressure stall metrics.
    pub cpu: PsiRecord,
    /// Memory pressure stall metrics.
    pub memory: PsiRecord,
    /// IO pressure stall metrics.
    pub io: PsiRecord,
    /// Capture timestamp in microseconds.
    pub timestamp_usec: u64,
    /// Whether this telemetry was synthetically derived rather than read from live PSI.
    #[serde(default)]
    pub synthetic: bool,
}

impl PressureTelemetry {
    /// Constructs a new `PressureTelemetry` snapshot.
    pub const fn new(
        unit: Option<String>,
        cpu: PsiRecord,
        memory: PsiRecord,
        io: PsiRecord,
        timestamp_usec: u64,
    ) -> Self {
        Self { unit, cpu, memory, io, timestamp_usec, synthetic: false }
    }

    /// Constructs a new synthetic `PressureTelemetry` snapshot.
    pub const fn new_synthetic(
        unit: Option<String>,
        cpu: PsiRecord,
        memory: PsiRecord,
        io: PsiRecord,
        timestamp_usec: u64,
    ) -> Self {
        Self { unit, cpu, memory, io, timestamp_usec, synthetic: true }
    }

    /// Constructs a synthetic baseline with zero pressure metrics.
    pub fn synthetic_zero(unit: Option<String>) -> Self {
        let timestamp_usec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        Self {
            unit,
            cpu: PsiRecord::zero(),
            memory: PsiRecord::zero(),
            io: PsiRecord::zero(),
            timestamp_usec,
            synthetic: true,
        }
    }

    /// Returns true if this telemetry snapshot was synthetically derived.
    pub fn is_synthetic(&self) -> bool {
        self.synthetic
    }

    /// Returns true if any subsystem exhibits critical pressure (> threshold_pct on avg10).
    pub fn has_critical_pressure(&self, threshold_pct: f64) -> bool {
        self.cpu.some.is_stalled(threshold_pct)
            || self.memory.some.is_stalled(threshold_pct)
            || self.io.some.is_stalled(threshold_pct)
    }
}
