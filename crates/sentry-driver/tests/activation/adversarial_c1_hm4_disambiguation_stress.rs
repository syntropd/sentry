//! Adversarial stress test harness for Milestone HM4 multi-socket disambiguation.
//!
//! Empirically validates:
//! 1. Multi-socket scenario with raw FDs 3 (UDP), 4 (Pipe), and 5 (Unix stream listener).
//! 2. Safe rejection of non-socket descriptors (pipes and regular files).
//! 3. Transport rejection matrix (TCP listeners, UNIX datagrams, connected stream pairs).
//! 4. Safe handling of invalid, closed, and negative file descriptors.

use sentry_driver::activation::socket_inspector::{is_socket, is_unix_stream_listener};
use sentry_driver::activation::{disambiguate_socket, parse_listen_fds, ActivatedSocket};
use std::env;
use std::fs::File;
use std::net::{TcpListener, UdpSocket};
use std::os::unix::io::{AsRawFd, IntoRawFd};
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::process;
use std::sync::Mutex;
use tempfile::tempdir;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn create_raw_pipe() -> (i32, i32) {
    let mut fds = [0i32; 2];
    unsafe {
        extern "C" { fn pipe(pipefd: *mut i32) -> i32; }
        assert_eq!(pipe(fds.as_mut_ptr()), 0);
    }
    (fds[0], fds[1])
}

#[test]
fn test_c1_multi_socket_scenario_udp3_pipe4_unix5() {
    let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let original_fd4 = unsafe {
        extern "C" {
            fn fcntl(fd: i32, cmd: i32, ...) -> i32;
            fn dup(fd: i32) -> i32;
        }
        if fcntl(4, 1) != -1 {
            let saved = dup(4);
            if saved >= 0 { Some(saved) } else { None }
        } else {
            None
        }
    };
    let dir = tempdir().unwrap();

    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (pipe_r, pipe_w) = create_raw_pipe();
    let sock_path = dir.path().join("sentry.sock");
    let unix_listener = UnixListener::bind(&sock_path).unwrap();

    // Relinquish Rust std ownership via into_raw_fd to prevent IO safety aborts
    let raw_u = udp.into_raw_fd();
    let raw_s = unix_listener.into_raw_fd();

    unsafe {
        extern "C" {
            fn dup2(oldfd: i32, newfd: i32) -> i32;
            fn close(fd: i32) -> i32;
        }
        // Duplicate to high scratch FDs to safely decouple from target FDs 3, 4, 5
        let (s_u, s_r, s_s) = (dup2(raw_u, 100), dup2(pipe_r, 101), dup2(raw_s, 102));
        close(raw_u); close(pipe_r); close(pipe_w); close(raw_s);

        assert_eq!(dup2(s_u, 3), 3);
        assert_eq!(dup2(s_r, 4), 4);
        assert_eq!(dup2(s_s, 5), 5);
        close(s_u); close(s_r); close(s_s);
    }

    // Verify individual raw FD properties
    assert!(is_socket(3) && !is_unix_stream_listener(3), "FD 3 (UDP) must NOT be stream listener");
    assert!(!is_socket(4) && !is_unix_stream_listener(4), "FD 4 (Pipe) must NOT be stream listener");
    assert!(is_socket(5) && is_unix_stream_listener(5), "FD 5 (UNIX listener) MUST be stream listener");

    // Scenario A: Standard target name match
    {
        let mut sockets = vec![
            ActivatedSocket::new(3, "udp_metrics", 0),
            ActivatedSocket::new(4, "log_pipe", 1),
            ActivatedSocket::new(5, "sentry.socket", 2),
        ];

        let chosen = disambiguate_socket(
            &mut sockets,
            &["sentry", "systemd-sentry", "sentry.socket"],
            None,
        )
        .expect("Must choose UNIX stream listener at FD 5");

        assert_eq!(chosen.fd, 5);
        assert_eq!(chosen.index, 2);
        assert_eq!(sockets.len(), 2);
        assert_eq!(sockets[0].fd, 3);
        assert_eq!(sockets[1].fd, 4);
    }

    // Scenario B: Hostile naming (FD 3 & 4 named 'sentry', FD 5 named 'other_service')
    {
        let mut sockets = vec![
            ActivatedSocket::new(3, "sentry", 0),
            ActivatedSocket::new(4, "sentry.socket", 1),
            ActivatedSocket::new(5, "other_service", 2),
        ];

        let chosen = disambiguate_socket(&mut sockets, &["sentry", "systemd-sentry"], None)
            .expect("Must fall back to Tier 3 for FD 5 despite hostile names on FDs 3 and 4");

        assert_eq!(chosen.fd, 5, "Must never select FD 3 (UDP) or FD 4 (Pipe)");
        assert_eq!(chosen.index, 2);
    }

    // Scenario C: End-to-end integration with parse_listen_fds
    {
        env::set_var("LISTEN_PID", process::id().to_string());
        env::set_var("LISTEN_FDS", "3");
        env::set_var("LISTEN_FDNAMES", "udp_dns:app_pipe:sentry");

        let mut parsed = parse_listen_fds(true).expect("parse_listen_fds must succeed");
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].fd, 3);
        assert_eq!(parsed[1].fd, 4);
        assert_eq!(parsed[2].fd, 5);

        let chosen = disambiguate_socket(&mut parsed, &["sentry", "systemd-sentry"], Some(&sock_path))
            .expect("End-to-end disambiguation must select FD 5");

        assert_eq!(chosen.fd, 5);
        let std_listener = chosen.into_std_unix_listener();
        assert!(std_listener.local_addr().is_ok(), "Must convert into active Std UnixListener");
    }

    unsafe {
        extern "C" {
            fn close(fd: i32) -> i32;
            fn dup2(oldfd: i32, newfd: i32) -> i32;
        }
        close(3); close(4);
        if let Some(saved) = original_fd4 {
            dup2(saved, 4);
            close(saved);
        }
    }
}

