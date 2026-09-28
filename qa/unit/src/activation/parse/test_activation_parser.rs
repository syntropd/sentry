//! 1:1 Unit QA tests for parse_listen_fds.

use super::ACTIVATION_ENV_LOCK;
use sentry_driver::activation::{parse_listen_fds, SD_LISTEN_FDS_START};
use std::env;
use std::process;

#[test]
fn test_parse_listen_fds_when_unset_returns_empty() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::remove_var("LISTEN_PID");
    env::remove_var("LISTEN_FDS");
    let result = parse_listen_fds(false).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_parse_listen_fds_pid_mismatch_returns_empty() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    let wrong_pid = process::id() + 77777;
    env::set_var("LISTEN_PID", wrong_pid.to_string());
    env::set_var("LISTEN_FDS", "2");

    let result = parse_listen_fds(false).unwrap();
    assert!(result.is_empty());

    env::remove_var("LISTEN_PID");
    env::remove_var("LISTEN_FDS");
}

#[test]
fn test_parse_listen_fds_zero_count_returns_empty() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::set_var("LISTEN_PID", process::id().to_string());
    env::set_var("LISTEN_FDS", "0");

    let result = parse_listen_fds(false).unwrap();
    assert!(result.is_empty());

    env::remove_var("LISTEN_PID");
    env::remove_var("LISTEN_FDS");
}

#[test]
fn test_sd_listen_fds_start_constant() {
    assert_eq!(SD_LISTEN_FDS_START, 3);
}

#[test]
fn test_parse_listen_fds_exceeding_max_returns_error() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::set_var("LISTEN_PID", process::id().to_string());
    env::set_var("LISTEN_FDS", "4097");

    let result = parse_listen_fds(false);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        sentry_driver::activation::ActivationError::InvalidFdCount(..)
    ));

    env::remove_var("LISTEN_PID");
    env::remove_var("LISTEN_FDS");
}
