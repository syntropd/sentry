//! Native systemd socket activation parser.

use crate::activation::error::ActivationError;
use crate::activation::fd_flags::harden_activated_fd;
use crate::activation::model::ActivatedSocket;
use crate::activation::name_parser::parse_listen_fdnames;
use crate::activation::pid_validator::validate_listen_pid;
use std::env;
use std::os::unix::io::RawFd;

/// Starting file descriptor offset for systemd socket activation (`SD_LISTEN_FDS_START = 3`).
pub const SD_LISTEN_FDS_START: RawFd = 3;

/// Maximum permissible file descriptors received via socket activation (4096).
pub const MAX_ACTIVATED_FDS: usize = 4096;

/// Parses passed systemd file descriptors from `$LISTEN_FDS` and `$LISTEN_FDNAMES`.
///
/// Validates `$LISTEN_PID`, hardens descriptors with `FD_CLOEXEC` and `O_NONBLOCK`,
/// maps configured names, and optionally removes the activation environment variables.
pub fn parse_listen_fds(unset_env: bool) -> Result<Vec<ActivatedSocket>, ActivationError> {
    if !validate_listen_pid()? {
        return Ok(Vec::new());
    }

    let fds_str = match env::var("LISTEN_FDS") {
        Ok(val) if !val.is_empty() => val,
        _ => return Ok(Vec::new()),
    };

    let count = fds_str.parse::<usize>().map_err(|e| {
        ActivationError::InvalidFdCount(fds_str.clone(), e)
    })?;

    if count > MAX_ACTIVATED_FDS {
        let overflow_err = "4097".parse::<u8>().unwrap_err();
        return Err(ActivationError::InvalidFdCount(fds_str, overflow_err));
    }

    if count == 0 {
        return Ok(Vec::new());
    }

    let names = parse_listen_fdnames(count);
    let mut sockets = Vec::with_capacity(count);

    for index in 0..count {
        let fd = SD_LISTEN_FDS_START + (index as RawFd);
        harden_activated_fd(fd)?;
        sockets.push(ActivatedSocket::new(fd, &names[index], index));
    }

    if unset_env {
        env::remove_var("LISTEN_PID");
        env::remove_var("LISTEN_FDS");
        env::remove_var("LISTEN_FDNAMES");
    }

    Ok(sockets)
}
