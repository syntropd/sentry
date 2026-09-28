//! Tests for systemd-timesyncd error patterns and clock drift detection.

use sentry_driver::timesync::TimesyncClient;

#[test]
fn test_timesync_clock_drift_error_detection() {
    assert!(TimesyncClient::is_clock_drift_error("OpenSSL error: CERT_DATE_INVALID"));
    assert!(TimesyncClient::is_clock_drift_error("certificate has expired or is not yet valid"));
    assert!(TimesyncClient::is_clock_drift_error("TLS alert: SSL_ERROR_CERT_EXPIRED"));
    assert!(TimesyncClient::is_clock_drift_error("Raft cluster: clock skew detected between peers"));
    assert!(TimesyncClient::is_clock_drift_error("CRITICAL: system time out of sync"));

    assert!(!TimesyncClient::is_clock_drift_error("Bad credentials"));
    assert!(!TimesyncClient::is_clock_drift_error("Syntax error in config"));
}

#[test]
fn test_timesync_not_yet_valid_pattern_and_case_insensitive() {
    assert!(TimesyncClient::is_clock_drift_error(
        "server certificate is not yet valid"
    ));
    assert!(TimesyncClient::is_clock_drift_error("CERT_DATE_INVALID"));
    assert!(TimesyncClient::is_clock_drift_error("CLOCK SKEW detected between peers"));

    assert!(!TimesyncClient::is_clock_drift_error("certificate revoked by issuer"));
    assert!(!TimesyncClient::is_clock_drift_error(""));
}
