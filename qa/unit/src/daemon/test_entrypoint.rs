//! Entrypoint dispatch proofs: exit codes for version, help, usage, completions.

use sentry_daemon::cli::{EX_OK, EX_USAGE};
use sentry_daemon::entrypoint;

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn missing_config_path() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join("no-such-config.toml")
        .to_str()
        .unwrap()
        .to_string();
    (dir, path)
}

#[tokio::test]
async fn test_entrypoint_version_flag_returns_ok() {
    assert_eq!(entrypoint(&argv(&["--version"])).await, EX_OK);
}

#[tokio::test]
async fn test_entrypoint_help_flag_returns_ok() {
    assert_eq!(entrypoint(&argv(&["--help"])).await, EX_OK);
}

#[tokio::test]
async fn test_entrypoint_unknown_subcommand_returns_usage() {
    assert_eq!(entrypoint(&argv(&["bogus-subcommand"])).await, EX_USAGE);
}

#[tokio::test]
async fn test_entrypoint_version_subcommand_with_missing_config() {
    let (_dir, missing) = missing_config_path();
    let args = argv(&["--config", &missing, "version"]);
    assert_eq!(entrypoint(&args).await, EX_OK);
}

#[tokio::test]
async fn test_entrypoint_completions_bash_returns_ok() {
    let (_dir, missing) = missing_config_path();
    let args = argv(&["--config", &missing, "completions", "bash"]);
    assert_eq!(entrypoint(&args).await, EX_OK);
}
