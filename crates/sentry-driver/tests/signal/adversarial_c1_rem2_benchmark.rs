//! Adversarial empirical benchmark and verification harness for Milestone HM2 Iteration 2 Remediation.
//!
//! Empirically validates:
//! 1. `parse_loadavg_to_psi` across 1,000 iterations: strictly 0 heap allocations and sub-microsecond latency.
//! 2. Live process statm extraction memory divergence bounded (< 32MB) across repeated reads.
//! 3. In simulated containers lacking `/sys/fs/cgroup`, `collect_cgroup_telemetry_resilient` returns
//!    telemetry with `synthetic == true` and `is_synthetic() == true` regardless of `/proc/<pid>/cgroup` contents.

use sentry_driver::cgroup::{
    collect_cgroup_telemetry_resilient_with_proc, extract_pid_metrics,
};
use sentry_driver::psi::parse_loadavg_to_psi;
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tempfile::tempdir;

struct AllocTracker;
static TOTAL_ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static TOTAL_ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

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
}

#[global_allocator]
static GLOBAL: AllocTracker = AllocTracker;

#[test]
fn test_empirical_container_fallback_benchmarks_and_invariants() {
    // -------------------------------------------------------------------------
    // Phase 1: Benchmark parse_loadavg_to_psi (1,000 iterations)
    // -------------------------------------------------------------------------
    let sample = "0.50 1.25 2.00 3/400 98765\n";

    // Warm-up call (initializes OnceLock<f64> cached CPU count)
    let warmup = parse_loadavg_to_psi(sample).expect("warm-up parse");
    assert!(warmup.some.avg10 >= 0.0);

    let iters = 1_000;
    let start_allocs = TOTAL_ALLOC_COUNT.load(Ordering::SeqCst);
    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::SeqCst);
    let start_time = Instant::now();

    for _ in 0..iters {
        let res = parse_loadavg_to_psi(sample).expect("iteration parse");
        std::hint::black_box(res);
    }

    let elapsed = start_time.elapsed();
    let end_allocs = TOTAL_ALLOC_COUNT.load(Ordering::SeqCst);
    let end_bytes = TOTAL_ALLOC_BYTES.load(Ordering::SeqCst);

    let alloc_delta = end_allocs.saturating_sub(start_allocs);
    let byte_delta = end_bytes.saturating_sub(start_bytes);
    let nanos_per_iter = elapsed.as_nanos() as f64 / iters as f64;
    let micros_per_iter = nanos_per_iter / 1000.0;

    println!(
        "[EMPIRICAL BENCHMARK] parse_loadavg_to_psi 1,000 iters: elapsed={:?}, per_iter={:.2}ns ({:.4}µs), allocs={}, bytes={}",
        elapsed, nanos_per_iter, micros_per_iter, alloc_delta, byte_delta
    );

    // Strictly 0 heap allocations across 1,000 iterations
    assert_eq!(
        alloc_delta, 0,
        "parse_loadavg_to_psi must perform 0 heap allocations across {} iters, got {}",
        iters, alloc_delta
    );
    assert_eq!(
        byte_delta, 0,
        "parse_loadavg_to_psi must allocate 0 bytes, got {}",
        byte_delta
    );

    // Latency assertion: sub-microsecond (<1.0µs) in optimized release, <50µs in debug build
    let max_micros = if cfg!(debug_assertions) { 50.0 } else { 1.0 };
    assert!(
        micros_per_iter < max_micros,
        "parse_loadavg_to_psi latency must be <{}µs, got {:.4}µs",
        max_micros, micros_per_iter
    );

    // -------------------------------------------------------------------------
    // Phase 2: Live process statm extraction memory divergence bounded (< 32MB)
    // -------------------------------------------------------------------------
    let my_pid = std::process::id();
    let proc_root = Path::new("/proc");

    let initial = extract_pid_metrics(proc_root, my_pid)
        .expect("initial live statm extraction");
    let initial_mem = initial.memory_current_bytes.expect("initial resident memory");
    assert!(initial_mem > 0, "Resident memory must be positive");

    let non_existent_cgroup = Path::new("/nonexistent_sys_fs_cgroup_dir");

    for i in 0..100 {
        let fallback = collect_cgroup_telemetry_resilient_with_proc(
            non_existent_cgroup,
            "sentry-test.service",
            Some(my_pid),
            proc_root,
        );

        let fallback_mem = fallback.memory_current_bytes.expect("fallback resident memory");
        let diff = fallback_mem.abs_diff(initial_mem);

        assert!(
            diff < 32 * 1024 * 1024,
            "Iteration {}: Memory divergence {} B exceeded 32MB bound (initial={}, fallback={})",
            i, diff, initial_mem, fallback_mem
        );

        assert!(fallback.synthetic, "Iteration {}: synthetic flag must be true", i);
        assert!(fallback.is_synthetic(), "Iteration {}: is_synthetic() must be true", i);
    }
    println!(
        "[EMPIRICAL INVARIANT] Live process statm extraction across 100 iterations bounded: initial={} B, synthetic=true",
        initial_mem
    );

    // -------------------------------------------------------------------------
    // Phase 3: Simulated container without /sys/fs/cgroup synthetic variants
    // -------------------------------------------------------------------------
    let cg_dir = tempdir().unwrap();
    let missing_cgroup = cg_dir.path().join("cgroup_not_mounted");
    let proc_dir = tempdir().unwrap();

    // Variant A: Standard systemd unified cgroup format in /proc/<pid>/cgroup
    {
        let p_dir = proc_dir.path().join("201");
        fs::create_dir_all(&p_dir).unwrap();
        fs::write(p_dir.join("statm"), "5000 1200 800 50 0 100 0\n").unwrap();
        fs::write(
            p_dir.join("stat"),
            "201 (daemon) S 1 201 201 0 -1 0 0 0 0 0 15 25 0 0 0 0 1 0 0 0 0\n",
        )
        .unwrap();
        fs::write(p_dir.join("cgroup"), "0::/user.slice/user-1000.slice/app.service\n").unwrap();

        let t = collect_cgroup_telemetry_resilient_with_proc(
            &missing_cgroup,
            "app.service",
            Some(201),
            proc_dir.path(),
        );

        assert!(t.synthetic, "Variant A: synthetic must be true");
        assert!(t.is_synthetic(), "Variant A: is_synthetic() must be true");
        assert!(t.cgroup_path.starts_with("/proc/201/cgroup:"), "Path: {}", t.cgroup_path);
        assert_eq!(t.memory_current_bytes, Some(1200 * rustix::param::page_size() as u64));
    }

    // Variant B: Docker container format in /proc/<pid>/cgroup
    {
        let p_dir = proc_dir.path().join("202");
        fs::create_dir_all(&p_dir).unwrap();
        fs::write(p_dir.join("statm"), "8000 2500 1000 60 0 200 0\n").unwrap();
        fs::write(
            p_dir.join("stat"),
            "202 (docker-app) S 1 202 202 0 -1 0 0 0 0 0 50 50 0 0 0 0 1 0 0 0 0\n",
        )
        .unwrap();
        fs::write(
            p_dir.join("cgroup"),
            "0::/docker/a1b2c3d4e5f67890abcdef1234567890\n",
        )
        .unwrap();

        let t = collect_cgroup_telemetry_resilient_with_proc(
            &missing_cgroup,
            "docker-app.service",
            Some(202),
            proc_dir.path(),
        );

        assert!(t.synthetic, "Variant B: synthetic must be true");
        assert!(t.is_synthetic(), "Variant B: is_synthetic() must be true");
        assert!(t.cgroup_path.starts_with("/proc/202/cgroup:"), "Path: {}", t.cgroup_path);
    }

    // Variant C: Absent /proc/<pid>/cgroup file
    {
        let p_dir = proc_dir.path().join("203");
        fs::create_dir_all(&p_dir).unwrap();
        fs::write(p_dir.join("statm"), "3000 600 300 20 0 50 0\n").unwrap();
        fs::write(
            p_dir.join("stat"),
            "203 (no-cg) S 1 203 203 0 -1 0 0 0 0 0 5 10 0 0 0 0 1 0 0 0 0\n",
        )
        .unwrap();

        let t = collect_cgroup_telemetry_resilient_with_proc(
            &missing_cgroup,
            "no-cg.service",
            Some(203),
            proc_dir.path(),
        );

        assert!(t.synthetic, "Variant C: synthetic must be true");
        assert!(t.is_synthetic(), "Variant C: is_synthetic() must be true");
        assert_eq!(t.cgroup_path, "/proc/203/synthetic");
    }

    // Variant D: Absent PID or zero PID
    {
        let t_none = collect_cgroup_telemetry_resilient_with_proc(
            &missing_cgroup,
            "pure-synthetic.service",
            None,
            proc_dir.path(),
        );
        assert!(t_none.synthetic, "Variant D (None): synthetic must be true");
        assert!(t_none.is_synthetic(), "Variant D (None): is_synthetic() must be true");

        let t_zero = collect_cgroup_telemetry_resilient_with_proc(
            &missing_cgroup,
            "pure-synthetic.service",
            Some(0),
            proc_dir.path(),
        );
        assert!(t_zero.synthetic, "Variant D (0): synthetic must be true");
        assert!(t_zero.is_synthetic(), "Variant D (0): is_synthetic() must be true");
    }

    println!("[EMPIRICAL INVARIANT] Simulated container variants A-D verified: synthetic=true and is_synthetic()=true");
}
