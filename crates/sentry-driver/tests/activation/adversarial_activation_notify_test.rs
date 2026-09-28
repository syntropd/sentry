//! Adversarial stress test harness for socket activation and notification subsystems.
//!
//! Evaluates behavior under corrupt environment variables, extreme descriptor counts,
//! missing descriptors, invalid socket paths, and watchdog configuration edge cases.

use sentry_driver::activation::{parse_listen_fdnames, parse_listen_fds, validate_listen_pid};
use sentry_driver::notify::{
    parse_watchdog_config, resolve_notify_address, send_notify, NotifyState,
};
use std::env;
use std::process;
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_adversarial_listen_pid_corrupt_values() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let invalid_pids = ["-1", "abc", "12.34", "99999999999999999999", " 123 "];
    for pid in invalid_pids {
        env::set_var("LISTEN_PID", pid);
        assert!(
            validate_listen_pid().is_err(),
            "PID '{pid}' should be rejected as invalid"
        );
    }

    env::remove_var("LISTEN_PID");
    assert_eq!(validate_listen_pid().unwrap(), false);

    // Matching PID
    env::set_var("LISTEN_PID", process::id().to_string());
    assert_eq!(validate_listen_pid().unwrap(), true);

    // Mismatched PID
    env::set_var("LISTEN_PID", (process::id() + 9999).to_string());
    assert_eq!(validate_listen_pid().unwrap(), false);

    env::remove_var("LISTEN_PID");
}

#[test]
fn test_adversarial_listen_fds_corrupted_inputs() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    env::set_var("LISTEN_PID", process::id().to_string());

    let invalid_counts = ["-1", "abc", "0x10", "1.0", " 5 "];
    for count in invalid_counts {
        env::set_var("LISTEN_FDS", count);
        assert!(
            parse_listen_fds(false).is_err(),
            "Count '{count}' should fail parsing"
        );
    }

    // Zero count returns empty list
    env::set_var("LISTEN_FDS", "0");
    assert_eq!(parse_listen_fds(false).unwrap().len(), 0);

    // Unset LISTEN_FDS returns empty list
    env::remove_var("LISTEN_FDS");
    assert_eq!(parse_listen_fds(false).unwrap().len(), 0);

    env::remove_var("LISTEN_PID");
}

#[test]
fn test_adversarial_listen_fds_unopened_descriptors_fail_safely() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    env::set_var("LISTEN_PID", process::id().to_string());
    // Claim 10 fds are passed (fds 3..12), but in this test process fd 10..12 are closed
    env::set_var("LISTEN_FDS", "10");
    env::set_var("LISTEN_FDNAMES", "s1:s2:s3:s4:s5:s6:s7:s8:s9:s10");

    let res = parse_listen_fds(false);
    // Since fd 3+ are probably not open sockets or descriptors, fcntl should fail safely with FcntlError
    match res {
        Ok(sockets) => {
            // In case fds happen to be open in test runner
            assert_eq!(sockets.len(), 10);
        }
        Err(e) => {
            // Must be clean error, no panic
            assert!(
                matches!(e, sentry_driver::activation::ActivationError::FcntlError(..)),
                "Unexpected error type: {e:?}"
            );
        }
    }

    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_FDNAMES");
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_adversarial_listen_fds_extreme_overflow_check() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    env::set_var("LISTEN_PID", process::id().to_string());
    env::set_var("LISTEN_FDS", usize::MAX.to_string());

    let res = parse_listen_fds(false);
    assert!(
        matches!(res, Err(sentry_driver::activation::ActivationError::InvalidFdCount(..))),
        "Expected InvalidFdCount error on extreme LISTEN_FDS, got: {res:?}"
    );

    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_adversarial_listen_fdnames_malformed_formats() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // Colons only
    env::set_var("LISTEN_FDNAMES", "::::");
    let names = parse_listen_fdnames(4);
    assert_eq!(names.len(), 4);
    for name in &names {
        assert_eq!(name, "unknown");
    }

    // Fewer names than count -> pads with unknown:N
    env::set_var("LISTEN_FDNAMES", "http");
    let names = parse_listen_fdnames(3);
    assert_eq!(names, vec!["http", "unknown:1", "unknown:2"]);

    // More names than count -> truncates
    env::set_var("LISTEN_FDNAMES", "a:b:c:d:e");
    let names = parse_listen_fdnames(2);
    assert_eq!(names, vec!["a", "b"]);

    env::remove_var("LISTEN_FDNAMES");
}

#[test]
fn test_adversarial_watchdog_config_boundary_and_corrupt() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // Invalid integers
    let invalid_usec = ["-1", "abc", "100.5", " 500 "];
    for val in invalid_usec {
        env::set_var("WATCHDOG_USEC", val);
        assert!(
            parse_watchdog_config(false).is_err(),
            "WATCHDOG_USEC='{val}' should fail parsing"
        );
    }

    // Zero usec -> disabled (Ok(None))
    env::set_var("WATCHDOG_USEC", "0");
    assert_eq!(parse_watchdog_config(false).unwrap(), None);

    // Extreme huge value u64::MAX -> half is u64::MAX / 2
    env::set_var("WATCHDOG_USEC", u64::MAX.to_string());
    let cfg = parse_watchdog_config(false).unwrap().expect("should parse u64::MAX");
    assert_eq!(cfg.raw_usec, u64::MAX);
    assert_eq!(cfg.interval.as_micros(), (u64::MAX / 2) as u128);

    // Watchdog PID mismatch
    env::set_var("WATCHDOG_USEC", "2000000");
    env::set_var("WATCHDOG_PID", (process::id() + 1000).to_string());
    assert_eq!(parse_watchdog_config(false).unwrap(), None);

    // Watchdog USEC=1 leads to interval clamped to at least 1 us
    env::set_var("WATCHDOG_USEC", "1");
    env::remove_var("WATCHDOG_PID");
    let cfg1 = parse_watchdog_config(false).unwrap().expect("should parse 1");
    assert_eq!(cfg1.interval, std::time::Duration::from_micros(1));

    env::remove_var("WATCHDOG_USEC");
}

#[tokio::test]
async fn test_adversarial_watchdog_ticker_zero_interval_panic() {
    use sentry_driver::notify::{WatchdogConfig, WatchdogTicker};
    let zero_cfg = WatchdogConfig {
        interval: std::time::Duration::ZERO,
        raw_usec: 1,
        target_pid: None,
    };

    let ticker = WatchdogTicker::spawn(zero_cfg);
    ticker.stop().await;
}

#[test]
fn test_adversarial_notify_socket_address_stress() {
    // Empty path
    assert!(resolve_notify_address("").is_err());

    // Path with null byte for filesystem path
    assert!(resolve_notify_address("/tmp/test\0bad.sock").is_err());

    // Path exceeding sockaddr_un limits (typically 108 bytes on Linux)
    let huge_path = format!("/tmp/{}", "a".repeat(200));
    assert!(
        resolve_notify_address(&huge_path).is_err(),
        "200-byte path should exceed sockaddr_un limits"
    );

    let huge_abstract = format!("@{}", "b".repeat(200));
    assert!(
        resolve_notify_address(&huge_abstract).is_err(),
        "200-byte abstract name should exceed limits"
    );
}

#[test]
fn test_adversarial_send_notify_missing_socket_safe() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    env::remove_var("NOTIFY_SOCKET");

    // When NOTIFY_SOCKET is unset, send_notify returns Ok(false) without failing
    let sent = send_notify(&[NotifyState::Ready], false).unwrap();
    assert_eq!(sent, false);
}
