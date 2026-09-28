//! 1:1 Unit QA tests for watchdog configuration parsing.

use sentry_driver::notify::parse_watchdog_config;
use std::env;
use std::process;
use std::sync::Mutex;
use std::time::Duration;

/// Process-wide lock to serialize tests that mutate notification environment variables.
pub static NOTIFY_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_parse_watchdog_config_valid() {
    let _guard = NOTIFY_ENV_LOCK.lock().unwrap();
    let pid = process::id();
    env::set_var("WATCHDOG_USEC", "10000000"); // 10 seconds
    env::set_var("WATCHDOG_PID", pid.to_string());

    let config = parse_watchdog_config(false)
        .expect("Valid configuration must parse")
        .expect("Must produce Some(WatchdogConfig)");

    assert_eq!(config.raw_usec, 10000000);
    // Interval should be half the watchdog interval: 5 seconds
    assert_eq!(config.interval, Duration::from_secs(5));
    assert_eq!(config.target_pid, Some(pid));

    env::remove_var("WATCHDOG_USEC");
    env::remove_var("WATCHDOG_PID");
}

#[test]
fn test_parse_watchdog_config_pid_mismatch() {
    let _guard = NOTIFY_ENV_LOCK.lock().unwrap();
    let mismatch_pid = process::id() + 99999;
    env::set_var("WATCHDOG_USEC", "2000000");
    env::set_var("WATCHDOG_PID", mismatch_pid.to_string());

    let config = parse_watchdog_config(false).unwrap();
    assert!(config.is_none());

    env::remove_var("WATCHDOG_USEC");
    env::remove_var("WATCHDOG_PID");
}

#[test]
fn test_parse_watchdog_config_zero_or_unset() {
    let _guard = NOTIFY_ENV_LOCK.lock().unwrap();
    env::remove_var("WATCHDOG_USEC");
    assert!(parse_watchdog_config(false).unwrap().is_none());

    env::set_var("WATCHDOG_USEC", "0");
    assert!(parse_watchdog_config(false).unwrap().is_none());
    env::remove_var("WATCHDOG_USEC");
}

#[test]
fn test_parse_watchdog_config_invalid_integer() {
    let _guard = NOTIFY_ENV_LOCK.lock().unwrap();
    env::set_var("WATCHDOG_USEC", "not_a_number");
    let err = parse_watchdog_config(false).unwrap_err();
    assert!(err.to_string().contains("Failed to parse WATCHDOG_USEC"));
    env::remove_var("WATCHDOG_USEC");
}

#[test]
fn test_parse_watchdog_config_usec_one_clamped() {
    let _guard = NOTIFY_ENV_LOCK.lock().unwrap();
    env::set_var("WATCHDOG_USEC", "1");
    let config = parse_watchdog_config(false)
        .expect("Valid configuration")
        .expect("Must produce Some(WatchdogConfig)");
    assert_eq!(config.raw_usec, 1);
    assert_eq!(config.interval, Duration::from_micros(1));
    env::remove_var("WATCHDOG_USEC");
}
