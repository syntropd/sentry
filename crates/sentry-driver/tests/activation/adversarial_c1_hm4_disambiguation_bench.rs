//! Adversarial empirical benchmark for Milestone HM4 socket disambiguation.
//!
//! Empirically validates:
//! 1. Disambiguation latency across 1,000 runs: < 50µs for 10 descriptors.
//! 2. Zero heap reallocation during socket disambiguation.
//! 3. Zero heap allocation during name and inode matching tiers.

use sentry_driver::activation::{disambiguate_socket, ActivatedSocket};
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::File;
use std::net::{TcpListener, UdpSocket};
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tempfile::tempdir;

struct AllocTracker;
static TOTAL_ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static TOTAL_ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static TOTAL_REALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for AllocTracker {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            TOTAL_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            TOTAL_ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        TOTAL_REALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: AllocTracker = AllocTracker;

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
fn test_c1_benchmark_disambiguation_latency_and_zero_realloc() {
    let dir = tempdir().unwrap();

    // 1. Construct 10 heterogeneous file descriptors
    let udp1 = UdpSocket::bind("127.0.0.1:0").unwrap();
    let udp2 = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (pipe_r, pipe_w) = create_pipe();
    let file1 = File::create(dir.path().join("dummy1.txt")).unwrap();
    let file2 = File::create(dir.path().join("dummy2.txt")).unwrap();
    let tcp1 = TcpListener::bind("127.0.0.1:0").unwrap();
    let unix_dgram = UnixDatagram::bind(dir.path().join("dgram.sock")).unwrap();
    let (stream_a, _stream_b) = UnixStream::pair().unwrap();
    let sentry_path = dir.path().join("sentry.sock");
    let unix_listener = UnixListener::bind(&sentry_path).unwrap();

    let raw_fds = [
        (udp1.as_raw_fd(), "udp_metrics"),
        (udp2.as_raw_fd(), "udp_dns"),
        (pipe_r.as_raw_fd(), "pipe_read"),
        (pipe_w.as_raw_fd(), "pipe_write"),
        (file1.as_raw_fd(), "file_log"),
        (file2.as_raw_fd(), "file_state"),
        (tcp1.as_raw_fd(), "tcp_http"),
        (unix_dgram.as_raw_fd(), "unix_dgram"),
        (stream_a.as_raw_fd(), "unix_stream_pair"),
        (unix_listener.as_raw_fd(), "sentry.socket"),
    ];

    assert_eq!(raw_fds.len(), 10, "Must benchmark exactly 10 descriptors");

    let iters = 1_000;
    let target_names = ["sentry", "systemd-sentry", "sentry.socket"];

    // Warm-up run
    let mut warmup_list = Vec::with_capacity(10);
    for (i, (fd, name)) in raw_fds.iter().enumerate() {
        warmup_list.push(ActivatedSocket::new(*fd, *name, i));
    }
    let warmup_sock = disambiguate_socket(&mut warmup_list, &target_names, None)
        .expect("Warm-up disambiguation must succeed");
    assert_eq!(warmup_sock.fd, unix_listener.as_raw_fd());

    let mut total_duration = std::time::Duration::ZERO;
    let mut measured_reallocs = 0usize;
    let mut measured_allocs = 0usize;

    for _ in 0..iters {
        // Pre-allocate vector with exact capacity to isolate disambiguate_socket
        let mut test_list = Vec::with_capacity(10);
        for (i, (fd, name)) in raw_fds.iter().enumerate() {
            test_list.push(ActivatedSocket::new(*fd, *name, i));
        }

        let realloc_before = TOTAL_REALLOC_COUNT.load(Ordering::SeqCst);
        let alloc_before = TOTAL_ALLOC_COUNT.load(Ordering::SeqCst);
        let start_time = Instant::now();

        // Target function call under empirical evaluation
        let result = disambiguate_socket(&mut test_list, &target_names, None);

        let elapsed = start_time.elapsed();
        let alloc_after = TOTAL_ALLOC_COUNT.load(Ordering::SeqCst);
        let realloc_after = TOTAL_REALLOC_COUNT.load(Ordering::SeqCst);

        total_duration += elapsed;
        measured_reallocs += realloc_after.saturating_sub(realloc_before);
        measured_allocs += alloc_after.saturating_sub(alloc_before);

        let chosen = result.expect("Must choose UNIX domain stream listener");
        assert_eq!(chosen.fd, unix_listener.as_raw_fd());
        assert_eq!(chosen.index, 9);
        assert_eq!(test_list.len(), 9);
    }

    let nanos_per_run = total_duration.as_nanos() as f64 / iters as f64;
    let micros_per_run = nanos_per_run / 1000.0;

    println!(
        "[EMPIRICAL BENCHMARK] disambiguate_socket across 1,000 runs (10 descriptors):\n\
         - Total Elapsed: {:?}\n\
         - Mean Latency:  {:.3}µs ({:.1}ns)\n\
         - Reallocations: {} (Target: 0)\n\
         - Allocations:   {} (Target: 0)",
        total_duration, micros_per_run, nanos_per_run, measured_reallocs, measured_allocs
    );

    // Hard requirements verification
    assert_eq!(
        measured_reallocs, 0,
        "disambiguate_socket must perform strictly zero heap reallocations across {} runs",
        iters
    );
    assert_eq!(
        measured_allocs, 0,
        "disambiguate_socket must perform strictly zero heap allocations during name matching across {} runs",
        iters
    );
    assert!(
        micros_per_run < 50.0,
        "Disambiguation latency ({:.3}µs) must be strictly < 50.0µs",
        micros_per_run
    );
}
