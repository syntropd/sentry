//! Adversarial stress tests for systemd socket activation parser.

use sentry_driver::activation::{
    parse_listen_fdnames, parse_listen_fds, validate_listen_pid, ActivationError,
};
use std::env;
use std::process;
use std::sync::Mutex;

static ACTIVATION_TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_activation_missing_pid_is_graceful_noop() {
    let _lock = ACTIVATION_TEST_MUTEX.lock().unwrap();

    // 1. LISTEN_FDS set to 1, but LISTEN_PID unset -> must return Ok(false) / Ok(vec![])
    env::remove_var("LISTEN_PID");
    env::set_var("LISTEN_FDS", "1");
    assert!(!validate_listen_pid().unwrap());
    let sockets = parse_listen_fds(false).unwrap();
    assert!(sockets.is_empty(), "Must not activate if LISTEN_PID is unset");

    // 2. LISTEN_PID set to empty string -> graceful no-op
    env::set_var("LISTEN_PID", "");
    assert!(!validate_listen_pid().unwrap());
    let sockets_empty = parse_listen_fds(false).unwrap();
    assert!(sockets_empty.is_empty(), "Must not activate if LISTEN_PID is empty");

    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_activation_wrong_pid_is_graceful_noop() {
    let _lock = ACTIVATION_TEST_MUTEX.lock().unwrap();

    let my_pid = process::id();
    let foreign_pid = my_pid + 98765;

    env::set_var("LISTEN_PID", foreign_pid.to_string());
    env::set_var("LISTEN_FDS", "2");

    assert!(!validate_listen_pid().unwrap());
    let sockets = parse_listen_fds(false).unwrap();
    assert!(sockets.is_empty(), "Must not activate for different PID");

    // Init PID (pid 1) mismatch
    if my_pid != 1 {
        env::set_var("LISTEN_PID", "1");
        assert!(!validate_listen_pid().unwrap());
        let sockets_init = parse_listen_fds(false).unwrap();
        assert!(sockets_init.is_empty());
    }

    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_activation_malformed_pid_returns_error() {
    let _lock = ACTIVATION_TEST_MUTEX.lock().unwrap();

    let malformed_pids = [
        "not_a_number",
        "-42",
        "12.34",
        "4294967296", // u32::MAX + 1
        "99999999999999999999999999999999",
        "0x2A",
    ];

    for bad_pid in malformed_pids {
        env::set_var("LISTEN_PID", bad_pid);
        env::set_var("LISTEN_FDS", "1");

        let val_err = validate_listen_pid().unwrap_err();
        assert!(
            matches!(val_err, ActivationError::InvalidPid(_, _)),
            "Expected InvalidPid for '{bad_pid}', got: {val_err:?}"
        );

        let parse_err = parse_listen_fds(false).unwrap_err();
        assert!(
            matches!(parse_err, ActivationError::InvalidPid(_, _)),
            "Expected InvalidPid for '{bad_pid}', got: {parse_err:?}"
        );
    }

    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_activation_negative_fd_count_rejected() {
    let _lock = ACTIVATION_TEST_MUTEX.lock().unwrap();

    env::set_var("LISTEN_PID", process::id().to_string());

    let negative_counts = ["-1", "-5", "-1000", "-999999999"];

    for bad_count in negative_counts {
        env::set_var("LISTEN_FDS", bad_count);
        let err = parse_listen_fds(false).unwrap_err();
        match err {
            ActivationError::InvalidFdCount(val, _) => {
                assert_eq!(val, bad_count);
            }
            other => panic!("Expected InvalidFdCount for '{bad_count}', got: {other:?}"),
        }
    }

    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_activation_overflow_and_invalid_fd_counts_rejected() {
    let _lock = ACTIVATION_TEST_MUTEX.lock().unwrap();

    env::set_var("LISTEN_PID", process::id().to_string());

    let overflow_counts = [
        "18446744073709551616", // u64::MAX + 1
        "999999999999999999999999999999999999",
        "one_hundred",
        "3.14",
        "1e6",
        "0x10",
        "",
    ];

    for bad_count in overflow_counts {
        env::set_var("LISTEN_FDS", bad_count);
        if bad_count.is_empty() {
            // Empty string counts as unset -> graceful empty vec
            let sockets = parse_listen_fds(false).unwrap();
            assert!(sockets.is_empty());
        } else {
            let err = parse_listen_fds(false).unwrap_err();
            match err {
                ActivationError::InvalidFdCount(val, _) => {
                    assert_eq!(val, bad_count);
                }
                other => panic!("Expected InvalidFdCount for '{bad_count}', got: {other:?}"),
            }
        }
    }

    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_activation_zero_count_and_unopened_fd_error() {
    let _lock = ACTIVATION_TEST_MUTEX.lock().unwrap();

    env::set_var("LISTEN_PID", process::id().to_string());

    // 1. Zero count is valid and returns empty vector
    env::set_var("LISTEN_FDS", "0");
    let sockets = parse_listen_fds(false).unwrap();
    assert!(sockets.is_empty());

    // 2. Count = 1 without file descriptor 3 being opened returns FcntlError (EBADF)
    env::set_var("LISTEN_FDS", "1");
    // Ensure fd 3 is not open or will fail fcntl
    let err = parse_listen_fds(false).unwrap_err();
    match err {
        ActivationError::FcntlError(fd, io_err) => {
            assert_eq!(fd, 3);
            // EBADF on Linux is raw OS error 9
            assert_eq!(io_err.raw_os_error(), Some(9));
        }
        other => panic!("Expected FcntlError(3, EBADF), got: {other:?}"),
    }

    env::remove_var("LISTEN_FDS");
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_activation_fdnames_parser_edge_cases() {
    let _lock = ACTIVATION_TEST_MUTEX.lock().unwrap();

    // 1. Names count matches requested count
    env::set_var("LISTEN_FDNAMES", "sentry-web:sentry-api:sentry-metrics");
    let names = parse_listen_fdnames(3);
    assert_eq!(names, vec!["sentry-web", "sentry-api", "sentry-metrics"]);

    // 2. Names count smaller than requested descriptors -> padded with unknown:N
    let padded = parse_listen_fdnames(5);
    assert_eq!(
        padded,
        vec![
            "sentry-web",
            "sentry-api",
            "sentry-metrics",
            "unknown:3",
            "unknown:4"
        ]
    );

    // 3. Names count larger than requested descriptors -> truncated
    let truncated = parse_listen_fdnames(2);
    assert_eq!(truncated, vec!["sentry-web", "sentry-api"]);

    // 4. Empty components in LISTEN_FDNAMES -> replaced with "unknown"
    env::set_var("LISTEN_FDNAMES", "first::third");
    let with_empty = parse_listen_fdnames(3);
    assert_eq!(with_empty, vec!["first", "unknown", "third"]);

    env::remove_var("LISTEN_FDNAMES");
}
