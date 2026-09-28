//! Socket activation error types.

use std::io;
use thiserror::Error;

/// Socket activation errors.
#[derive(Error, Debug)]
pub enum ActivationError {
    /// Standard I/O error while manipulating socket descriptors.
    #[error("I/O error configuring descriptor: {0}")]
    Io(#[from] io::Error),

    /// Failed to parse LISTEN_FDS count.
    #[error("Failed to parse LISTEN_FDS count '{0}': {1}")]
    InvalidFdCount(String, std::num::ParseIntError),

    /// Failed to parse LISTEN_PID.
    #[error("Failed to parse LISTEN_PID '{0}': {1}")]
    InvalidPid(String, std::num::ParseIntError),

    /// Failed to set fcntl flags on descriptor.
    #[error("Failed to set fcntl flags on descriptor {0}: {1}")]
    FcntlError(i32, io::Error),
}
