//! Adversarial stress suite for socket activation disambiguation (HM4 / R4).
//!
//! Evaluates behavior under corrupt/closed descriptors, 10+ heterogeneous descriptors,
//! reordered descriptors, unparseable/missing $LISTEN_FDNAMES, descriptor leak freedom,
//! and infinite loop prevention.

use sentry_driver::activation::{disambiguate_socket, ActivatedSocket};
use std::fs::File;
use std::net::{TcpListener, UdpSocket};
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;
use tempfile::tempdir;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn count_proc_fds() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(|entries| entries.flatten().count())
        .unwrap_or(0)
}

fn create_pipe() -> (OwnedFd, OwnedFd) {
    let mut fds = [0i32; 2];
    let res = unsafe {
        extern "C" {
            fn pipe(pipefd: *mut i32) -> i32;
        }
        pipe(fds.as_mut_ptr())
    };
    assert_eq!(res, 0);
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

#[test]
fn test_disambiguate_corrupt_and_closed_fds() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let stream_path = dir.path().join("valid_stream.sock");
    let stream_listener = UnixListener::bind(&stream_path).unwrap();

    let mut sockets = vec![
        ActivatedSocket::new(-1, "unknown:0", 0),
        ActivatedSocket::new(-999, "unknown:1", 1),
        ActivatedSocket::new(i32::MIN, "unknown:2", 2),
        ActivatedSocket::new(99999, "sentry", 3),
        ActivatedSocket::new(88888, "systemd-sentry", 4),
        ActivatedSocket::new(stream_listener.as_raw_fd(), "unknown:5", 5),
        ActivatedSocket::new(77777, "sentry.socket", 6),
    ];

    let chosen = disambiguate_socket(&mut sockets, &["sentry"], None)
        .expect("Must safely skip corrupt/closed FDs and select valid stream listener");

    assert_eq!(chosen.fd, stream_listener.as_raw_fd());
    assert_eq!(chosen.index, 5);

    let mut bad_sockets = vec![
        ActivatedSocket::new(-1, "sentry", 0),
        ActivatedSocket::new(65535, "sentry", 1),
    ];
    assert!(disambiguate_socket(&mut bad_sockets, &["sentry"], None).is_none());
}

#[test]
fn test_disambiguate_ten_plus_heterogeneous_descriptors() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();

    let (pipe_r, _pipe_w) = create_pipe();
    let udp1 = UdpSocket::bind("127.0.0.1:0").unwrap();
    let udp2 = UdpSocket::bind("127.0.0.1:0").unwrap();
    let tcp1 = TcpListener::bind("127.0.0.1:0").unwrap();
    let tcp2 = TcpListener::bind("127.0.0.1:0").unwrap();
    let dgram = UnixDatagram::bind(dir.path().join("dgram.sock")).unwrap();
    let (pair_a, _pair_b) = UnixStream::pair().unwrap();
    let file = File::open("/dev/null").unwrap();
    let real_stream_other = UnixListener::bind(dir.path().join("other.sock")).unwrap();
    let target_path = dir.path().join("sentry_target.sock");
    let real_stream_target = UnixListener::bind(&target_path).unwrap();

    let mut sockets = vec![
        ActivatedSocket::new(pipe_r.as_raw_fd(), "sentry", 0),
        ActivatedSocket::new(udp1.as_raw_fd(), "sentry", 1),
        ActivatedSocket::new(udp2.as_raw_fd(), "systemd-sentry", 2),
        ActivatedSocket::new(tcp1.as_raw_fd(), "sentry.socket", 3),
        ActivatedSocket::new(tcp2.as_raw_fd(), "other", 4),
        ActivatedSocket::new(dgram.as_raw_fd(), "sentry", 5),
        ActivatedSocket::new(pair_a.as_raw_fd(), "sentry", 6),
        ActivatedSocket::new(file.as_raw_fd(), "sentry", 7),
        ActivatedSocket::new(real_stream_other.as_raw_fd(), "unmatched", 8),
        ActivatedSocket::new(real_stream_target.as_raw_fd(), "sentry", 9),
    ];
    assert!(sockets.len() >= 10);

    let chosen = disambiguate_socket(
        &mut sockets,
        &["sentry", "systemd-sentry"],
        Some(&target_path),
    )
    .expect("Tier 1 must disambiguate exact bound path among 10+ heterogeneous FDs");

    assert_eq!(chosen.fd, real_stream_target.as_raw_fd());
    assert_eq!(chosen.index, 9);
}

