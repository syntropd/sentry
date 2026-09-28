//! 1:1 Unit QA tests for socket_inspector.

use sentry_driver::activation::socket_inspector::{
    get_socket_bound_path, is_socket, is_unix_stream_listener, matches_bound_path,
};
use std::fs::File;
use std::net::{TcpListener, UdpSocket};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use tempfile::tempdir;

#[test]
fn test_is_socket_with_file_returns_false() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("regular_file.txt");
    let file = File::create(&file_path).unwrap();

    assert!(!is_socket(file.as_raw_fd()));
}

#[test]
fn test_is_socket_with_invalid_fd_returns_false() {
    assert!(!is_socket(-1));
    assert!(!is_socket(999999));
}

#[test]
fn test_is_socket_with_various_sockets_returns_true() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("test.sock");
    let unix_listener = UnixListener::bind(&sock_path).unwrap();
    assert!(is_socket(unix_listener.as_raw_fd()));

    let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    assert!(is_socket(tcp_listener.as_raw_fd()));

    let udp_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    assert!(is_socket(udp_socket.as_raw_fd()));

    let (s1, _s2) = UnixStream::pair().unwrap();
    assert!(is_socket(s1.as_raw_fd()));
}

#[test]
fn test_is_unix_stream_listener() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("stream.sock");
    let dgram_path = dir.path().join("dgram.sock");
    let file_path = dir.path().join("regular.txt");

    let file = File::create(&file_path).unwrap();
    assert!(!is_unix_stream_listener(file.as_raw_fd()));

    let unix_listener = UnixListener::bind(&sock_path).unwrap();
    assert!(is_unix_stream_listener(unix_listener.as_raw_fd()));

    let (stream1, _stream2) = UnixStream::pair().unwrap();
    // Connected stream pair is not in listening state
    assert!(!is_unix_stream_listener(stream1.as_raw_fd()));

    let unix_dgram = UnixDatagram::bind(&dgram_path).unwrap();
    // Datagram socket is not SOCK_STREAM
    assert!(!is_unix_stream_listener(unix_dgram.as_raw_fd()));

    let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    // TCP listener is AF_INET, not AF_UNIX
    assert!(!is_unix_stream_listener(tcp_listener.as_raw_fd()));

    let udp_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    assert!(!is_unix_stream_listener(udp_socket.as_raw_fd()));
}

#[test]
fn test_get_socket_bound_path_and_matches() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("bound_test.sock");
    let other_path = dir.path().join("other.sock");

    let listener = UnixListener::bind(&sock_path).unwrap();
    let fd = listener.as_raw_fd();

    let bound = get_socket_bound_path(fd);
    assert!(bound.is_some());
    assert_eq!(bound.as_ref().unwrap(), &sock_path);

    assert!(matches_bound_path(fd, &sock_path));
    assert!(!matches_bound_path(fd, &other_path));

    let (s1, _s2) = UnixStream::pair().unwrap();
    // Socketpair sockets are unnamed
    assert_eq!(get_socket_bound_path(s1.as_raw_fd()), None);
    assert!(!matches_bound_path(s1.as_raw_fd(), &sock_path));
}
