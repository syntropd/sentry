//! 1:1 Unit QA tests for LISTEN_PID validation.

use super::ACTIVATION_ENV_LOCK;
use sentry_driver::activation::validate_listen_pid;
use std::env;
use std::process;

#[test]
fn test_validate_listen_pid_matching() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::set_var("LISTEN_PID", process::id().to_string());
    assert!(validate_listen_pid().unwrap());
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_validate_listen_pid_mismatching() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    let wrong_pid = process::id() + 42424;
    env::set_var("LISTEN_PID", wrong_pid.to_string());
    assert!(!validate_listen_pid().unwrap());
    env::remove_var("LISTEN_PID");
}

#[test]
fn test_validate_listen_pid_unset() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::remove_var("LISTEN_PID");
    assert!(!validate_listen_pid().unwrap());
}

#[test]
fn test_validate_listen_pid_invalid_integer() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::set_var("LISTEN_PID", "invalid_pid");
    let err = validate_listen_pid().unwrap_err();
    assert!(err.to_string().contains("Failed to parse LISTEN_PID"));
    env::remove_var("LISTEN_PID");
}