#[test]
fn test_disambiguate_descriptor_reordering_stream_at_fd_10() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let mut pipes = Vec::new();
    let mut sockets = Vec::new();

    for i in 0..7 {
        let (r, w) = create_pipe();
        sockets.push(ActivatedSocket::new(r.as_raw_fd(), format!("pipe:{i}"), i));
        pipes.push((r, w));
    }

    let stream_path = dir.path().join("high_fd.sock");
    let listener = UnixListener::bind(&stream_path).unwrap();
    sockets.push(ActivatedSocket::new(listener.as_raw_fd(), "sentry", 7));

    assert_eq!(sockets.len(), 8);
    assert_eq!(sockets[7].fd, listener.as_raw_fd());

    let chosen = disambiguate_socket(&mut sockets, &["sentry"], None)
        .expect("Must select UNIX stream listener at high index when lower FDs are pipes");

    assert_eq!(chosen.fd, listener.as_raw_fd());
    assert_eq!(chosen.name, "sentry");
    assert_eq!(sockets.len(), 7, "Remaining pipes must stay in vector");
}

#[test]
fn test_disambiguate_missing_and_unparseable_listen_fdnames() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (stream_pair, _other) = UnixStream::pair().unwrap();
    let target_sock = dir.path().join("unnamed.sock");
    let listener = UnixListener::bind(&target_sock).unwrap();

    let mut sockets = vec![
        ActivatedSocket::new(udp.as_raw_fd(), "unknown:0", 0),
        ActivatedSocket::new(stream_pair.as_raw_fd(), "unknown:1", 1),
        ActivatedSocket::new(listener.as_raw_fd(), "unknown:2", 2),
    ];

    let chosen = disambiguate_socket(&mut sockets, &["sentry", "systemd-sentry"], None)
        .expect("Must find stream listener via Tier 3 when names are unknown:N");

    assert_eq!(chosen.fd, listener.as_raw_fd());
    assert_eq!(chosen.index, 2);
}

#[test]
fn test_assert_never_panics_leaks_or_infinite_loops() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let (pipe_r, _pipe_w) = create_pipe();
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let listener = UnixListener::bind(dir.path().join("leak_check.sock")).unwrap();

    let start_fd_count = count_proc_fds();
    let start_time = Instant::now();

    for _ in 0..1000 {
        let mut sockets = vec![
            ActivatedSocket::new(-1, "corrupt", 0),
            ActivatedSocket::new(pipe_r.as_raw_fd(), "pipe", 1),
            ActivatedSocket::new(udp.as_raw_fd(), "udp", 2),
            ActivatedSocket::new(listener.as_raw_fd(), "unknown:3", 3),
            ActivatedSocket::new(88888, "closed", 4),
        ];

        let res = disambiguate_socket(
            &mut sockets,
            &["sentry", "systemd-sentry"],
            Some(Path::new("/nonexistent/path.sock")),
        );

        assert!(res.is_some());
        assert_eq!(res.unwrap().fd, listener.as_raw_fd());
    }

    let elapsed = start_time.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "1000 iterations must complete rapidly (no infinite loop), took: {elapsed:?}"
    );

    let end_fd_count = count_proc_fds();
    let growth = end_fd_count.saturating_sub(start_fd_count);
    assert_eq!(
        growth, 0,
        "Disambiguation must not leak descriptors (start: {start_fd_count}, end: {end_fd_count})"
    );
}
