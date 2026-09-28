//! Challenger 1 Empirical Test Harness for Milestone HM2 (Requirement R2).
//!
//! Validates:
//! 1. Missing /proc/pressure empty/non-existent fallback to synthetic telemetry.
//! 2. Missing /sys/fs/cgroup or missing system.slice fallback to synthetic telemetry.
//! 3. Valid PID extraction for /proc/<pid>/statm and /proc/<pid>/stat.
//! 4. Invalid / exited PID clean fallback without panicking.
//! 5. Empirical check on CgroupTelemetry::is_synthetic() contract under procfs fallback.

use sentry_core::models::CpuStat;
use sentry_driver::cgroup::{
    collect_cgroup_telemetry_resilient, collect_cgroup_telemetry_resilient_with_proc,
    extract_pid_metrics,
};
use sentry_driver::psi::{collect_cgroup_psi_resilient, collect_system_psi_resilient};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_c1_psi_resilient_missing_proc_pressure_empty_dir() {
    let empty = tempdir().unwrap();
    let sys_psi = collect_system_psi_resilient(empty.path());
    assert!(sys_psi.is_synthetic(), "Must be synthetic on empty dir");
    assert_eq!(sys_psi.unit, None);

    let cg_psi = collect_cgroup_psi_resilient(empty.path(), Some("svc.service".into()));
    assert!(cg_psi.is_synthetic(), "Must be synthetic cgroup PSI");
    assert_eq!(cg_psi.unit.as_deref(), Some("svc.service"));
}

#[test]
fn test_c1_psi_resilient_missing_proc_pressure_nonexistent_dir() {
    let temp = tempdir().unwrap();
    let non_existent = temp.path().join("does_not_exist_pressure");

    let sys_psi = collect_system_psi_resilient(&non_existent);
    assert!(sys_psi.is_synthetic(), "Must be synthetic on non-existent path");

    let cg_psi = collect_cgroup_psi_resilient(&non_existent, Some("app.service".into()));
    assert!(cg_psi.is_synthetic(), "Must be synthetic on non-existent path");
    assert_eq!(cg_psi.unit.as_deref(), Some("app.service"));
}

#[test]
fn test_c1_cgroup_resilient_missing_sys_fs_cgroup_nonexistent() {
    let temp = tempdir().unwrap();
    let non_existent = temp.path().join("does_not_exist_cgroup");

    let cg = collect_cgroup_telemetry_resilient(&non_existent, "nginx.service", None);
    assert!(cg.is_synthetic(), "Must report synthetic fallback");
    assert_eq!(cg.unit.as_deref(), Some("nginx.service"));
    assert!(cg.memory_current_bytes.is_none());
    assert_eq!(cg.cpu_stat, CpuStat::default());
}

#[test]
fn test_c1_cgroup_resilient_missing_system_slice() {
    let empty_cgroup = tempdir().unwrap();
    let cg = collect_cgroup_telemetry_resilient(empty_cgroup.path(), "redis.service", None);
    assert!(cg.is_synthetic(), "Must report synthetic fallback");
    assert_eq!(cg.unit.as_deref(), Some("redis.service"));
    assert!(cg.memory_current_bytes.is_none());
}

#[test]
fn test_c1_statm_and_cpu_extraction_live_process() {
    let my_pid = std::process::id();
    let proc_root = Path::new("/proc");

    let metrics = extract_pid_metrics(proc_root, my_pid)
        .expect("extract_pid_metrics must succeed on current process");

    let mem = metrics.memory_current_bytes.expect("Must have resident memory");
    assert!(mem > 0, "Resident memory must be positive, got {}", mem);

    let temp_cg = tempdir().unwrap();
    let fallback = collect_cgroup_telemetry_resilient_with_proc(
        temp_cg.path(),
        "my_test.service",
        Some(my_pid),
        proc_root,
    );

    assert_eq!(fallback.unit.as_deref(), Some("my_test.service"));
    let fallback_mem = fallback.memory_current_bytes.expect("Must have resident memory");
    assert!(fallback_mem > 0, "Fallback memory must be positive");
    assert!(
        fallback_mem.abs_diff(mem) < 32 * 1024 * 1024,
        "Fallback memory ({}) diverged excessively from initial read ({})",
        fallback_mem,
        mem
    );
}

#[test]
fn test_c1_invalid_and_exited_pid_clean_synthetic_fallback() {
    let temp_cg = tempdir().unwrap();
    let temp_proc = tempdir().unwrap();

    // 1. PID = 0
    let cg_zero = collect_cgroup_telemetry_resilient_with_proc(
        temp_cg.path(),
        "zero.service",
        Some(0),
        temp_proc.path(),
    );
    assert!(cg_zero.is_synthetic(), "PID 0 must yield synthetic");
    assert!(cg_zero.memory_current_bytes.is_none());

    // 2. Extremely large non-existent PID
    let cg_huge = collect_cgroup_telemetry_resilient_with_proc(
        temp_cg.path(),
        "huge.service",
        Some(u32::MAX),
        temp_proc.path(),
    );
    assert!(cg_huge.is_synthetic(), "Huge invalid PID must yield synthetic");
    assert!(cg_huge.memory_current_bytes.is_none());

    // 3. Exited child process PID
    let mut child = std::process::Command::new("true")
        .spawn()
        .expect("Failed to spawn dummy child");
    let child_pid = child.id();
    let _ = child.wait();

    let cg_dead = collect_cgroup_telemetry_resilient_with_proc(
        temp_cg.path(),
        "dead.service",
        Some(child_pid),
        Path::new("/proc"),
    );
    assert!(cg_dead.memory_current_bytes.is_none() || cg_dead.is_synthetic());
}

#[test]
fn test_c1_cgroup_procfs_fallback_is_synthetic_contract() {
    let cg_dir = tempdir().unwrap();
    let proc_dir = tempdir().unwrap();

    let pid_dir = proc_dir.path().join("7777");
    fs::create_dir_all(&pid_dir).unwrap();
    fs::write(pid_dir.join("statm"), "1000 250 100 10 0 5 0\n").unwrap();
    fs::write(
        pid_dir.join("stat"),
        "7777 (test) S 1 7777 7777 0 -1 0 0 0 0 0 10 20 0 0 0 0 1 0 0 0 0\n",
    )
    .unwrap();
    // Simulate /proc/<pid>/cgroup containing typical systemd cgroup path
    fs::write(pid_dir.join("cgroup"), "0::/user.slice/user-1000.slice/app.service\n").unwrap();

    let telemetry = collect_cgroup_telemetry_resilient_with_proc(
        cg_dir.path(),
        "app.service",
        Some(7777),
        proc_dir.path(),
    );

    let page_size = rustix::param::page_size() as u64;
    assert_eq!(telemetry.memory_current_bytes, Some(250 * page_size));
    assert_eq!(telemetry.cpu_stat.usage_usec, 30 * 10_000);

    // Documented contract:
    // "Returns true if this cgroup telemetry snapshot is synthetic or procfs fallback."
    let is_synthetic = telemetry.is_synthetic();
    assert!(
        is_synthetic,
        "CgroupTelemetry::is_synthetic() must evaluate to true on procfs fallback"
    );
    assert!(telemetry.synthetic, "telemetry.synthetic must be true");
    assert!(
        telemetry.cgroup_path.starts_with("/proc/7777/cgroup:"),
        "cgroup_path must start with /proc/7777/cgroup:, got '{}'",
        telemetry.cgroup_path
    );
}
