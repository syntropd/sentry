//! 1:1 Unit QA tests for LISTEN_FDNAMES parsing.

use super::ACTIVATION_ENV_LOCK;
use sentry_driver::activation::parse_listen_fdnames;
use std::env;

#[test]
fn test_parse_listen_fdnames_exact() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::set_var("LISTEN_FDNAMES", "http.socket:admin.socket");
    let names = parse_listen_fdnames(2);
    assert_eq!(names, vec!["http.socket", "admin.socket"]);
    env::remove_var("LISTEN_FDNAMES");
}

#[test]
fn test_parse_listen_fdnames_partial() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::set_var("LISTEN_FDNAMES", "metrics.socket");
    let names = parse_listen_fdnames(3);
    assert_eq!(names, vec!["metrics.socket", "unknown:1", "unknown:2"]);
    env::remove_var("LISTEN_FDNAMES");
}

#[test]
fn test_parse_listen_fdnames_empty_or_unset() {
    let _guard = ACTIVATION_ENV_LOCK.lock().unwrap();
    env::remove_var("LISTEN_FDNAMES");
    let names = parse_listen_fdnames(2);
    assert_eq!(names, vec!["unknown:0", "unknown:1"]);
}
