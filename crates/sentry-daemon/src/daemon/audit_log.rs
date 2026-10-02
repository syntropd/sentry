//! Structured systemd-journald audit logger for System One actions.

use chrono::Utc;
use sentry_core::models::{DecisionTier, FaultClass};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use tracing::info;

const JOURNALD_SOCKET: &str = "/run/systemd/journal/socket";

/// Structured audit record describing a triage decision or operator action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Incident tracking ID.
    pub incident_id: String,
    /// UTC timestamp of the audit entry.
    pub timestamp: String,
    /// Systemd service unit name.
    pub unit: String,
    /// Classified fault category.
    pub fault_class: FaultClass,
    /// Tri-tier gating category.
    pub tier: DecisionTier,
    /// Remediation action taken or proposed.
    pub action: String,
    /// Calibrated confidence score.
    pub confidence: f32,
    /// Outcome status (e.g. AUTO_REMEDIATED, STAGED_PENDING, LOCKED_OUT).
    pub status: String,
    /// Rationale for the decision.
    pub explanation: String,
}

/// Audit logger sending structured entries to journald and audit.log.
pub struct AuditLogger;

impl AuditLogger {
    /// Emits a structured audit entry.
    #[allow(clippy::too_many_arguments)]
    pub fn log(
        incident_id: &str,
        unit: &str,
        fault_class: FaultClass,
        tier: DecisionTier,
        action: &str,
        confidence: f32,
        status: &str,
        explanation: &str,
    ) {
        let record = AuditRecord {
            incident_id: incident_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            unit: unit.to_string(),
            fault_class,
            tier,
            action: action.to_string(),
            confidence,
            status: status.to_string(),
            explanation: explanation.to_string(),
        };

        Self::send_to_journald(&record);
        Self::append_audit_file(&record);

        info!(
            incident_id = %record.incident_id,
            unit = %record.unit,
            tier = %record.tier.as_str(),
            fault = %record.fault_class.as_str(),
            action = %record.action,
            status = %record.status,
            confidence = %record.confidence,
            "Audit event: {}",
            record.explanation
        );
    }

    fn send_to_journald(record: &AuditRecord) {
        let payload = format!(
            "MESSAGE=System One decision for {}: {} ({})\n\
             PRIORITY=6\n\
             SYSLOG_IDENTIFIER=systemd-sentry\n\
             SYNTROP_INCIDENT_ID={}\n\
             SYNTROP_UNIT={}\n\
             SYNTROP_FAULT_CLASS={}\n\
             SYNTROP_TIER={}\n\
             SYNTROP_ACTION={}\n\
             SYNTROP_STATUS={}\n\
             SYNTROP_CONFIDENCE={:.4}\n",
            record.unit,
            record.action,
            record.status,
            record.incident_id,
            record.unit,
            record.fault_class.as_str(),
            record.tier.as_str(),
            record.action,
            record.status,
            record.confidence
        );

        if Path::new(JOURNALD_SOCKET).exists() {
            if let Ok(sock) = UnixDatagram::unbound() {
                let _ = sock.send_to(payload.as_bytes(), JOURNALD_SOCKET);
            }
        }
    }

    fn append_audit_file(record: &AuditRecord) {
        let base = std::env::var("RUNTIME_DIRECTORY")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/run/syntrop"));
        let audit_path = base.join("audit.log");

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(audit_path)
        {
            if let Ok(line) = serde_json::to_string(record) {
                let _ = writeln!(file, "{line}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_record_serialization() {
        let rec = AuditRecord {
            incident_id: "test-inc".into(),
            timestamp: Utc::now().to_rfc3339(),
            unit: "app.service".into(),
            fault_class: FaultClass::TransientRestart,
            tier: DecisionTier::High,
            action: "RestartUnit".into(),
            confidence: 0.95,
            status: "AUTO_REMEDIATED".into(),
            explanation: "High confidence auto restart".into(),
        };

        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("app.service"));
        assert!(json.contains("AUTO_REMEDIATED"));
    }
}
