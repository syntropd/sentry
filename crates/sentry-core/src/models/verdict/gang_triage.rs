//! Multi-GPU crash isolation and gang scheduling triage.

use super::remediation::RemediationAction;
use super::severity::Severity;
use serde::{Deserialize, Serialize};

/// Error category detected during multi-device gang execution failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GangErrorCategory {
    /// Kernel DRM / GPU driver hang or ring timeout.
    DriverHang,
    /// Out-of-memory or VRAM allocation exhaustion on device.
    MemoryExhaustion,
    /// Inter-device DMA / NVLink transfer timeout or link drop.
    PeerDmaTimeout,
    /// Worker process crash or SIGSEGV during execution.
    ProcessCrash,
    /// Bus error or uncorrectable ECC memory error.
    HardwareBusError,
}

/// Contextual details of a failed multi-device gang lease or pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GangCrashContext {
    /// Unique identifier of the executing gang.
    pub gang_id: String,
    /// Associated composite lease ID if assigned.
    pub lease_id: Option<String>,
    /// All device paths participating in the gang.
    pub member_devices: Vec<String>,
    /// Specific device path where failure was triggered.
    pub failed_device: String,
    /// Category of error detected.
    pub error_category: GangErrorCategory,
    /// Summary of journal lines capturing the fault.
    pub raw_journal_summary: String,
}

/// Triage outcome isolating healthy devices and recommending remediation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GangTriageVerdict {
    /// Identifier of the evaluated gang.
    pub gang_id: String,
    /// Device identified as faulty.
    pub faulty_device: String,
    /// Healthy devices that can be safely recovered and re-leased.
    pub healthy_devices: Vec<String>,
    /// Assessed severity level of the crash.
    pub severity: Severity,
    /// Recommended remediation action for the service unit.
    pub recommended_action: RemediationAction,
    /// Human-readable explanation and operational guidance.
    pub explanation: String,
}

/// Perform automated crash isolation for a multi-GPU gang execution failure.
pub fn triage_gang_crash(ctx: &GangCrashContext) -> GangTriageVerdict {
    let mut healthy = Vec::new();
    for dev in &ctx.member_devices {
        if dev != &ctx.failed_device && !healthy.contains(dev) {
            healthy.push(dev.clone());
        }
    }

    let (severity, action, explanation) = match ctx.error_category {
        GangErrorCategory::HardwareBusError => (
            Severity::Critical,
            RemediationAction::EscalateToAdmin,
            format!(
                "Uncorrectable hardware or bus error on {}. Isolated {} healthy devices.",
                ctx.failed_device,
                healthy.len()
            ),
        ),
        GangErrorCategory::DriverHang => (
            Severity::High,
            RemediationAction::RestartWithBackoff,
            format!(
                "DRM driver hang on device {}. Quarantining device and releasing gang.",
                ctx.failed_device
            ),
        ),
        GangErrorCategory::MemoryExhaustion => (
            Severity::Medium,
            RemediationAction::Restart,
            format!(
                "VRAM allocation exhaustion on {}. Gang can be rescheduled with reduced batch.",
                ctx.failed_device
            ),
        ),
        GangErrorCategory::PeerDmaTimeout => (
            Severity::High,
            RemediationAction::RestartWithBackoff,
            format!(
                "Peer DMA activation timeout involving {}. Resetting link interconnect.",
                ctx.failed_device
            ),
        ),
        GangErrorCategory::ProcessCrash => (
            Severity::Medium,
            RemediationAction::Restart,
            format!(
                "Process terminated unexpectedly on {}. Restarting gang worker.",
                ctx.failed_device
            ),
        ),
    };

    GangTriageVerdict {
        gang_id: ctx.gang_id.clone(),
        faulty_device: ctx.failed_device.clone(),
        healthy_devices: healthy,
        severity,
        recommended_action: action,
        explanation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gang_triage_driver_hang_isolates_healthy() {
        let ctx = GangCrashContext {
            gang_id: "gang-101".into(),
            lease_id: Some("lease-abc".into()),
            member_devices: vec!["/dev/dri/card0".into(), "/dev/dri/card1".into()],
            failed_device: "/dev/dri/card0".into(),
            error_category: GangErrorCategory::DriverHang,
            raw_journal_summary: "GPU hang detected on card0".into(),
        };

        let verdict = triage_gang_crash(&ctx);
        assert_eq!(verdict.gang_id, "gang-101");
        assert_eq!(verdict.faulty_device, "/dev/dri/card0");
        assert_eq!(verdict.healthy_devices, vec!["/dev/dri/card1"]);
        assert_eq!(verdict.severity, Severity::High);
        assert_eq!(verdict.recommended_action, RemediationAction::RestartWithBackoff);
    }

    #[test]
    fn test_gang_triage_hardware_bus_error_escalates() {
        let ctx = GangCrashContext {
            gang_id: "gang-102".into(),
            lease_id: None,
            member_devices: vec!["/dev/dri/card1".into()],
            failed_device: "/dev/dri/card1".into(),
            error_category: GangErrorCategory::HardwareBusError,
            raw_journal_summary: "Uncorrectable ECC error on card1".into(),
        };

        let verdict = triage_gang_crash(&ctx);
        assert_eq!(verdict.faulty_device, "/dev/dri/card1");
        assert!(verdict.healthy_devices.is_empty());
        assert_eq!(verdict.severity, Severity::Critical);
        assert_eq!(verdict.recommended_action, RemediationAction::EscalateToAdmin);
    }

    #[test]
    fn test_gang_triage_other_categories() {
        let ctx_mem = GangCrashContext {
            gang_id: "gang-103".into(),
            lease_id: None,
            member_devices: vec!["/dev/dri/card0".into()],
            failed_device: "/dev/dri/card0".into(),
            error_category: GangErrorCategory::MemoryExhaustion,
            raw_journal_summary: "CUDA out of memory".into(),
        };
        assert_eq!(triage_gang_crash(&ctx_mem).recommended_action, RemediationAction::Restart);

        let ctx_dma = GangCrashContext {
            gang_id: "gang-104".into(),
            lease_id: None,
            member_devices: vec!["/dev/dri/card0".into(), "/dev/dri/card1".into()],
            failed_device: "/dev/dri/card0".into(),
            error_category: GangErrorCategory::PeerDmaTimeout,
            raw_journal_summary: "DMA engine timed out".into(),
        };
        assert_eq!(triage_gang_crash(&ctx_dma).recommended_action, RemediationAction::RestartWithBackoff);

        let ctx_crash = GangCrashContext {
            gang_id: "gang-105".into(),
            lease_id: None,
            member_devices: vec!["/dev/dri/card0".into()],
            failed_device: "/dev/dri/card0".into(),
            error_category: GangErrorCategory::ProcessCrash,
            raw_journal_summary: "SIGSEGV in worker process".into(),
        };
        assert_eq!(triage_gang_crash(&ctx_crash).recommended_action, RemediationAction::Restart);
    }

    #[test]
    fn test_gang_triage_deduplicates_healthy_devices() {
        let ctx = GangCrashContext {
            gang_id: "gang-106".into(),
            lease_id: None,
            // Multiple slices on card1
            member_devices: vec!["/dev/dri/card0".into(), "/dev/dri/card1".into(), "/dev/dri/card1".into()],
            failed_device: "/dev/dri/card0".into(),
            error_category: GangErrorCategory::DriverHang,
            raw_journal_summary: "GPU hang".into(),
        };
        let verdict = triage_gang_crash(&ctx);
        assert_eq!(verdict.healthy_devices, vec!["/dev/dri/card1"]);
    }
}
