//! Pure-Rust socket inspection utilities using `rustix`.

use rustix::fd::BorrowedFd;
use rustix::fs::fstat;
use rustix::net::sockopt::{get_socket_acceptconn, get_socket_domain, get_socket_type};
use rustix::net::{getsockname, AddressFamily, SocketAddrAny, SocketType};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

/// Returns true if the file descriptor refers to a socket inode.
pub fn is_socket(fd: RawFd) -> bool {
    if fd < 0 {
        return false;
    }
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
    match fstat(borrowed) {
        Ok(stat) => (stat.st_mode as u32 & 0o170000) == 0o140000,
        Err(_) => false,
    }
}

/// Returns true if the file descriptor is a listening UNIX domain stream socket.
pub fn is_unix_stream_listener(fd: RawFd) -> bool {
    if !is_socket(fd) {
        return false;
    }
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };

    if get_socket_domain(borrowed) != Ok(AddressFamily::UNIX) {
        return false;
    }
    if get_socket_type(borrowed) != Ok(SocketType::STREAM) {
        return false;
    }
    if get_socket_acceptconn(borrowed) != Ok(true) {
        return false;
    }

    true
}

/// Returns the filesystem path bound to a UNIX domain socket, if any.
pub fn get_socket_bound_path(fd: RawFd) -> Option<PathBuf> {
    if !is_socket(fd) {
        return None;
    }
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
    let sock_addr = getsockname(borrowed).ok()?;

    match sock_addr {
        SocketAddrAny::Unix(addr) => {
            addr.path().map(|p| PathBuf::from(std::ffi::OsStr::from_bytes(p.to_bytes())))
        }
        _ => None,
    }
}

fn canonicalize_path(path: &Path) -> PathBuf {
    if let Ok(c) = path.canonicalize() {
        return c;
    }
    if let Some(parent) = path.parent() {
        if let Ok(p_canon) = parent.canonicalize() {
            if let Some(file_name) = path.file_name() {
                return p_canon.join(file_name);
            }
        }
    }
    path.to_path_buf()
}

/// Returns true if the socket's bound filesystem path matches the expected path.
pub fn matches_bound_path(fd: RawFd, expected: &Path) -> bool {
    let bound = match get_socket_bound_path(fd) {
        Some(b) => b,
        None => return false,
    };

    if bound == expected {
        return true;
    }

    let bound_canon = canonicalize_path(&bound);
    let expected_canon = canonicalize_path(expected);
    bound_canon == expected_canon
}

