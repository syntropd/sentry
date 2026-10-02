//! Command-line argument parser for systemd-sentry.
//!
//! Pure Rust parser conforming to POSIX argument conventions.

use crate::cli::args::{CliArgs, Command};
use crate::cli::exit_codes::{EX_OK, EX_USAGE};
use crate::cli::help::{format_global_help, format_subcommand_help};

/// Parse a slice of string arguments (skipping `argv[0]`) into `CliArgs`.
pub fn parse_cli_args(args: &[String]) -> Result<CliArgs, (String, i32)> {
    if args.is_empty() {
        return Ok(CliArgs::default());
    }

    let has_help = args.iter().any(|a| a == "-h" || a == "--help");
    let has_version = args.iter().any(|a| a == "-V" || a == "--version");

    // Extract global path options (-c/--config, -s/--socket) and remaining arguments
    let mut config_path = None;
    let mut socket_path = None;
    let mut remaining = Vec::new();
    let mut idx = 0;

    while idx < args.len() {
        let arg = &args[idx];
        if arg == "-c" || arg == "--config" {
            if idx + 1 < args.len() {
                idx += 1;
                config_path = Some(args[idx].clone());
            } else {
                return Err((
                    format!("Option '{}' requires a path value.\nRun 'systemd-sentry --help' for usage.", arg),
                    EX_USAGE,
                ));
            }
        } else if arg == "-s" || arg == "--socket" {
            if idx + 1 < args.len() {
                idx += 1;
                socket_path = Some(args[idx].clone());
            } else {
                return Err((
                    format!("Option '{}' requires a socket path value.\nRun 'systemd-sentry --help' for usage.", arg),
                    EX_USAGE,
                ));
            }
        } else {
            remaining.push(arg.as_str());
        }
        idx += 1;
    }

    if has_help {
        let sub = remaining
            .iter()
            .find(|&&a| a != "-h" && a != "--help" && !a.starts_with('-'));
        let help_text = match sub {
            Some(&subcmd) => format_subcommand_help(subcmd),
            None => format_global_help(),
        };
        return Err((help_text, EX_OK));
    }

    if has_version {
        return Err((format!("systemd-sentry {}", env!("CARGO_PKG_VERSION")), EX_OK));
    }

    let tokens: Vec<&str> = remaining
        .into_iter()
        .filter(|&a| a != "-h" && a != "--help")
        .collect();

    if tokens.is_empty() {
        return Ok(CliArgs { config_path, socket_path, command: Command::Daemon });
    }

    if tokens[0] == "--setup" {
        return Ok(CliArgs { config_path, socket_path, command: Command::Setup });
    }

    let subcmd = tokens[0];
    let sub_args = &tokens[1..];

    let command = match subcmd {
        "daemon" => Command::Daemon,
        "status" => {
            let json = sub_args.contains(&"--json");
            Command::Status { json }
        }
        "check" => {
            let verbose = sub_args.iter().any(|&a| a == "-v" || a == "--verbose");
            Command::Check { verbose }
        }
        "triage" => {
            let unit_opt = sub_args.iter().find(|&&a| !a.starts_with('-'));
            let Some(&unit) = unit_opt else {
                return Err((
                    "Missing required argument <UNIT> for 'triage'.\nUsage: systemd-sentry triage <UNIT> [--json]".to_string(),
                    EX_USAGE,
                ));
            };
            let json = sub_args.contains(&"--json");
            Command::Triage { unit: unit.to_string(), json }
        }
        "monitor" => Command::Monitor,
        "incidents" => {
            let mut limit = 20;
            let mut json = false;
            let mut i = 0;
            while i < sub_args.len() {
                if sub_args[i] == "--json" {
                    json = true;
                } else if sub_args[i] == "--limit" && i + 1 < sub_args.len() {
                    i += 1;
                    limit = sub_args[i].parse().unwrap_or(20);
                }
                i += 1;
            }
            Command::Incidents { limit, json }
        }
        "inspect" => {
            let id_opt = sub_args.iter().find(|&&a| !a.starts_with('-'));
            let Some(&id) = id_opt else {
                return Err((
                    "Missing required argument <ID> for 'inspect'.\nUsage: systemd-sentry inspect <ID> [--json]".to_string(),
                    EX_USAGE,
                ));
            };
            let json = sub_args.contains(&"--json");
            Command::Inspect { id: id.to_string(), json }
        }
        "reset" => {
            let unit_opt = sub_args.iter().find(|&&a| !a.starts_with('-'));
            let Some(&unit) = unit_opt else {
                return Err((
                    "Missing required argument <UNIT> for 'reset'.\nUsage: systemd-sentry reset <UNIT>".to_string(),
                    EX_USAGE,
                ));
            };
            Command::Reset { unit: unit.to_string() }
        }
        "mcp" => Command::Mcp,
        "completions" => {
            let shell_opt = sub_args.iter().find(|&&a| !a.starts_with('-'));
            let Some(&shell) = shell_opt else {
                return Err((
                    "Missing required argument <SHELL> for 'completions'.\nUsage: systemd-sentry completions <bash|zsh|fish>".to_string(),
                    EX_USAGE,
                ));
            };
            Command::Completions { shell: shell.to_string() }
        }
        "setup" => Command::Setup,
        "version" => Command::Version,
        "help" => {
            let sub = sub_args.first().copied();
            let text = sub.map(format_subcommand_help).unwrap_or_else(format_global_help);
            return Err((text, EX_OK));
        }
        other if other.starts_with('-') => {
            return Err((
                format!("Unknown option: {}\nRun 'systemd-sentry --help' for usage.", other),
                EX_USAGE,
            ));
        }
        other => {
            return Err((
                format!("Unknown subcommand '{}'.\nRun 'systemd-sentry --help' for usage.", other),
                EX_USAGE,
            ));
        }
    };

    Ok(CliArgs { config_path, socket_path, command })
}
