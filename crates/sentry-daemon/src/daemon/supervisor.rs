//! Daemon supervisor lifecycle management and initialization.

use crate::config::daemon_config::DaemonConfig;
use crate::daemon::event_multiplexer::run_event_loop;
use crate::daemon::state::DaemonState;
use crate::ipc::listener::bind_or_activate_socket;
use crate::system::{RemediationExecutor, SignalListener};
use sentry_driver::dbus::SystemdDbusListener;
use sentry_driver::notify::{notify_ready, notify_stopping, parse_watchdog_config, WatchdogTicker};
use sentry_safety::policy::{load_policy_with_dropins, PolicyGatekeeper};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

/// Runs the complete systemd-sentry supervisor lifecycle.
pub async fn run_supervisor(config: DaemonConfig) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting systemd-sentry supervisor daemon v{}", env!("CARGO_PKG_VERSION"));

    // 1. Load safety policy and drop-ins
    let base_path = std::path::Path::new(&config.policy_path);
    let dropin_dir = std::path::Path::new(&config.policy_dropin_dir);
    let policy_config = load_policy_with_dropins(base_path, dropin_dir);
    let gatekeeper = PolicyGatekeeper::new(policy_config);

    // 2. Initialize in-memory daemon state
    let state = Arc::new(Mutex::new(DaemonState::new(config.clone(), gatekeeper.clone())));

    // 3. Bind or adopt UNIX domain socket
    let is_socket_activated = std::env::var_os("LISTEN_FDS").is_some();
    let ipc_listener = bind_or_activate_socket(&config.socket_path)?;
    info!("IPC server listening on {}", config.socket_path);

    // 4. Initialize D-Bus listener and shared remediation executor
    let dbus_listener = match SystemdDbusListener::connect_system().await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to connect to system D-Bus: {}. Terminating supervisor.", e);
            return Err(e.into());
        }
    };
    let dbus_conn = dbus_listener.connection().clone();
    let (_dbus_task, dbus_rx) = dbus_listener.spawn_event_stream();
    let remediation_executor = RemediationExecutor::with_connection(gatekeeper, dbus_conn);

    // 5. Initialize signal listeners for SIGHUP, SIGTERM, SIGINT
    let signal_listener = SignalListener::new()?;

    // 6. Spawn persistent watchdog ticker if configured by systemd
    let _watchdog = match parse_watchdog_config(false) {
        Ok(Some(w_cfg)) => {
            info!("Systemd WatchdogSec detected; spawning persistent watchdog ticker");
            Some(WatchdogTicker::spawn(w_cfg))
        }
        _ => None,
    };

    // 7. Dispatch READY=1 notification to systemd
    let _ = notify_ready();
    info!("Supervisor initialization complete; READY=1 notified");

    // 8. Run multiplexed event loop until termination
    run_event_loop(ipc_listener, state, dbus_rx, signal_listener, remediation_executor).await;

    // 9. Graceful shutdown sequence
    info!("Shutting down supervisor: notifying STOPPING=1");
    let _ = notify_stopping();

    // Clean up socket file if present on filesystem and not socket activated
    if !is_socket_activated {
        let sock_path = Path::new(&config.socket_path);
        if sock_path.exists() {
            let _ = fs::remove_file(sock_path);
        }
    }

    info!("Supervisor shutdown cleanly completed");
    Ok(())
}
