//! Domain models representing external systemd subsystem states.
//!
//! Tracks states for systemd-inhibit, resolved, networkd, timesyncd, and pstore.

use serde::{Deserialize, Serialize};

/// Host shutdown or sleep transition state from systemd-logind/inhibit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShutdownState {
    /// Normal operation: host is neither shutting down nor sleeping.
    #[default]
    Normal,
    /// Host is entering shutdown or reboot sequence.
    PreparingForShutdown,
    /// Host is entering suspend or hibernation.
    PreparingForSleep,
}

impl ShutdownState {
    /// Returns true if host lifecycle transition is in progress.
    pub fn is_transitioning(&self) -> bool {
        !matches!(self, Self::Normal)
    }
}

/// DNS health state derived from systemd-resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DnsHealthState {
    /// DNS resolution is operational across active interfaces.
    #[default]
    Operational,
    /// Resolution is degraded or failing queries.
    Degraded,
    /// No DNS servers configured or daemon unreachable.
    Unavailable,
}

/// Network operational state derived from systemd-networkd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NetworkOperationalState {
    /// Host has routable connectivity.
    #[default]
    Routable,
    /// One or more links are degraded.
    Degraded,
    /// Carrier is present but address not yet routable.
    Carrier,
    /// Physical link or carrier is lost.
    NoCarrier,
    /// Network interfaces are powered off or administratively down.
    Off,
    /// State is unknown or networkd is not running.
    Unknown,
}

impl NetworkOperationalState {
    /// Parses string state from systemd-networkd D-Bus property.
    pub fn from_dbus_str(s: &str) -> Self {
        match s.trim() {
            "routable" => Self::Routable,
            "degraded" => Self::Degraded,
            "carrier" => Self::Carrier,
            "no-carrier" => Self::NoCarrier,
            "off" => Self::Off,
            _ => Self::Unknown,
        }
    }

    /// Returns true if network is able to route packets.
    pub fn is_routable(&self) -> bool {
        matches!(self, Self::Routable)
    }
}

/// Time synchronization state from systemd-timesyncd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TimeSyncStatus {
    /// System clock is synchronized via NTP.
    #[default]
    Synchronized,
    /// Clock is unsynchronized; time drift may occur.
    Unsynchronized,
    /// Clock skew or leap second transition detected.
    DriftDetected,
}

/// Post-mortem report extracted from systemd-pstore kernel panic files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PstorePanicReport {
    /// Path of the source pstore file (e.g. /sys/fs/pstore/dmesg-efi-12345).
    pub source_path: String,
    /// Header or primary error reason (e.g. "Kernel panic - not syncing").
    pub summary: String,
    /// Top stack frame lines extracted from kernel backtrace.
    pub backtrace_snippet: Vec<String>,
}
