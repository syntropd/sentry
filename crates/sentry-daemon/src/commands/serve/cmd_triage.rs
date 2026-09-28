//! Command: On-demand root-cause triage of a systemd unit.

use crate::cli::exit_codes::EX_OK;
use crate::config::daemon_config::DaemonConfig;
use sentry_core::models::{IncidentContext, UnitFailedDetails};
use sentry_diagnostic::DiagnosticEngine;
use sentry_driver::cgroup::collect_cgroup_telemetry_resilient;
use sentry_driver::coredump::find_latest_coredump;
use sentry_driver::dbus::{decode_unit_properties, get_service_properties, get_unit_properties};
use sentry_driver::psi::collect_system_psi_resilient;

/// Execute the `triage` subcommand.
pub async fn execute_triage(unit: &str, json: bool, config: &DaemonConfig) -> i32 {
    let mut active_state = "failed".to_string();
    let mut sub_state = "failed".to_string();
    let mut result = Some("exit-code".to_string());
    let mut exec_status = Some(1);
    let mut main_pid = None;

    if let Ok(conn) = zbus::Connection::system().await {
        if let Ok(unit_props) = get_unit_properties(&conn, unit).await {
            if let Some(update) = decode_unit_properties(unit.to_string(), "org.freedesktop.systemd1.Unit", &unit_props) {
                if let Some(s) = update.active_state {
                    active_state = s;
                }
                if let Some(s) = update.sub_state {
                    sub_state = s;
                }
            }
        }
        if let Ok(svc_props) = get_service_properties(&conn, unit).await {
            if let Some(update) = decode_unit_properties(unit.to_string(), "org.freedesktop.systemd1.Service", &svc_props) {
                if let Some(r) = update.result {
                    result = Some(r);
                }
                if let Some(s) = update.exec_main_status {
                    exec_status = Some(s);
                }
                main_pid = update.main_pid;
            }
        }
    }

    let cgroup_root = std::path::Path::new("/sys/fs/cgroup");
    let proc_path = std::path::Path::new("/proc/pressure");
    let cgroup = Some(collect_cgroup_telemetry_resilient(cgroup_root, unit, main_pid));
    let pressure = Some(collect_system_psi_resilient(proc_path));

    let coredump_dir = std::path::Path::new("/var/lib/systemd/coredump");
    let comm_prefix = unit.strip_suffix(".service").unwrap_or(unit);
    let coredump = find_latest_coredump(coredump_dir, Some(comm_prefix));

    if let Some(ref cd) = coredump {
        if cd.signal > 0 {
            exec_status = Some(cd.signal);
            result = Some("core-dump".to_string());
        }
    }

    let failure_event = sentry_core::models::DriverEvent::UnitFailed(UnitFailedDetails {
        unit: unit.to_string(),
        active_state,
        sub_state,
        result,
        exec_status,
        main_pid,
    });

    let mut incident = IncidentContext::new(unit, failure_event);
    incident.cgroup = cgroup;
    incident.telemetry = pressure;
    incident.coredump = coredump;

    let provider: std::sync::Arc<dyn sentry_diagnostic::LlmProvider> = std::sync::Arc::from(
        sentry_diagnostic::provider::create_provider(&config.provider),
    );
    let engine = DiagnosticEngine::with_config(Some(provider), config.provider.adaptive.clone());
    let diagnostic = engine.diagnose(&incident).await;

    if json {
        println!("{}", serde_json::to_string_pretty(&diagnostic).unwrap_or_default());
    } else {
        println!("============================================================");
        println!(" DIAGNOSTIC TRIAGE REPORT: {}", unit);
        println!("============================================================");
        println!("Severity:     {:?}", diagnostic.severity);
        println!("Root Cause:   {}", diagnostic.root_cause.summary);
        println!("Details:      {}", diagnostic.root_cause.detail);
        println!("\nProposed Remediation:");
        println!("  Action:     {:?}", diagnostic.proposed_remediation.action);
        println!("  Confidence: {:.2}", diagnostic.proposed_remediation.confidence);
        println!("  Rationale:  {}", diagnostic.proposed_remediation.rationale);
        println!("============================================================");
    }

    EX_OK
}
