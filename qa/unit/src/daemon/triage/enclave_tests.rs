//! 1:1 QA tests for the pre-diagnosis triage enclave ping.
//!
//! A fake enclave scripts accepted/declined replies so protection,
//! fail-open absence, and refusal are covered with no daemon running.
//! One sequential test: the ping resolves its socket from the process
//! environment, which parallel tests must not race on.

use sentry_daemon::commands::serve::ping_enclave::{
    ping_triage_enclave, EnclaveStatus, TRIAGE_SOCKET_ENV,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

/// Scripted fake enclave: replies `reply` to every ping, records requests.
struct FakeEnclave {
    _dir: TempDir,
    seen: Arc<Mutex<Vec<Value>>>,
}

impl FakeEnclave {
    fn start(reply: Value) -> (Self, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sentry.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let listener = UnixListener::from_std(listener).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_clone = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut conn, _)) = listener.accept().await else { break };
                let mut buf = vec![0u8; 65536];
                let Ok(n) = conn.read(&mut buf).await else { continue };
                if n == 0 {
                    continue;
                }
                let req: Value = serde_json::from_slice(&buf[..n]).unwrap();
                seen_clone.lock().unwrap().push(req);
                let bytes = serde_json::to_vec(&reply).unwrap();
                if conn.write_all(&bytes).await.is_err() {
                    break;
                }
            }
        });
        (Self { _dir: dir, seen }, path)
    }
}

#[tokio::test]
async fn test_triage_enclave_ping_outcomes() {
    // Accepted: protection granted on the assigned plane.
    let (_fake, path) = FakeEnclave::start(json!({
        "status": "accepted",
        "incident_id": "inc-1",
        "allocated_plane": "npu-sentry-0",
        "plane_assigned": "npu-sentry-0",
        "queue_latency_us": 0,
        "preemption_triggered": true,
        "analysis": "ok",
    }));
    std::env::set_var(TRIAGE_SOCKET_ENV, &path);
    match ping_triage_enclave("inc-1", "victim.service").await {
        EnclaveStatus::Protected { plane } => assert_eq!(plane, "npu-sentry-0"),
        EnclaveStatus::Unprotected { reason } => panic!("expected protection: {reason}"),
    }
    assert_eq!(_fake.seen.lock().unwrap()[0]["incident_id"], "inc-1");
    assert_eq!(_fake.seen.lock().unwrap()[0]["unit_name"], "victim.service");

    // Declined: unprotected with the enclave's reason.
    let (_fake2, path2) = FakeEnclave::start(json!({"status": "refused"}));
    std::env::set_var(TRIAGE_SOCKET_ENV, &path2);
    match ping_triage_enclave("inc-2", "victim.service").await {
        EnclaveStatus::Protected { plane } => panic!("expected refusal, got {plane}"),
        EnclaveStatus::Unprotected { reason } => assert!(reason.contains("declined"), "{reason}"),
    }

    // Absent: unprotected, diagnosis proceeds standalone.
    std::env::set_var(TRIAGE_SOCKET_ENV, "/tmp/qa-nope-enclave/never.sock");
    match ping_triage_enclave("inc-3", "victim.service").await {
        EnclaveStatus::Protected { plane } => panic!("expected absence, got {plane}"),
        EnclaveStatus::Unprotected { reason } => assert!(reason.contains("absent"), "{reason}"),
    }
    std::env::remove_var(TRIAGE_SOCKET_ENV);
}
