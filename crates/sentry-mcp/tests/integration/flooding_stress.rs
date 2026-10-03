//! Stress test harness for McpState incident queue flooding and RSS memory bounds.

use sentry_core::models::{
    DiagnosticPayload, ProposedRemediation, RemediationAction, RiskLevel, RootCause, Severity,
};
use sentry_mcp::storage::mcp_state::MAX_STORED_INCIDENTS;
use sentry_mcp::storage::McpState;
use std::sync::Arc;
use uuid::Uuid;

fn make_incident(id: Uuid, seq: usize) -> DiagnosticPayload {
    DiagnosticPayload {
        incident_id: id,
        timestamp: chrono::Utc::now(),
        unit_name: format!("service-{seq}.service"),
        root_cause: RootCause {
            summary: format!("Crash #{seq}"),
            detail: "Kernel SIGSEGV fault simulated in stress harness".to_string(),
        },
        evidence: Default::default(),
        severity: Severity::Critical,
        proposed_remediation: ProposedRemediation {
            action: RemediationAction::RestartWithBackoff,
            rationale: "Automated backoff restart".to_string(),
            risk_level: RiskLevel::Medium,
            confidence: 0.95,
        },
    }
}

fn read_proc_vmrss_bytes() -> u64 {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return kb * 1024;
                    }
                }
            }
        }
    }
    0
}

#[tokio::test]
async fn test_sequential_flooding_1000_incidents_bounds_and_fifo() {
    let state = McpState::new();
    let mut incident_ids = Vec::with_capacity(1000);

    for seq in 0..1000 {
        let id = Uuid::new_v4();
        incident_ids.push(id);
        state.record_incident(make_incident(id, seq));

        // Invariant check: at every step, store size must never exceed MAX_STORED_INCIDENTS
        let current_count = state.list_incidents(2000, None, None).len();
        assert!(
            current_count <= MAX_STORED_INCIDENTS,
            "State size {current_count} exceeded limit {MAX_STORED_INCIDENTS} at step {seq}"
        );
    }

    let final_incidents = state.list_incidents(2000, None, None);
    assert_eq!(
        final_incidents.len(),
        MAX_STORED_INCIDENTS,
        "Final incident count must equal MAX_STORED_INCIDENTS (100)"
    );

    // Verify FIFO eviction: First 900 incidents must be evicted
    for id in &incident_ids[0..900] {
        assert!(
            state.get_incident(&id.to_string()).is_none(),
            "Incident {id} should have been evicted by FIFO policy"
        );
    }

    // Verify the latest 100 incidents (900..1000) are retained
    for id in &incident_ids[900..1000] {
        assert!(
            state.get_incident(&id.to_string()).is_some(),
            "Incident {id} must still be stored in bounded queue"
        );
    }
}

#[tokio::test]
async fn test_concurrent_flooding_multi_tasks() {
    let state = Arc::new(McpState::new());
    let mut handles = Vec::new();

    // Spawn 10 concurrent tasks each recording 100 incidents (1,000 total)
    for task_idx in 0..10 {
        let state_clone = Arc::clone(&state);
        handles.push(tokio::spawn(async move {
            for item_idx in 0..100 {
                let id = Uuid::new_v4();
                let seq = task_idx * 100 + item_idx;
                state_clone.record_incident(make_incident(id, seq));
            }
        }));
    }

    for h in handles {
        h.await.expect("Task panicked during concurrent flood");
    }

    let count = state.list_incidents(2000, None, None).len();
    assert_eq!(
        count, MAX_STORED_INCIDENTS,
        "Concurrent flood must not exceed MAX_STORED_INCIDENTS"
    );
}

#[tokio::test]
async fn test_memory_rss_budget_under_flooding() {
    let state = McpState::new();
    let initial_rss = read_proc_vmrss_bytes();

    // Flood 5,000 rapid incidents to test for memory leaks in ring buffer
    for seq in 0..5000 {
        let id = Uuid::new_v4();
        state.record_incident(make_incident(id, seq));
    }

    let final_rss = read_proc_vmrss_bytes();
    let max_budget_bytes = 15 * 1024 * 1024; // 15MB budget from audit requirements

    // If /proc/self/status is available on this environment, verify RSS
    if final_rss > 0 {
        assert!(
            final_rss < max_budget_bytes,
            "Final RSS {} bytes exceeds 15MB budget ({} bytes)",
            final_rss,
            max_budget_bytes
        );
    }

    // Verify incidents count is strictly bounded
    assert_eq!(state.list_incidents(10000, None, None).len(), 100);
    let _ = initial_rss;
}
