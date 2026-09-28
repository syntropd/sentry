//! Command: Run the persistent supervisor daemon.

use crate::config::daemon_config::DaemonConfig;
use crate::daemon::supervisor::run_supervisor;
use tracing::error;

/// Execute the `daemon` subcommand.
pub async fn execute_daemon(config: DaemonConfig) -> i32 {
    match run_supervisor(config).await {
        Ok(_) => crate::cli::exit_codes::EX_OK,
        Err(e) => {
            eprintln!("Daemon terminated with error: {}", e);
            error!("Daemon error: {}", e);
            crate::cli::exit_codes::EX_SOFTWARE
        }
    }
}
