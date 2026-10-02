//! Systemd-timesyncd client for NTP synchronization verification.

use sentry_core::error::DriverError;
use sentry_core::models::TimeSyncStatus;
use zbus::Connection;

/// Client querying `org.freedesktop.timesync1` for NTP synchronization.
pub struct TimesyncClient {
    conn: Connection,
}

impl TimesyncClient {
    /// Creates a new timesync client.
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// Connects to the system bus.
    pub async fn connect_system() -> Result<Self, DriverError> {
        let conn = Connection::system()
            .await
            .map_err(|e| DriverError::Io(std::io::Error::other(e.to_string())))?;
        Ok(Self::new(conn))
    }

    /// Queries the current NTP synchronization status from `org.freedesktop.timesync1.Manager`.
    pub async fn query_sync_status(&self) -> TimeSyncStatus {
        let reply = self
            .conn
            .call_method(
                Some("org.freedesktop.timesync1"),
                "/org/freedesktop/timesync1",
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &("org.freedesktop.timesync1.Manager", "ServerName"),
            )
            .await;

        match reply {
            Ok(msg) => {
                if let Ok((val,)) = msg.body().deserialize::<(zbus::zvariant::OwnedValue,)>() {
                    if let Ok(server) = <&str>::try_from(&val) {
                        if !server.is_empty() {
                            return TimeSyncStatus::Synchronized;
                        }
                    }
                }
                TimeSyncStatus::Unsynchronized
            }
            Err(_) => TimeSyncStatus::Unsynchronized,
        }
    }

    /// Checks if a log entry suggests TLS certificate validation failure due to clock skew.
    pub fn is_clock_drift_error(log_line: &str) -> bool {
        let lower = log_line.to_ascii_lowercase();
        lower.contains("cert_date_invalid")
            || lower.contains("certificate has expired or is not yet valid")
            || lower.contains("ssl_error_cert_expired")
            || lower.contains("clock skew")
            || lower.contains("time out of sync")
            || lower.contains("certificate is not yet valid")
    }
}
