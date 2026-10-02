//! Pure Rust remediation executor using systemd D-Bus.
//!
//! Strictly verifies proposed actions against PolicyGatekeeper before execution.

use sentry_core::models::{ProposedRemediation, RemediationAction};
use sentry_safety::policy::PolicyGatekeeper;
use tracing::{error, info, warn};
use zbus::Connection;

/// Executes authorized remediations against systemd units.
#[derive(Clone)]
pub struct RemediationExecutor {
    gatekeeper: PolicyGatekeeper,
    connection: Option<Connection>,
}

impl RemediationExecutor {
    /// Create a new executor wrapping the active policy gatekeeper.
    pub fn new(gatekeeper: PolicyGatekeeper) -> Self {
        Self {
            gatekeeper,
            connection: None,
        }
    }

    /// Create an executor with an existing system D-Bus connection to avoid reconnecting.
    pub fn with_connection(gatekeeper: PolicyGatekeeper, connection: Connection) -> Self {
        Self {
            gatekeeper,
            connection: Some(connection),
        }
    }

    /// Update the internal policy gatekeeper (e.g. during SIGHUP reload).
    pub fn update_gatekeeper(&mut self, gatekeeper: PolicyGatekeeper) {
        self.gatekeeper = gatekeeper;
    }

    /// Execute a remediation action validating strictly against the provided policy gatekeeper.
    pub async fn execute_with_gatekeeper(
        &self,
        unit: &str,
        remediation: &ProposedRemediation,
        gatekeeper: &PolicyGatekeeper,
    ) -> Result<String, String> {
        // 1. Validate through zero-trust policy gatekeeper
        if let Err(e) = gatekeeper.validate(unit, remediation.action) {
            warn!("Remediation for unit {} blocked by policy: {}", unit, e);
            return Err(format!("Blocked by policy: {}", e));
        }

        // 2. Reuse shared connection or connect to system D-Bus
        let fallback_conn;
        let connection = match &self.connection {
            Some(c) => c,
            None => {
                fallback_conn = Connection::system().await.map_err(|e| {
                    format!("Failed to connect to system D-Bus: {}", e)
                })?;
                &fallback_conn
            }
        };

        match remediation.action {
            RemediationAction::Restart | RemediationAction::RestartWithBackoff => {
                info!("Executing systemd RestartUnit on {}", unit);
                match sentry_driver::dbus::call_systemd_unit_method(connection, "RestartUnit", unit, "replace").await {
                    Ok(path) => Ok(format!("Dispatched RestartUnit: {}", path)),
                    Err(e) => {
                        error!("RestartUnit failed for {}: {}", unit, e);
                        Err(format!("D-Bus call RestartUnit failed: {}", e))
                    }
                }
            }
            RemediationAction::ResetFailed => {
                info!("Executing systemd ResetFailedUnit on {}", unit);
                match sentry_driver::dbus::call_systemd_unit_method(connection, "ResetFailedUnit", unit, "").await {
                    Ok(_) => Ok(format!("ResetFailedUnit succeeded for {}", unit)),
                    Err(e) => {
                        error!("ResetFailedUnit failed for {}: {}", unit, e);
                        Err(format!("D-Bus call ResetFailedUnit failed: {}", e))
                    }
                }
            }
            RemediationAction::Reload => {
                info!("Executing systemd ReloadUnit on {}", unit);
                match sentry_driver::dbus::call_systemd_unit_method(connection, "ReloadUnit", unit, "replace").await {
                    Ok(path) => Ok(format!("Dispatched ReloadUnit: {}", path)),
                    Err(e) => {
                        error!("ReloadUnit failed for {}: {}", unit, e);
                        Err(format!("D-Bus call ReloadUnit failed: {}", e))
                    }
                }
            }
            RemediationAction::NoAction | RemediationAction::EscalateToAdmin => {
                info!("EscalateToAdmin / NoAction for {}: no automatic remediation taken", unit);
                Ok("No automatic action taken; observation only".to_string())
            }
        }
    }

    /// Execute a remediation action if permitted by safety policy.
    pub async fn execute(
        &self,
        unit: &str,
        remediation: &ProposedRemediation,
    ) -> Result<String, String> {
        self.execute_with_gatekeeper(unit, remediation, &self.gatekeeper).await
    }
}
