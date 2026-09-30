//! Command: Interactive setup wizard detecting LLM backends and generating configuration.

use crate::cli::exit_codes::{EX_IOERR, EX_OK};
use crate::config::daemon_config::DaemonConfig;
use sentry_diagnostic::provider::{ProviderConfig, ProviderKind};
use std::fs;
use std::path::Path;

/// Execute the `setup` wizard subcommand.
pub async fn execute_setup() -> i32 {
    println!("============================================================");
    println!(" systemd-sentry: Interactive Configuration Setup Wizard");
    println!("============================================================");
    println!("Scanning host environment for inference providers...\n");

    let mut detected_kind = None;
    let mut detected_endpoint = None;

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    // 1. Probe for active routerd
    if let Ok(res) = http.get("http://127.0.0.1:32768/v1/models").send().await {
        if res.status().is_success() {
            println!("  [+] Detected active local routerd instance on http://127.0.0.1:32768/v1");
            detected_kind = Some(ProviderKind::OpenAi);
            detected_endpoint = Some("http://127.0.0.1:32768/v1".to_string());
        }
    }

    // 2. Probe for local llama.cpp
    if detected_kind.is_none() {
        if let Ok(res) = http.get("http://127.0.0.1:8080/health").send().await {
            if res.status().is_success() {
                println!("  [+] Detected active local llama.cpp server on http://127.0.0.1:8080");
                detected_kind = Some(ProviderKind::LlamaCpp);
                detected_endpoint = Some("http://127.0.0.1:8080".to_string());
            }
        }
    }

    // 3. Check for OpenAI API key in environment
    let has_openai_key = std::env::var("OPENAI_API_KEY").is_ok();
    if has_openai_key && detected_kind.is_none() {
        println!("  [+] Detected OPENAI_API_KEY environment variable");
        detected_kind = Some(ProviderKind::OpenAi);
        detected_endpoint = Some("https://api.openai.com/v1".to_string());
    }

    if detected_kind.is_none() {
        println!("  [*] No local LLM detected. Defaulting to routerd with deterministic fallback.");
        println!("      (Offline-first fallback triage if routerd is unreachable)");
    }

    let kind = detected_kind.unwrap_or(ProviderKind::OpenAi);
    let base_url = detected_endpoint.unwrap_or_else(|| "http://127.0.0.1:32768/v1".to_string());

    // 4. Construct proposed configuration
    let mut config = DaemonConfig::default();
    config.provider = ProviderConfig {
        kind,
        base_url: base_url.clone(),
        api_key: None,
        model: match kind {
            ProviderKind::LlamaCpp => "default".to_string(),
            ProviderKind::OpenAi => {
                if base_url.starts_with("https://api.openai.com") {
                    "gpt-4o-mini".to_string()
                } else {
                    "fast".to_string()
                }
            }
        },
        timeout: std::time::Duration::from_secs(45),
        temperature: 0.1,
        adaptive: Default::default(),
        reasoning_effort: Some("low".to_string()),
    };

    println!("\nProposed /etc/systemd-sentry/config.toml:");
    println!("------------------------------------------------------------");
    let serialized = toml::to_string_pretty(&config).unwrap_or_default();
    println!("{}", serialized);
    println!("------------------------------------------------------------");

    // 5. Offer to write configuration file
    let target_dir = Path::new("/etc/systemd-sentry");
    let target_file = target_dir.join("config.toml");

    let is_root = rustix::process::getuid().as_raw() == 0;
    if is_root {
        if let Err(e) = fs::create_dir_all(target_dir) {
            eprintln!("Failed to create /etc/systemd-sentry directory: {}", e);
            return EX_IOERR;
        }
        if let Err(e) = fs::write(&target_file, &serialized) {
            eprintln!("Failed to write {}: {}", target_file.display(), e);
            return EX_IOERR;
        }
        println!("\n[SUCCESS] Configuration saved to {}", target_file.display());
        println!("To start supervisor: systemctl enable --now systemd-sentry");
    } else {
        println!("\n[INFO] Running unprivileged (UID != 0). To install configuration:");
        println!("  sudo mkdir -p /etc/systemd-sentry");
        println!("  echo '{}' | sudo tee /etc/systemd-sentry/config.toml", serialized.trim());
    }

    EX_OK
}
