//! Acquires and manages systemd-logind inhibitor locks.
//!
//! Prevents immediate shutdown or sleep while critical crash triage flushes.

use sentry_core::error::DriverError;
use zbus::zvariant::OwnedFd;
use zbus::Connection;

/// RAII wrapper holding a systemd-inhibit lock file descriptor.
pub struct InhibitorLock {
    fd: OwnedFd,
    what: String,
    who: String,
    why: String,
}

impl InhibitorLock {
    /// Acquires a delay inhibitor lock from `org.freedesktop.login1`.
    pub async fn acquire(
        conn: &Connection,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> Result<Self, DriverError> {
        let reply = conn
            .call_method(
                Some("org.freedesktop.login1"),
                "/org/freedesktop/login1",
                Some("org.freedesktop.login1.Manager"),
                "Inhibit",
                &(what, who, why, mode),
            )
            .await
            .map_err(|e| DriverError::Io(std::io::Error::other(e.to_string())))?;

        let fd: OwnedFd = reply
            .body()
            .deserialize()
            .map_err(|e| DriverError::Io(std::io::Error::other(e.to_string())))?;

        Ok(Self {
            fd,
            what: what.to_string(),
            who: who.to_string(),
            why: why.to_string(),
        })
    }

    /// Creates a mock inhibitor lock wrapper with a given file descriptor (for unit testing).
    pub fn from_fd(fd: OwnedFd, what: &str, who: &str, why: &str) -> Self {
        Self {
            fd,
            what: what.to_string(),
            who: who.to_string(),
            why: why.to_string(),
        }
    }

    /// Returns a reference to the held file descriptor.
    pub fn fd(&self) -> &OwnedFd {
        &self.fd
    }

    /// What operations are inhibited (e.g. "shutdown:sleep").
    pub fn what(&self) -> &str {
        &self.what
    }

    /// Entity holding the lock.
    pub fn who(&self) -> &str {
        &self.who
    }

    /// Rationale for the lock.
    pub fn why(&self) -> &str {
        &self.why
    }
}
