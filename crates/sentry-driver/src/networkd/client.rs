//! Systemd-networkd client and carrier status reader.

use sentry_core::error::DriverError;
use sentry_core::models::NetworkOperationalState;
use zbus::Connection;

/// Client interacting with `org.freedesktop.network1` for network status.
pub struct NetworkdClient {
    conn: Connection,
}

impl NetworkdClient {
    /// Creates a new networkd client with an existing connection.
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

    /// Queries the global `OperationalState` property from `org.freedesktop.network1.Manager`.
    pub async fn query_operational_state(&self) -> NetworkOperationalState {
        let reply = self
            .conn
            .call_method(
                Some("org.freedesktop.network1"),
                "/org/freedesktop/network1",
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &("org.freedesktop.network1.Manager", "OperationalState"),
            )
            .await;

        match reply {
            Ok(msg) => {
                if let Ok((val,)) = msg.body().deserialize::<(zbus::zvariant::OwnedValue,)>() {
                    if let Ok(state_str) = <&str>::try_from(&val) {
                        return NetworkOperationalState::from_dbus_str(state_str);
                    }
                }
                NetworkOperationalState::Unknown
            }
            Err(_) => NetworkOperationalState::Unknown,
        }
    }

    /// Checks if a log line indicates physical or logical network disconnection.
    pub fn is_network_down_error(log_line: &str) -> bool {
        let lower = log_line.to_ascii_lowercase();
        lower.contains("network is unreachable")
            || lower.contains("network down")
            || lower.contains("no route to host")
            || lower.contains("carrier lost")
            || lower.contains("link down")
            || lower.contains("ehostunreach")
            || lower.contains("enetunreach")
    }
}
