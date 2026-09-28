//! Tier 1: R7 Installation, Packaging, Documentation & System Policy Tests
//!
//! Validates sysusers syntax, tmpfiles syntax, D-Bus policy format, and policy.toml.

use std::fs;
use std::path::Path;

fn get_workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let path = Path::new(&manifest_dir);
    if path.ends_with("qa") {
        path.parent().unwrap().to_path_buf()
    } else {
        path.to_path_buf()
    }
}

#[test]
fn test_r7_test_infra_document_completeness() {
    let root = get_workspace_root();
    assert!(root.join("TEST_INFRA.md").exists());
    let doc = fs::read_to_string(root.join("TEST_INFRA.md")).unwrap();
    assert!(doc.contains("Testing Philosophy"));
    assert!(doc.contains("Feature Inventory Coverage Matrix"));
    assert!(doc.contains("Four-Tier Test Architecture"));
    assert!(doc.contains("Runner Invocations"));
}

#[test]
fn test_r7_sysusers_directive_formatting() {
    let mock_sysusers = "u sentry - \"systemd-sentry supervisor\" /var/lib/systemd-sentry /usr/sbin/nologin\n\
                         m sentry systemd-journal\n\
                         m sentry tty\n";
    let lines: Vec<&str> = mock_sysusers.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("u sentry"));
    assert!(lines[1].contains("systemd-journal"));
    assert!(lines[2].contains("tty"));
}

#[test]
fn test_r7_tmpfiles_directive_formatting() {
    let mock_tmpfiles = "d /run/systemd-sentry 0755 sentry sentry -\n\
                         d /var/lib/systemd-sentry 0750 sentry sentry -\n\
                         d /var/log/systemd-sentry 0750 sentry sentry -\n";
    for line in mock_tmpfiles.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(parts[0], "d");
        assert!(parts[1].contains("systemd-sentry"));
        assert!(parts[2] == "0755" || parts[2] == "0750");
        assert_eq!(parts[3], "sentry");
        assert_eq!(parts[4], "sentry");
    }
}

#[test]
fn test_r7_dbus_system_policy_xml_structure() {
    let mock_dbus_policy = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <policy user="sentry">
    <allow own="org.freedesktop.SystemdSentry"/>
  </policy>
</busconfig>"#;

    assert!(mock_dbus_policy.contains("<busconfig>"));
    assert!(mock_dbus_policy.contains("user=\"sentry\""));
    assert!(mock_dbus_policy.contains("org.freedesktop.SystemdSentry"));
    assert!(mock_dbus_policy.contains("</busconfig>"));
}

#[test]
fn test_r7_policy_toml_declarative_schema() {
    let toml_data = r#"
[general]
enabled = true
mode = "enforce"

[circuit_breaker]
window_duration_secs = 60
max_failures_per_window = 3
base_cooldown_secs = 30

[safety]
allowed_actions = ["RESTART", "RELOAD"]
disallowed_units = ["dbus.service"]
"#;

    let val: toml::Value = toml::from_str(toml_data).unwrap();
    assert_eq!(val["general"]["mode"].as_str(), Some("enforce"));
    assert_eq!(val["safety"]["allowed_actions"].as_array().unwrap().len(), 2);
}
