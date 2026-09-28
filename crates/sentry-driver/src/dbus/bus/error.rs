//! Pure Rust D-Bus driver error types.

use thiserror::Error;

/// D-Bus driver error variants.
#[derive(Error, Debug)]
pub enum DbusDriverError {
    /// Pure Rust zbus protocol or connection error.
    #[error("zbus communication error: {0}")]
    Zbus(#[from] zbus::Error),

    /// Hex sequence decoding failure in unit path unescape.
    #[error("Invalid D-Bus object path escape: '{0}' ({1})")]
    InvalidPathEscape(String, String),

    /// Invalid D-Bus object path syntax.
    #[error("Invalid D-Bus object path: '{0}' ({1})")]
    InvalidObjectPath(String, String),

    /// Failed to subscribe to systemd1.Manager signals.
    #[error("D-Bus subscription failed on systemd1.Manager: {0}")]
    SubscriptionFailed(String),

    /// Internal event channel closed.
    #[error("D-Bus event channel disconnected")]
    ChannelClosed,
}
