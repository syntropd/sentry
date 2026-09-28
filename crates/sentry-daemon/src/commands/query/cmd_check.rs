//! Command: Validate configuration and drop-in safety policies.

use crate::cli::exit_codes::{EX_CONFIG, EX_OK};
use crate::config::daemon_config::DaemonConfig;
use crate::config::validator::validate_configuration;

/// Execute the `check` subcommand.
pub fn execute_check(config: &DaemonConfig, verbose: bool) -> i32 {
    let report = validate_configuration(config);

    if report.is_valid {
        println!("OK: Configuration and drop-in policies are valid.");
        if verbose {
            for msg in &report.messages {
                println!("  [+] {}", msg);
            }
        }
        EX_OK
    } else {
        eprintln!("FAILURE: Configuration or policy validation failed:");
        for msg in &report.messages {
            eprintln!("  [-] {}", msg);
        }
        EX_CONFIG
    }
}
