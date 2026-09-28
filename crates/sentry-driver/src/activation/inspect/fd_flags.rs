//! Enforces `FD_CLOEXEC` and `O_NONBLOCK` flags on activated file descriptors.

use crate::activation::error::ActivationError;
use rustix::fs::{fcntl_getfd, fcntl_getfl, fcntl_setfd, fcntl_setfl, FdFlags, OFlags};
use std::os::unix::io::RawFd;

/// Hardens an activated file descriptor by setting `FD_CLOEXEC` and `O_NONBLOCK`.
pub fn harden_activated_fd(fd: RawFd) -> Result<(), ActivationError> {
    // 1. Enforce FD_CLOEXEC to prevent leaking descriptor across exec
    let current_fd_flags = fcntl_getfd(unsafe { rustix::fd::BorrowedFd::borrow_raw(fd) })
        .map_err(|e| ActivationError::FcntlError(fd, e.into()))?;
    let target_fd_flags = current_fd_flags | FdFlags::CLOEXEC;
    fcntl_setfd(unsafe { rustix::fd::BorrowedFd::borrow_raw(fd) }, target_fd_flags)
        .map_err(|e| ActivationError::FcntlError(fd, e.into()))?;

    // 2. Enforce O_NONBLOCK for asynchronous Tokio runtime readiness
    let current_fl_flags = fcntl_getfl(unsafe { rustix::fd::BorrowedFd::borrow_raw(fd) })
        .map_err(|e| ActivationError::FcntlError(fd, e.into()))?;
    let target_fl_flags = current_fl_flags | OFlags::NONBLOCK;
    fcntl_setfl(unsafe { rustix::fd::BorrowedFd::borrow_raw(fd) }, target_fl_flags)
        .map_err(|e| ActivationError::FcntlError(fd, e.into()))?;

    Ok(())
}