#[test]
fn test_c1_non_socket_descriptor_rejection() {
    let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let (pipe_r, pipe_w) = create_raw_pipe();
    let file1 = File::create(dir.path().join("file1.log")).unwrap();
    let file2 = File::open("/dev/null").unwrap();

    let non_sockets = [pipe_r, pipe_w, file1.as_raw_fd(), file2.as_raw_fd()];
    for &fd in &non_sockets {
        assert!(!is_socket(fd), "FD {} must not be identified as socket", fd);
        assert!(!is_unix_stream_listener(fd), "FD {} must not be stream listener", fd);
    }

    let mut sockets = vec![
        ActivatedSocket::new(pipe_r, "sentry", 0),
        ActivatedSocket::new(pipe_w, "systemd-sentry", 1),
        ActivatedSocket::new(file1.as_raw_fd(), "sentry.socket", 2),
        ActivatedSocket::new(file2.as_raw_fd(), "sentry", 3),
    ];

    let result = disambiguate_socket(
        &mut sockets,
        &["sentry", "systemd-sentry", "sentry.socket"],
        Some(&dir.path().join("sentry.sock")),
    );

    assert!(result.is_none(), "Non-socket descriptors must be safely rejected");
    assert_eq!(sockets.len(), 4, "Vector must remain unmodified upon rejection");

    unsafe {
        extern "C" { fn close(fd: i32) -> i32; }
        close(pipe_r); close(pipe_w);
    }
}

#[test]
fn test_c1_adversarial_socket_type_and_transport_matrix() {
    let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();

    let tcp_v4 = TcpListener::bind("127.0.0.1:0").unwrap();
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let dgram = UnixDatagram::bind(dir.path().join("dgram.sock")).unwrap();
    let (pair_a, _pair_b) = UnixStream::pair().unwrap();

    // Verify raw socket types
    assert!(is_socket(tcp_v4.as_raw_fd()) && !is_unix_stream_listener(tcp_v4.as_raw_fd()));
    assert!(is_socket(udp.as_raw_fd()) && !is_unix_stream_listener(udp.as_raw_fd()));
    assert!(is_socket(dgram.as_raw_fd()) && !is_unix_stream_listener(dgram.as_raw_fd()));
    assert!(is_socket(pair_a.as_raw_fd()) && !is_unix_stream_listener(pair_a.as_raw_fd()));

    // Closed, extreme, and negative file descriptors
    assert!(!is_socket(-1) && !is_socket(-100) && !is_socket(65534));
    assert!(!is_unix_stream_listener(-1) && !is_unix_stream_listener(65534));

    // Reject all invalid socket types even if named 'sentry'
    let mut sockets = vec![
        ActivatedSocket::new(tcp_v4.as_raw_fd(), "sentry", 0),
        ActivatedSocket::new(udp.as_raw_fd(), "sentry", 1),
        ActivatedSocket::new(dgram.as_raw_fd(), "sentry", 2),
        ActivatedSocket::new(pair_a.as_raw_fd(), "sentry", 3),
        ActivatedSocket::new(-1, "sentry", 4),
        ActivatedSocket::new(65534, "sentry", 5),
    ];

    let chosen = disambiguate_socket(&mut sockets, &["sentry"], None);
    assert!(chosen.is_none(), "Invalid socket types must return None");
    assert_eq!(sockets.len(), 6);
}
