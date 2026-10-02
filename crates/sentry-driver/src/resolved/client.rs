//! Systemd-resolved client and DNS health prober.

use sentry_core::error::DriverError;
use sentry_core::models::DnsHealthState;
use zbus::Connection;

/// Queries systemd-resolved over D-Bus for DNS resolution health.
pub struct ResolvedClient {
    conn: Connection,
}

impl ResolvedClient {
    /// Creates a new resolved client using the provided D-Bus connection.
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// Creates a client connecting directly to the system D-Bus.
    pub async fn connect_system() -> Result<Self, DriverError> {
        let conn = Connection::system()
            .await
            .map_err(|e| DriverError::Io(std::io::Error::other(e.to_string())))?;
        Ok(Self::new(conn))
    }

    /// Queries DNS health by probing hostname resolution via `org.freedesktop.resolve1.Manager.ResolveHostname`.
    ///
    /// Parameters:
    /// - `ifindex`: Interface index (0 for all interfaces)
    /// - `name`: Hostname to probe (e.g. "localhost")
    /// - `flags`: DNSSEC / lookup flags (0 for standard lookup)
    pub async fn probe_dns(&self, name: &str) -> DnsHealthState {
        let ifindex: i32 = 0;
        let flags: u64 = 0;
        let family: i32 = 0; // AF_UNSPEC

        let reply = self
            .conn
            .call_method(
                Some("org.freedesktop.resolve1"),
                "/org/freedesktop/resolve1",
                Some("org.freedesktop.resolve1.Manager"),
                "ResolveHostname",
                &(ifindex, name, family, flags),
            )
            .await;

        match reply {
            Ok(_) => DnsHealthState::Operational,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("NoNameServers") || err_str.contains("NoSuchResource") {
                    DnsHealthState::Degraded
                } else if err_str.contains("ServiceUnknown") || err_str.contains("NameHasNoOwner") {
                    DnsHealthState::Unavailable
                } else {
                    DnsHealthState::Degraded
                }
            }
        }
    }

    /// Evaluates if a given error string indicates a DNS-related failure.
    pub fn is_dns_error(log_line: &str) -> bool {
        let lower = log_line.to_ascii_lowercase();
        lower.contains("eai_again")
            || lower.contains("name or service not known")
            || lower.contains("getaddrinfo failed")
            || lower.contains("nxdomain")
            || lower.contains("servfail")
            || lower.contains("dns resolution failed")
            || lower.contains("temporary failure in name resolution")
    }
}
