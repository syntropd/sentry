//! Adversarial stress tests for pure Rust `sd_notify` socket sender.

use sentry_driver::notify::{
    encode_notify_payload, notify_ready, notify_reloading, notify_status, notify_stopping,
    notify_watchdog, resolve_notify_address, send_notify, NotifyError, NotifyState,
};
use std::env;
use std::io;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::sync::Mutex;
use tempfile::tempdir;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_notify_socket_unset_and_empty_graceful_noop() {
    let _lock = TEST_MUTEX.lock().unwrap();

    // 1. Completely unset NOTIFY_SOCKET
    env::remove_var("NOTIFY_SOCKET");
    assert!(!send_notify(&[NotifyState::Ready], false).unwrap());
    assert!(!send_notify(&[NotifyState::Ready], true).unwrap());
    assert!(!notify_ready().unwrap());
    assert!(!notify_status("Testing unset").unwrap());
    assert!(!notify_watchdog().unwrap());
    assert!(!notify_stopping().unwrap());
    assert!(!notify_reloading().unwrap());

    // 2. Empty string NOTIFY_SOCKET
    env::set_var("NOTIFY_SOCKET", "");
    assert!(!send_notify(&[NotifyState::Ready], false).unwrap());
    assert!(!notify_ready().unwrap());
    assert!(!notify_status("Testing empty").unwrap());
    env::remove_var("NOTIFY_SOCKET");
}

#[test]
fn test_notify_nonexistent_filesystem_path_returns_error() {
    let _lock = TEST_MUTEX.lock().unwrap();

    let bogus_path = "/nonexistent/directory/sentry_never_exists_987654.sock";
    env::set_var("NOTIFY_SOCKET", bogus_path);

    let err = send_notify(&[NotifyState::Ready], false).unwrap_err();
    match err {
        NotifyError::Io(io_err) => {
            assert_eq!(io_err.kind(), io::ErrorKind::NotFound);
        }
        other => panic!("Expected NotifyError::Io(NotFound), got: {other:?}"),
    }

    let ready_err = notify_ready().unwrap_err();
    assert!(matches!(ready_err, NotifyError::Io(_)));

    env::remove_var("NOTIFY_SOCKET");
}

#[test]
fn test_notify_non_socket_filesystem_target_fails() {
    let _lock = TEST_MUTEX.lock().unwrap();

    // Target a regular file / char device which is not a socket
    env::set_var("NOTIFY_SOCKET", "/dev/null");
    let err = send_notify(&[NotifyState::Ready], false).unwrap_err();
    assert!(matches!(err, NotifyError::Io(_)));

    env::remove_var("NOTIFY_SOCKET");
}

#[test]
fn test_notify_abstract_socket_sender_unbound_fails() {
    let _lock = TEST_MUTEX.lock().unwrap();

    // Linux abstract socket without listener bound returns ECONNREFUSED
    let abstract_name = format!("@sentry_challenger_unbound_{}", std::process::id());
    env::set_var("NOTIFY_SOCKET", &abstract_name);

    let err = notify_ready().unwrap_err();
    match err {
        NotifyError::Io(io_err) => {
            assert_eq!(io_err.kind(), io::ErrorKind::ConnectionRefused);
        }
        other => panic!("Expected ConnectionRefused io error, got: {other:?}"),
    }

    env::remove_var("NOTIFY_SOCKET");
}

#[test]
fn test_notify_abstract_socket_sender_bound_delivers_payload() {
    let _lock = TEST_MUTEX.lock().unwrap();

    let abstract_name = format!("sentry_challenger_bound_{}", std::process::id());
    let addr = SocketAddr::from_abstract_name(abstract_name.as_bytes()).unwrap();
    let listener = UnixDatagram::bind_addr(&addr).unwrap();

    env::set_var("NOTIFY_SOCKET", format!("@{abstract_name}"));

    // 1. Verify READY pulse delivery
    let sent = notify_ready().unwrap();
    assert!(sent);

    let mut buf = [0u8; 512];
    let (n, _) = listener.recv_from(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"READY=1\n");

    // 2. Verify WATCHDOG pulse delivery
    let sent = notify_watchdog().unwrap();
    assert!(sent);
    let (n, _) = listener.recv_from(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"WATCHDOG=1\n");

    // 3. Verify unset_env = true removes variable after sending
    let sent = send_notify(&[NotifyState::Stopping], true).unwrap();
    assert!(sent);
    let (n, _) = listener.recv_from(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"STOPPING=1\n");
    assert!(env::var("NOTIFY_SOCKET").is_err());
}

#[test]
fn test_notify_abstract_socket_name_too_long() {
    // Linux abstract socket names cannot exceed sockaddr_un.sun_path (107 bytes)
    let overly_long_abstract = format!("@{}", "a".repeat(200));
    let err = resolve_notify_address(&overly_long_abstract).unwrap_err();
    assert!(matches!(err, NotifyError::InvalidSocketAddress(_, _)));
}

#[test]
fn test_notify_status_boundaries_and_rejections() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("boundary_notify.sock");
    let receiver = UnixDatagram::bind(&sock_path).unwrap();
    env::set_var("NOTIFY_SOCKET", sock_path.to_str().unwrap());

    // 1. Boundary: Exactly 1024 bytes must succeed
    let status_1024 = "x".repeat(1024);
    let res = notify_status(&status_1024);
    assert!(res.is_ok());
    assert!(res.unwrap());

    let mut buf = [0u8; 2048];
    let (n, _) = receiver.recv_from(&mut buf).unwrap();
    let expected = format!("STATUS={status_1024}\n");
    assert_eq!(&buf[..n], expected.as_bytes());

    // 2. Violation: Exactly 1025 bytes must be rejected
    let status_1025 = "x".repeat(1025);
    let err = notify_status(&status_1025).unwrap_err();
    match err {
        NotifyError::StatusTooLong(len) => assert_eq!(len, 1025),
        other => panic!("Expected StatusTooLong(1025), got: {other:?}"),
    }

    // 3. Violation: 65,536 bytes must be rejected
    let status_huge = "y".repeat(65536);
    let err_huge = notify_status(&status_huge).unwrap_err();
    match err_huge {
        NotifyError::StatusTooLong(len) => assert_eq!(len, 65536),
        other => panic!("Expected StatusTooLong(65536), got: {other:?}"),
    }

    // 4. Multiline newline sanitization
    let multiline = "Line1\nLine2\nLine3";
    let encoded = encode_notify_payload(&[NotifyState::Status(multiline.into())]);
    assert_eq!(encoded, "STATUS=Line1 Line2 Line3\n");

    env::remove_var("NOTIFY_SOCKET");
}
