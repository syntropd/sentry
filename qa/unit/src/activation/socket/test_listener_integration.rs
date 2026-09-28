//! Integration unit tests for bind_or_activate_socket and fallback behavior.

use super::ACTIVATION_ENV_LOCK;
use sentry_daemon::ipc::bind_or_activate_socket;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[tokio::test]
async fn test_bind_or_activate_socket_manual_fallback() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::remove_var("LISTEN_PID");
    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_FDNAMES");

    let dir = tempdir().unwrap();
    let socket_path = dir.path().join("sub").join("sentry.sock");
    let socket_str = socket_path.to_str().unwrap();

    let listener = bind_or_activate_socket(socket_str)
        .expect("Manual socket binding fallback must succeed");

    assert!(socket_path.exists());
    let perms = fs::metadata(&socket_path).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, 0o660);

    drop(listener);
}

#[tokio::test]
async fn test_bind_or_activate_socket_stale_socket_cleanup() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::remove_var("LISTEN_PID");
    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_FDNAMES");

    let dir = tempdir().unwrap();
    let socket_path = dir.path().join("stale.sock");
    let socket_str = socket_path.to_str().unwrap();

    // Create a dead/stale socket file
    let _ = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
    // Drop listener so it's not actively listening (connect will fail -> ECONNREFUSED)

    let listener = bind_or_activate_socket(socket_str)
        .expect("Must recover from stale unlistening socket");

    assert!(socket_path.exists());
    drop(listener);
}
