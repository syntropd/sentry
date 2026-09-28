//! 1:1 Unit QA tests for socket descriptor flag hardening.

use rustix::fs::{fcntl_getfd, fcntl_getfl, FdFlags, OFlags};
use sentry_driver::activation::harden_activated_fd;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixDatagram;

#[test]
fn test_harden_activated_fd_sets_cloexec_and_nonblock() {
    let socket = UnixDatagram::unbound().unwrap();
    let raw_fd = socket.as_raw_fd();

    harden_activated_fd(raw_fd).expect("Hardening must succeed on valid descriptor");

    let fd_flags = fcntl_getfd(unsafe { rustix::fd::BorrowedFd::borrow_raw(raw_fd) }).unwrap();
    assert!(fd_flags.contains(FdFlags::CLOEXEC));

    let fl_flags = fcntl_getfl(unsafe { rustix::fd::BorrowedFd::borrow_raw(raw_fd) }).unwrap();
    assert!(fl_flags.contains(OFlags::NONBLOCK));
}
