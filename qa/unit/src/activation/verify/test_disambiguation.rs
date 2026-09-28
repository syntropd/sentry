//! 1:1 Unit QA tests for socket disambiguation.

use sentry_driver::activation::{disambiguate_socket, ActivatedSocket};
use std::fs::File;
use std::net::UdpSocket;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixDatagram, UnixListener};
use tempfile::tempdir;

#[test]
fn test_disambiguate_empty_returns_none() {
    let mut sockets = Vec::new();
    let res = disambiguate_socket(&mut sockets, &["sentry"], None);
    assert!(res.is_none());
}

#[test]
fn test_disambiguate_tier1_bound_path_match() {
    let dir = tempdir().unwrap();
    let path_a = dir.path().join("other.sock");
    let path_b = dir.path().join("sentry.sock");

    let listener_a = UnixListener::bind(&path_a).unwrap();
    let listener_b = UnixListener::bind(&path_b).unwrap();

    let mut sockets = vec![
        ActivatedSocket::new(listener_a.as_raw_fd(), "unknown:0", 0),
        ActivatedSocket::new(listener_b.as_raw_fd(), "unknown:1", 1),
    ];

    let chosen = disambiguate_socket(&mut sockets, &["sentry"], Some(&path_b))
        .expect("Should match Tier 1 by bound path");

    assert_eq!(chosen.fd, listener_b.as_raw_fd());
    assert_eq!(chosen.index, 1);
    assert_eq!(sockets.len(), 1);
    assert_eq!(sockets[0].fd, listener_a.as_raw_fd());
}

#[test]
fn test_disambiguate_tier2_name_match() {
    let dir = tempdir().unwrap();
    let path_metrics = dir.path().join("metrics.sock");
    let path_sentry = dir.path().join("sentry.sock");

    let listener_metrics = UnixListener::bind(&path_metrics).unwrap();
    let listener_sentry = UnixListener::bind(&path_sentry).unwrap();

    let mut sockets = vec![
        ActivatedSocket::new(listener_metrics.as_raw_fd(), "metrics", 0),
        ActivatedSocket::new(listener_sentry.as_raw_fd(), "sentry", 1),
    ];

    let chosen = disambiguate_socket(
        &mut sockets,
        &["sentry", "systemd-sentry", "sentry.socket"],
        None,
    )
    .expect("Should match Tier 2 by target name");

    assert_eq!(chosen.fd, listener_sentry.as_raw_fd());
    assert_eq!(chosen.name, "sentry");
    assert_eq!(sockets.len(), 1);
}

#[test]
fn test_disambiguate_tier2_rejects_non_stream_listener_with_target_name() {
    let dir = tempdir().unwrap();
    let dgram_path = dir.path().join("sentry_dgram.sock");
    let stream_path = dir.path().join("real_stream.sock");

    let dgram_sock = UnixDatagram::bind(&dgram_path).unwrap();
    let stream_listener = UnixListener::bind(&stream_path).unwrap();

    // The socket named "sentry" is a datagram socket (invalid for stream listener)
    // The socket named "other" is a stream listener (Tier 3 candidate)
    let mut sockets = vec![
        ActivatedSocket::new(dgram_sock.as_raw_fd(), "sentry", 0),
        ActivatedSocket::new(stream_listener.as_raw_fd(), "other", 1),
    ];

    let chosen = disambiguate_socket(
        &mut sockets,
        &["sentry", "systemd-sentry"],
        None,
    )
    .expect("Should fall through to Tier 3 for actual stream listener");

    // Must NOT pick the datagram socket at index 0 even though its name matched!
    assert_eq!(chosen.fd, stream_listener.as_raw_fd());
    assert_eq!(chosen.name, "other");
}

#[test]
fn test_disambiguate_tier3_unknown_names_inode_filtering() {
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let dir = tempdir().unwrap();
    let stream_path = dir.path().join("active.sock");
    let stream_listener = UnixListener::bind(&stream_path).unwrap();

    let mut sockets = vec![
        ActivatedSocket::new(udp.as_raw_fd(), "unknown:0", 0),
        ActivatedSocket::new(stream_listener.as_raw_fd(), "unknown:1", 1),
    ];

    let chosen = disambiguate_socket(
        &mut sockets,
        &["sentry", "systemd-sentry"],
        None,
    )
    .expect("Should find stream listener in Tier 3 via inode inspection");

    assert_eq!(chosen.fd, stream_listener.as_raw_fd());
    assert_eq!(chosen.index, 1);
}

#[test]
fn test_disambiguate_ignores_non_socket_descriptors() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("not_a_socket.log");
    let file = File::create(&file_path).unwrap();

    let stream_path = dir.path().join("listener.sock");
    let stream_listener = UnixListener::bind(&stream_path).unwrap();

    let mut sockets = vec![
        ActivatedSocket::new(file.as_raw_fd(), "sentry", 0),
        ActivatedSocket::new(stream_listener.as_raw_fd(), "unknown:1", 1),
    ];

    let chosen = disambiguate_socket(
        &mut sockets,
        &["sentry"],
        None,
    )
    .expect("Must ignore regular file and select stream listener");

    assert_eq!(chosen.fd, stream_listener.as_raw_fd());
}

#[test]
fn test_disambiguate_tier4_fallback_when_no_valid_stream_listener() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    let file = File::create(&file_path).unwrap();
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();

    let mut sockets = vec![
        ActivatedSocket::new(file.as_raw_fd(), "file", 0),
        ActivatedSocket::new(udp.as_raw_fd(), "udp", 1),
    ];

    let chosen = disambiguate_socket(
        &mut sockets,
        &["sentry"],
        None,
    );

    assert!(chosen.is_none(), "Must return None when no stream listener is present");
    assert_eq!(sockets.len(), 2, "Sockets list must remain untouched on failure");
}
