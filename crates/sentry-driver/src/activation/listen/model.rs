//! Data structures representing activated file descriptors.

use std::io;
use std::net::TcpListener as StdTcpListener;
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::net::UnixListener as StdUnixListener;
use std::path::PathBuf;
use tokio::net::{TcpListener as TokioTcpListener, UnixListener as TokioUnixListener};

/// File descriptor passed to the process via systemd socket activation.
#[derive(Debug)]
pub struct ActivatedSocket {
    /// Raw Linux file descriptor (e.g. 3, 4, ...).
    pub fd: RawFd,
    /// Name assigned to the socket from `$LISTEN_FDNAMES`.
    pub name: String,
    /// 0-based activation index (index 0 corresponds to fd 3).
    pub index: usize,
}

impl ActivatedSocket {
    /// Constructs a new `ActivatedSocket`.
    pub fn new(fd: RawFd, name: impl Into<String>, index: usize) -> Self {
        Self {
            fd,
            name: name.into(),
            index,
        }
    }

    /// Converts the activated socket into a standard library TCP listener.
    ///
    /// # Safety
    /// Takes ownership of the file descriptor `self.fd`.
    pub fn into_std_tcp_listener(self) -> StdTcpListener {
        unsafe { StdTcpListener::from_raw_fd(self.fd) }
    }

    /// Converts the activated socket into an asynchronous Tokio TCP listener.
    pub fn into_tokio_tcp_listener(self) -> io::Result<TokioTcpListener> {
        let std_listener = self.into_std_tcp_listener();
        std_listener.set_nonblocking(true)?;
        TokioTcpListener::from_std(std_listener)
    }

    /// Converts the activated socket into a standard library Unix domain listener.
    ///
    /// # Safety
    /// Takes ownership of the file descriptor `self.fd`.
    pub fn into_std_unix_listener(self) -> StdUnixListener {
        unsafe { StdUnixListener::from_raw_fd(self.fd) }
    }

    /// Converts the activated socket into an asynchronous Tokio Unix listener.
    pub fn into_tokio_unix_listener(self) -> io::Result<TokioUnixListener> {
        let std_listener = self.into_std_unix_listener();
        std_listener.set_nonblocking(true)?;
        TokioUnixListener::from_std(std_listener)
    }

    /// Returns true if this activated descriptor is a listening UNIX domain stream socket.
    pub fn is_unix_stream_listener(&self) -> bool {
        crate::activation::socket_inspector::is_unix_stream_listener(self.fd)
    }

    /// Returns the filesystem path bound to this socket, if any.
    pub fn bound_path(&self) -> Option<PathBuf> {
        crate::activation::socket_inspector::get_socket_bound_path(self.fd)
    }
}
