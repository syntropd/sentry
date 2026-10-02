//! 1:1 QA tests for configuration loader, validator, and systemd-creds integration.

use sentry_daemon::config::{
    load_daemon_config, validate_configuration, DaemonConfig, MAX_CONFIG_FILE_SIZE,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_default_daemon_config() {
    let config = DaemonConfig::default();
    assert_eq!(config.socket_path, "/run/systemd-sentry/sentry.sock");
    assert_eq!(config.rss_limit_mb, 15);
    assert_eq!(config.rss_degraded_mb, 13);
    assert_eq!(config.rss_recover_mb, 11);
    assert_eq!(config.provider.kind, sentry_diagnostic::ProviderKind::OpenAi);
    assert_eq!(config.provider.base_url, "http://127.0.0.1:32768/v1");
    assert_eq!(config.provider.model, "fast");

    let report = validate_configuration(&config);
    assert!(report.is_valid);
}

#[test]
fn test_validator_rejects_inverted_hysteresis() {
    let config = DaemonConfig {
        rss_degraded_mb: 16, // degraded > limit (invalid)
        ..Default::default()
    };
    let report = validate_configuration(&config);
    assert!(!report.is_valid);

    let config2 = DaemonConfig {
        rss_recover_mb: 14, // recover > degraded (invalid)
        ..Default::default()
    };
    let report2 = validate_configuration(&config2);
    assert!(!report2.is_valid);
}

#[test]
fn test_reject_oversized_config_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("oversized.toml");

    let giant_data = vec![b' '; (MAX_CONFIG_FILE_SIZE + 10) as usize];
    fs::write(&file_path, giant_data).unwrap();

    let result = load_daemon_config(Some(file_path.to_str().unwrap()));
    assert!(result.is_err());
}

#[test]
fn test_systemd_creds_directory_discovery() {
    let dir = tempdir().unwrap();
    let creds_dir = dir.path().join("credentials");
    fs::create_dir_all(&creds_dir).unwrap();

    let key_file = creds_dir.join("openai_api_key");
    fs::write(&key_file, "sk-systemd-creds-decrypted-secret\n").unwrap();

    let prev = std::env::var("CREDENTIALS_DIRECTORY").ok();
    std::env::set_var("CREDENTIALS_DIRECTORY", &creds_dir);

    let config = load_daemon_config(Some("/nonexistent/path.toml")).unwrap();
    assert_eq!(
        config.provider.api_key.as_deref(),
        Some("sk-systemd-creds-decrypted-secret")
    );

    match prev {
        Some(v) => std::env::set_var("CREDENTIALS_DIRECTORY", v),
        None => std::env::remove_var("CREDENTIALS_DIRECTORY"),
    }
}

#[tokio::test]
async fn test_setup_wizard_execution() {
    let exit_code = sentry_daemon::commands::execute_setup().await;
    assert_eq!(exit_code, sentry_daemon::cli::EX_OK);
}

#[tokio::test]
async fn test_setup_probe_routerd_detected() {
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:32768").await {
        Ok(l) => l,
        Err(_) => return,
    };

    let server_task = tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let body = r#"{"object":"list","data":[{"id":"fast"}]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes()).await;
        }
    });

    let exit_code = sentry_daemon::commands::execute_setup().await;
    assert_eq!(exit_code, sentry_daemon::cli::EX_OK);
    let _ = server_task.await;
}
