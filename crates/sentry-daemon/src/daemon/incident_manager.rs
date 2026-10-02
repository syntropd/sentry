//! Processes unit failure events, collects host telemetry, runs triage, and triggers remediation.

use crate::daemon::audit_log::AuditLogger;
use crate::daemon::pending_queue::{PendingIncident, PendingQueue};
use crate::daemon::state::DaemonState;
use crate::ipc::protocol::IpcResponse;
use crate::system::RemediationExecutor;
use sentry_core::models::{DecisionTier, FaultClass, IncidentContext, RemediationAction};
use sentry_diagnostic::fallback::DeterministicFallbackEngine;
use sentry_diagnostic::system_one::SystemOneClassifier;
use sentry_driver::cgroup::collect_cgroup_telemetry_resilient;
use sentry_driver::dbus::UnitFailedEvent;
use sentry_driver::psi::collect_system_psi_resilient;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Orchestrates telemetry collection, triage analysis, circuit breaking, and remediation.
pub struct IncidentManager;

impl IncidentManager {
    /// Process a unit failure event asynchronously.
    pub async fn handle_unit_failure(
        event: UnitFailedEvent,
        state: Arc<Mutex<DaemonState>>,
        remediation_executor: &RemediationExecutor,
    ) {
        let unit_name = event.unit.clone();
        info!("Processing failure incident for unit: {}", unit_name);

        // 1. Check load shedding status
        let is_degraded = {
            let s = state.lock().await;
            s.load_shedder.is_degraded()
        };

        // 2. Gather kernel telemetry
        let cgroup_root = std::path::Path::new("/sys/fs/cgroup");
        let proc_path = std::path::Path::new("/proc/pressure");
        let cgroup = Some(collect_cgroup_telemetry_resilient(cgroup_root, &unit_name, None));
        let pressure = Some(collect_system_psi_resilient(proc_path));

        // 3. Assemble incident context
        let details = sentry_core::models::UnitFailedDetails {
            unit: event.unit.clone(),
            active_state: event.active_state.clone(),
            sub_state: event.sub_state.clone(),
            result: event.result.clone(),
            exec_status: event.exec_status,
            main_pid: None,
        };
        let failure_event = sentry_core::models::DriverEvent::UnitFailed(details.clone());
        let mut incident = IncidentContext::new(&unit_name, failure_event);
        incident.cgroup = cgroup;
        incident.telemetry = pressure;

        // Connect systemd-coredump: scan for core dumps matching crashed unit
        let coredump_dir = std::path::Path::new("/var/lib/systemd/coredump");
        if details.result.as_deref() == Some("core-dump") || details.result.as_deref() == Some("signal") {
            let comm_prefix = unit_name.strip_suffix(".service").unwrap_or(&unit_name);
            incident.coredump = sentry_driver::coredump::find_latest_coredump(coredump_dir, Some(comm_prefix));
        }

        // 4. Perform System One fast classification
        let s1_classifier = SystemOneClassifier::default();
        let s1_verdict = s1_classifier.classify_resilient(&incident).await;

        let diagnostic = if is_degraded || s1_verdict.tier == DecisionTier::High {
            if is_degraded {
                info!("Load shedding active: performing deterministic fallback triage on {}", unit_name);
            }
            DeterministicFallbackEngine::new().triage(&incident)
        } else {
            let engine = {
                let s = state.lock().await;
                s.diagnostic_engine.clone()
            };
            engine.diagnose(&incident).await
        };

        // 5. Update circuit breaker state and evaluate lockout
        let (breaker_state_label, action_allowed, gatekeeper) = {
            let mut s = state.lock().await;
            let breaker_state = s.circuit_registry.record_failure(&unit_name, std::time::Instant::now());
            let allowed = breaker_state.allows_remediation();
            (breaker_state.label(), allowed, s.policy_gatekeeper.clone())
        };

        info!(
            "Unit {} circuit state: {}, action allowed: {}, s1_tier: {}",
            unit_name, breaker_state_label, action_allowed, s1_verdict.tier.as_str()
        );

        // 6. Tri-Tier state machine gating
        match s1_verdict.tier {
            DecisionTier::High => {
                // S >= 0.90: Auto-remediate immediately + structured journal log (zero terminal noise)
                let action = match s1_verdict.fault_class {
                    FaultClass::TransientRestart => RemediationAction::Restart,
                    FaultClass::ConfigDrift => RemediationAction::Reload,
                    FaultClass::DependencyFailure => RemediationAction::RestartWithBackoff,
                    FaultClass::ManualTriageRequired => RemediationAction::NoAction,
                };
                let mut remediation = diagnostic.proposed_remediation.clone();
                remediation.action = action;

                if action_allowed && action.is_active_modification() {
                    match remediation_executor
                        .execute_with_gatekeeper(&unit_name, &remediation, &gatekeeper)
                        .await
                    {
                        Ok(msg) => {
                            AuditLogger::log(
                                &incident.incident_id.to_string(),
                                &unit_name,
                                s1_verdict.fault_class,
                                s1_verdict.tier,
                                action.as_str(),
                                s1_verdict.confidence,
                                "AUTO_REMEDIATED",
                                &format!("{}: {msg}", s1_verdict.explanation),
                            );
                        }
                        Err(err) => {
                            AuditLogger::log(
                                &incident.incident_id.to_string(),
                                &unit_name,
                                s1_verdict.fault_class,
                                s1_verdict.tier,
                                action.as_str(),
                                s1_verdict.confidence,
                                "REMEDIATION_FAILED",
                                &format!("{}: {err}", s1_verdict.explanation),
                            );
                        }
                    }
                } else if !action_allowed {
                    warn!("Remediation skipped for {}: circuit breaker locked out", unit_name);
                }
            }
            DecisionTier::Medium => {
                // 0.60 <= S < 0.90: Stage in pending queue + update pending count
                let action = match s1_verdict.fault_class {
                    FaultClass::TransientRestart => RemediationAction::Restart,
                    FaultClass::ConfigDrift => RemediationAction::Reload,
                    FaultClass::DependencyFailure => RemediationAction::RestartWithBackoff,
                    FaultClass::ManualTriageRequired => RemediationAction::EscalateToAdmin,
                };
                let pending_inc = PendingIncident {
                    incident_id: incident.incident_id.to_string(),
                    unit: unit_name.clone(),
                    timestamp: incident.timestamp.to_rfc3339(),
                    fault_class: s1_verdict.fault_class,
                    confidence: s1_verdict.confidence,
                    tier: s1_verdict.tier,
                    proposed_action: action,
                    explanation: s1_verdict.explanation.clone(),
                    journal_excerpt: incident.journal_lines.clone(),
                };
                let queue = PendingQueue::default();
                let _ = queue.stage(pending_inc).await;

                AuditLogger::log(
                    &incident.incident_id.to_string(),
                    &unit_name,
                    s1_verdict.fault_class,
                    s1_verdict.tier,
                    action.as_str(),
                    s1_verdict.confidence,
                    "STAGED_PENDING",
                    &s1_verdict.explanation,
                );
            }
            DecisionTier::Low => {
                // S < 0.60: Lock unit against restart loops; record forensic log
                {
                    let mut s = state.lock().await;
                    s.circuit_registry.lock_unit(&unit_name, std::time::Instant::now());
                }
                AuditLogger::log(
                    &incident.incident_id.to_string(),
                    &unit_name,
                    s1_verdict.fault_class,
                    s1_verdict.tier,
                    "Lockout",
                    s1_verdict.confidence,
                    "LOCKED_OUT",
                    &format!(
                        "Low confidence ({:.3}) - unit locked against restart loops: {}",
                        s1_verdict.confidence, s1_verdict.explanation
                    ),
                );
            }
        }

        // 7. Store incident in MCP state and broadcast
        let event_payload = serde_json::json!({
            "incident_id": incident.incident_id,
            "unit": unit_name,
            "severity": diagnostic.severity,
            "root_cause": diagnostic.root_cause.summary,
            "action": diagnostic.proposed_remediation.action,
            "circuit_state": breaker_state_label,
            "system_one_tier": s1_verdict.tier.as_str(),
        });

        let s = state.lock().await;
        s.mcp_state.record_incident(diagnostic);
        let _ = s.event_broadcaster.send(IpcResponse::Event { data: event_payload });
    }
}
