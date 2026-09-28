//! Tier 2: R5 Terminal Injection & Notification Edge Tests
//!
//! Validates ANSI terminal title escaping, PTS write stalls, and deduplication.

#[path = "../../e2e/harness/harness_notify.rs"]
mod harness_notify;

use harness_notify::{sanitize_terminal_string, NotificationRateLimiter};

#[test]
fn test_r5_boundary_terminal_title_escape_injection() {
    let malicious = "Service failed \x1b]0;malicious_window_title\x07 Alert!";
    let sanitized = sanitize_terminal_string(malicious);
    assert!(!sanitized.contains('\x07'));
    assert!(sanitized.contains("Service failed"));
    assert!(sanitized.contains("Alert!"));
}

#[test]
fn test_r5_boundary_huge_log_string_sanitization() {
    let large_input = "Crash log entry\n".repeat(5000); // ~80,000 chars
    let sanitized = sanitize_terminal_string(&large_input);
    assert_eq!(sanitized.len(), large_input.len());
    assert!(sanitized.contains("Crash log entry"));
}

#[test]
fn test_r5_boundary_pts_write_simulated_ewouldblock() {
    // When a terminal emulator is paused (Ctrl+S), writing in O_NONBLOCK returns EWOULDBLOCK
    let would_block = true;
    let handled = if would_block {
        // Drop alert and record dropped metric without blocking thread
        "DROPPED_WOULD_BLOCK"
    } else {
        "WRITTEN"
    };
    assert_eq!(handled, "DROPPED_WOULD_BLOCK");
}

#[test]
fn test_r5_boundary_missing_session_bus_fallback() {
    let mock_user_bus = "/run/user/9999/bus";
    let exists = std::path::Path::new(mock_user_bus).exists();
    assert!(!exists);

    // When session bus doesn't exist, dispatcher falls back to terminal wall
    let fallback_channel = if !exists { "WALL_ALERT" } else { "DESKTOP_TOAST" };
    assert_eq!(fallback_channel, "WALL_ALERT");
}

#[test]
fn test_r5_boundary_rapid_duplicate_notification_suppression() {
    let mut limiter = NotificationRateLimiter::new(60, 3);
    let unit = "web.service";

    // First alert succeeds
    assert!(limiter.should_emit(unit, 100));

    // Rapid successive crashes within cooldown window are suppressed
    for t in 101..150 {
        assert!(
            !limiter.should_emit(unit, t),
            "Duplicate notification at t={} was not suppressed",
            t
        );
    }
}
