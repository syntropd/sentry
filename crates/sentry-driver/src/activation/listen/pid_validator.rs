//! Validates `$LISTEN_PID` against the current process identifier.

use crate::activation::error::ActivationError;
use std::env;
use std::process;

/// Validates whether `$LISTEN_PID` matches `std::process::id()`.
///
/// Under systemd specification, if `$LISTEN_PID` is not set or does not match
/// the current process ID, the socket descriptors do not belong to this process.
pub fn validate_listen_pid() -> Result<bool, ActivationError> {
    let pid_str = match env::var("LISTEN_PID") {
        Ok(val) if !val.is_empty() => val,
        _ => return Ok(false),
    };

    let listen_pid = pid_str.parse::<u32>().map_err(|e| {
        ActivationError::InvalidPid(pid_str.clone(), e)
    })?;

    Ok(listen_pid == process::id())
}
