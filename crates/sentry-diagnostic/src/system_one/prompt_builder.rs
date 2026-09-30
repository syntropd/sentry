//! Formats incident telemetry into single-pass classification prompts.

use sentry_core::models::DriverEvent;
use sentry_core::models::IncidentContext;

/// Builds a structured prompt presenting incident context and candidate choices.
pub fn build_decision_prompt(incident: &IncidentContext) -> String {
    let mut prompt = String::with_capacity(1024);
    prompt.push_str("Systemd service failure analysis.\n\n");
    prompt.push_str(&format!("Unit: {}\n", incident.unit));

    if let DriverEvent::UnitFailed(ref details) = incident.failure_event {
        prompt.push_str(&format!(
            "ActiveState: {}, SubState: {}, Result: {}, ExitCode: {}\n",
            details.active_state,
            details.sub_state,
            details.result.as_deref().unwrap_or("unknown"),
            details
                .exec_status
                .map(|s| s.to_string())
                .unwrap_or_else(|| "none".into()),
        ));
    }

    if let Some(ref cgroup) = incident.cgroup {
        let current = cgroup.memory_current_bytes.unwrap_or(0);
        let max = cgroup
            .memory_max_bytes
            .map(|m| m.to_string())
            .unwrap_or_else(|| "unlimited".into());
        prompt.push_str(&format!(
            "Memory: current={}B, max={}, oom_kills={}\n",
            current, max, cgroup.memory_events.oom_kill
        ));
    }

    if let Some(ref dump) = incident.coredump {
        prompt.push_str(&format!(
            "Crash: signal={} ({}), executable={}\n",
            dump.signal,
            dump.signal_name,
            dump.executable.as_deref().unwrap_or("unknown")
        ));
    }

    if !incident.journal_lines.is_empty() {
        prompt.push_str("\nRecent journal logs:\n");
        let start = incident.journal_lines.len().saturating_sub(6);
        for line in &incident.journal_lines[start..] {
            prompt.push_str(&format!("- {line}\n"));
        }
    }

    prompt.push_str(
        "\nSelect the most accurate fault classification for this unit failure:\n\
A: TransientRestart (Transient glitch, segfault, or temporary error safely resolved by restart)\n\
B: ConfigDrift (Syntax error, permission denial, or drift in service configuration)\n\
C: DependencyFailure (Upstream socket, mount, or dependent service unavailable)\n\
D: ManualTriageRequired (Complex data corruption, unknown failure, or dangerous condition)\n\
Answer:",
    );

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry_core::models::UnitFailedDetails;

    #[test]
    fn test_prompt_builder_includes_unit_and_options() {
        let details = UnitFailedDetails {
            unit: "web.service".into(),
            active_state: "failed".into(),
            sub_state: "failed".into(),
            result: Some("exit-code".into()),
            exec_status: Some(1),
            main_pid: Some(1234),
        };
        let incident =
            IncidentContext::new("web.service", DriverEvent::UnitFailed(details))
                .with_journal_lines(vec!["Failed to bind port: Address already in use".into()]);

        let prompt = build_decision_prompt(&incident);
        assert!(prompt.contains("Unit: web.service"));
        assert!(prompt.contains("Address already in use"));
        assert!(prompt.contains("A: TransientRestart"));
        assert!(prompt.contains("Answer:"));
    }
}
