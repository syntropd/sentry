//! User-level systemd session and bus discoverer for rootless services.

use sentry_core::error::DriverError;
use std::path::{Path, PathBuf};
use zbus::Connection;

/// Discoverer for active rootless user sessions and D-Bus instances.
pub struct UserSessionDiscoverer;

impl UserSessionDiscoverer {
    /// Discovers all active user UIDs with an existing runtime directory in `/run/user`.
    pub fn discover_active_uids() -> Vec<u32> {
        Self::discover_active_uids_in_path(Path::new("/run/user"))
    }

    /// Maximum number of user session sockets discovered in a single scan.
    pub const MAX_DISCOVERED_UIDS: usize = 64;

    /// Discovers active user UIDs in a specific base directory (for testing).
    pub fn discover_active_uids_in_path(base: &Path) -> Vec<u32> {
        let mut uids = Vec::new();
        let Ok(entries) = std::fs::read_dir(base) else {
            return uids;
        };

        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(uid) = name.parse::<u32>() {
                    let bus_path = entry.path().join("bus");
                    if bus_path.exists() {
                        uids.push(uid);
                    }
                }
            }
        }
        uids.sort_unstable();
        uids.truncate(Self::MAX_DISCOVERED_UIDS);
        uids
    }

    /// Resolves the socket path for a user's systemd session bus.
    pub fn resolve_user_socket(uid: u32) -> PathBuf {
        PathBuf::from(format!("/run/user/{uid}/bus"))
    }

    /// Connects to a user session D-Bus for the current or specified user.
    pub async fn connect_user_bus(custom_path: Option<&Path>) -> Result<Connection, DriverError> {
        if let Some(path) = custom_path {
            let addr = format!("unix:path={}", path.display());
            zbus::connection::Builder::address(addr.as_str())
                .map_err(|e| DriverError::Io(std::io::Error::other(e.to_string())))?
                .build()
                .await
                .map_err(|e| DriverError::Io(std::io::Error::other(e.to_string())))
        } else {
            Connection::session()
                .await
                .map_err(|e| DriverError::Io(std::io::Error::other(e.to_string())))
        }
    }
}
