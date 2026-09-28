//! 1:1 Unit QA tests for notify socket transmission.

use super::test_watchdog_config::NOTIFY_ENV_LOCK;
use sentry_driver::notify::{
    notify_ready, notify_reloading, notify_status, notify_stopping, notify_watchdog, send_notify,
    NotifyState,
};
use std::env;
use std::os::unix::net::UnixDatagram;
use tempfile::tempdir;

#[test]
fn test_send_notify_when_unset_returns_false() {
    let _guard = NOTIFY_ENV_LOCK.lock().unwrap();
    env::remove_var("NOTIFY_SOCKET");
    let result = send_notify(&[NotifyState::Ready], false).unwrap();
    assert!(!result);

    assert!(!notify_ready().unwrap());
    assert!(!notify_watchdog().unwrap());
    assert!(!notify_stopping().unwrap());
    assert!(!notify_reloading().unwrap());
    assert!(!notify_status("test").unwrap());
}

#[test]
fn test_send_notify_to_live_socket() {
    let _guard = NOTIFY_ENV_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("test_notify.sock");
    let receiver = UnixDatagram::bind(&sock_path).unwrap();

    env::set_var("NOTIFY_SOCKET", sock_path.to_str().unwrap());

    let sent = notify_ready().unwrap();
    assert!(sent);

    let mut buf = [0u8; 128];
    let (n, _) = receiver.recv_from(&mut buf).unwrap();
    let received = std::str::from_utf8(&buf[..n]).unwrap();
    assert_eq!(received, "READY=1\n");

    let sent_status = notify_status("All systems nominal").unwrap();
    assert!(sent_status);
    let (n, _) = receiver.recv_from(&mut buf).unwrap();
    let received = std::str::from_utf8(&buf[..n]).unwrap();
    assert_eq!(received, "STATUS=All systems nominal\n");

    env::remove_var("NOTIFY_SOCKET");
}

#[test]
fn test_notify_status_too_long_rejected() {
    let _guard = NOTIFY_ENV_LOCK.lock().unwrap();
    let overly_long = "a".repeat(1025);
    let err = notify_status(overly_long).unwrap_err();
    assert!(err.to_string().contains("exceeds maximum permitted length"));
}
