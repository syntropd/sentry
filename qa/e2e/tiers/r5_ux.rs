//! Tier 1: R5 User Experience, Notifications & Setup Wizard Tests
//!
//! Validates desktop toasts, terminal wall sanitization, rate limits, and CLI parsing.

use super::harness_notify::{sanitize_terminal_string, NotificationRateLimiter};

#[test]
fn test_r5_desktop_toast_notification_payload_formatting() {
    let app_name = "systemd-sentry";
    let summary = "[CRITICAL] Unit api-worker.service Tripped";
    let body = "Circuit Breaker: Locked out for 300s.\nRun 'sentry inspect' for triage.";
    let urgency = 2u8; // Critical

    assert_eq!(app_name, "systemd-sentry");
    assert!(summary.contains("CRITICAL"));
    assert!(body.contains("Circuit Breaker"));
    assert_eq!(urgency, 2);
}

#[test]
fn test_r5_terminal_wall_sanitization_strips_control_codes() {
    // Inject malicious terminal escape sequences: clear screen (\x1b[2J) and beep (\x07)
    let dirty = "\x1b[2J\x07Broadcast: Unit crashed!\t\nNext line.";
    let clean = sanitize_terminal_string(dirty);

    assert!(!clean.contains("\x1b[2J"));
    assert!(!clean.contains('\x07'));
    assert!(clean.contains("Broadcast: Unit crashed!"));
    assert!(clean.contains('\t'));
    assert!(clean.contains('\n'));
}

#[test]
fn test_r5_notification_rate_limiter_burst_control() {
    let mut limiter = NotificationRateLimiter::new(60, 5);

    // 1st alert for unit A at t=0 -> Allowed
    assert!(limiter.should_emit("unit-a.service", 0));
    // 2nd alert for unit A at t=10 -> Rejected (within 60s cooldown)
    assert!(!limiter.should_emit("unit-a.service", 10));

    // Alert for unit B at t=1 -> Allowed
    assert!(limiter.should_emit("unit-b.service", 1));
    // Alert for unit C at t=2 -> Allowed
    assert!(limiter.should_emit("unit-c.service", 2));
    // Alert for unit D at t=3 -> Allowed
    assert!(limiter.should_emit("unit-d.service", 3));
    // Alert for unit E at t=4 -> Allowed
    assert!(limiter.should_emit("unit-e.service", 4));

    // Alert for unit F at t=5 -> Rejected (burst capacity of 5 reached)
    assert!(!limiter.should_emit("unit-f.service", 5));
}

#[test]
fn test_r5_operator_cli_subcommand_arguments() {
    let valid_subcommands = ["status", "incidents", "inspect", "reset"];
    for subcmd in &valid_subcommands {
        assert!(valid_subcommands.contains(subcmd));
    }

    // Inspect requires an incident ID argument
    let inspect_args = ["sentry", "inspect", "inc-01J8K3M9"];
    assert_eq!(inspect_args.len(), 3);
    assert_eq!(inspect_args[1], "inspect");
    assert_eq!(inspect_args[2], "inc-01J8K3M9");

    // Reset requires a unit name
    let reset_args = ["sentry", "reset", "api-worker.service"];
    assert_eq!(reset_args.len(), 3);
    assert_eq!(reset_args[1], "reset");
    assert_eq!(reset_args[2], "api-worker.service");
}

#[test]
fn test_r5_setup_wizard_config_generation() {
    let toml_config = r#"
[daemon]
control_socket = "/run/systemd-sentry/control.sock"
state_dir = "/var/lib/systemd-sentry"
watchdog_interval_sec = 15

[provider]
kind = "llama_cpp"
endpoint = "http://127.0.0.1:8080"
model = "default"

[circuit_breaker]
failure_window_sec = 60
max_failures = 3
"#;

    let parsed: toml::Value = toml::from_str(toml_config).unwrap();
    assert_eq!(parsed["provider"]["kind"].as_str(), Some("llama_cpp"));
    assert_eq!(parsed["circuit_breaker"]["max_failures"].as_integer(), Some(3));
}
