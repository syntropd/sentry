//! Adversarial stress test harness for unprivileged container & virtualized host fallback resilience (Milestone HM2).
//!
//! Evaluates edge cases: completely unmounted /proc, truncated/malformed loadavg & statm,
//! process exit before statm read, EACCES (0o000) sysfs permissions, and flat container layouts.

use sentry_core::models::PsiRecord;
use sentry_driver::cgroup::{
    collect_cgroup_telemetry, collect_cgroup_telemetry_resilient_with_proc,
    extract_pid_metrics, locate_unit_cgroup, read_cgroup_cpu, read_cgroup_io,
    read_cgroup_memory,
};
use sentry_driver::psi::{
    collect_cgroup_psi_resilient, collect_system_psi_resilient, parse_loadavg_to_psi,
    read_proc_loadavg,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
fn test_adversarial_completely_unmounted_proc_or_empty_loadavg() {
    let dir = tempdir().unwrap();
    let empty_proc = dir.path().join("unmounted_proc");
    fs::create_dir_all(&empty_proc).unwrap();

    // 1. Completely unmounted /proc/pressure
    let syn_psi = collect_system_psi_resilient(&empty_proc.join("pressure"));
    assert!(syn_psi.is_synthetic());
    assert_eq!(syn_psi.io, PsiRecord::zero());

    // 2. Empty 0-byte /proc/loadavg
    let empty_loadavg = dir.path().join("loadavg_zero");
    fs::write(&empty_loadavg, b"").unwrap();
    assert!(read_proc_loadavg(&empty_loadavg).is_err());
    assert!(parse_loadavg_to_psi("").is_err());

    // 3. 0-byte loadavg and meminfo in parent of pressure directory
    let proc_root = dir.path().join("proc_zero");
    let pressure_dir = proc_root.join("pressure");
    fs::create_dir_all(&pressure_dir).unwrap();
    fs::write(proc_root.join("loadavg"), b"").unwrap();
    fs::write(proc_root.join("meminfo"), b"").unwrap();

    let telemetry = collect_system_psi_resilient(&pressure_dir);
    assert!(telemetry.is_synthetic());
}

#[test]
fn test_adversarial_truncated_and_malformed_loadavg() {
    // Single field
    assert!(parse_loadavg_to_psi("1.50").is_err());
    // Two fields
    assert!(parse_loadavg_to_psi("1.50 2.00").is_err());
    // Non-float fields
    assert!(parse_loadavg_to_psi("corrupted 0.00 0.00").is_err());
    assert!(parse_loadavg_to_psi("0.00 corrupted 0.00").is_err());
    assert!(parse_loadavg_to_psi("0.00 0.00 corrupted").is_err());
    // Negative load numbers
    assert!(parse_loadavg_to_psi("-0.50 0.00 0.00").is_err());
    // Special IEEE-754 float values
    let nan_record = parse_loadavg_to_psi("NaN 0.00 0.00").unwrap();
    assert_eq!(nan_record.some.avg10, 0.0);
    let inf_record = parse_loadavg_to_psi("inf 0.00 0.00").unwrap();
    assert_eq!(inf_record.some.avg10, 100.0);
}

#[test]
fn test_adversarial_truncated_and_malformed_statm_and_stat() {
    let dir = tempdir().unwrap();
    let pid_dir = dir.path().join("7777");
    fs::create_dir_all(&pid_dir).unwrap();

    // 1. Truncated statm: 0 bytes
    fs::write(pid_dir.join("statm"), b"").unwrap();
    fs::write(pid_dir.join("stat"), b"").unwrap();
    assert!(extract_pid_metrics(dir.path(), 7777).is_err());

    // 2. statm with only 1 field (size without resident)
    fs::write(pid_dir.join("statm"), b"9999\n").unwrap();
    assert!(extract_pid_metrics(dir.path(), 7777).is_err());

    // 3. statm with non-numeric resident field
    fs::write(pid_dir.join("statm"), b"9999 not_a_number 100\n").unwrap();
    assert!(extract_pid_metrics(dir.path(), 7777).is_err());

    // 4. statm with valid resident field, but malformed stat
    fs::write(pid_dir.join("statm"), b"9999 500 100 0 0 0 0\n").unwrap();
    // stat has no comm parentheses
    fs::write(pid_dir.join("stat"), b"7777 malformed stat line").unwrap();
    let metrics = extract_pid_metrics(dir.path(), 7777).expect("statm alone should provide memory");
    assert!(metrics.memory_current_bytes.is_some());
    assert_eq!(metrics.cpu_stat.usage_usec, 0);

    // 5. stat with fewer than 13 tokens after comm
    fs::write(pid_dir.join("stat"), b"7777 (app) S 1 2 3 4 5").unwrap();
    let metrics2 = extract_pid_metrics(dir.path(), 7777).unwrap();
    assert_eq!(metrics2.cpu_stat.usage_usec, 0);

    // 6. stat with non-numeric utime/stime
    fs::write(pid_dir.join("stat"), b"7777 (app) S 1 2 3 4 5 6 7 8 9 10 bad bad").unwrap();
    let metrics3 = extract_pid_metrics(dir.path(), 7777).unwrap();
    assert_eq!(metrics3.cpu_stat.usage_usec, 0);
}

#[test]
fn test_adversarial_process_exits_before_statm_read() {
    let cgroup_dir = tempdir().unwrap();
    let proc_dir = tempdir().unwrap();

    // Process 99999 does not exist in proc_dir
    let telemetry = collect_cgroup_telemetry_resilient_with_proc(
        cgroup_dir.path(),
        "ephemeral.service",
        Some(99999),
        proc_dir.path(),
    );

    assert!(telemetry.is_synthetic());
    assert_eq!(telemetry.unit.as_deref(), Some("ephemeral.service"));
    assert!(telemetry.memory_current_bytes.is_none());
    assert_eq!(telemetry.cpu_stat.usage_usec, 0);
    assert!(!telemetry.had_oom_kill());
}

#[test]
#[cfg(unix)]
fn test_adversarial_eacces_sysfs_and_procfs_mode_000() {
    let dir = tempdir().unwrap();
    let cgroup = dir.path().join("restricted.service");
    fs::create_dir_all(&cgroup).unwrap();

    let files = [
        "cpu.stat",
        "memory.current",
        "memory.max",
        "memory.events",
        "io.stat",
        "cgroup.events",
    ];

    for name in &files {
        let fpath = cgroup.join(name);
        fs::write(&fpath, b"dummy content\n").unwrap();
        fs::set_permissions(&fpath, fs::Permissions::from_mode(0o000)).unwrap();
    }

    // Individual readers must return clean defaults without returning Err
    let cpu = read_cgroup_cpu(&cgroup).expect("cpu_reader must handle EACCES");
    assert_eq!(cpu.usage_usec, 0);

    let (cur, max, ev) = read_cgroup_memory(&cgroup).expect("memory_reader must handle EACCES");
    assert_eq!(cur, None);
    assert_eq!(max, None);
    assert_eq!(ev.oom, 0);

    let io = read_cgroup_io(&cgroup).expect("io_reader must handle EACCES");
    assert!(io.is_empty());

    let full = collect_cgroup_telemetry(dir.path(), "restricted.service").expect("collect must handle EACCES");
    assert_eq!(full.memory_current_bytes, None);
    assert_eq!(full.cpu_stat.usage_usec, 0);

    // Now test when cgroup directory does not exist (unmounted / absent cgroup hierarchy)
    let resilient = collect_cgroup_telemetry_resilient_with_proc(
        &dir.path().join("unmounted_cgroup"),
        "restricted.service",
        None,
        dir.path(),
    );
    assert!(resilient.is_synthetic());
    assert_eq!(resilient.cgroup_path, "/sys/fs/cgroup/synthetic/restricted.service");

    // Restore permissions for tempdir cleanup
    fs::set_permissions(&cgroup, fs::Permissions::from_mode(0o755)).unwrap();
    for name in &files {
        let fpath = cgroup.join(name);
        let _ = fs::set_permissions(&fpath, fs::Permissions::from_mode(0o644));
    }
}

#[test]
fn test_adversarial_cgroup_procfs_fallback_with_cgroup_file_contract() {
    let cgroup_dir = tempdir().unwrap();
    let proc_dir = tempdir().unwrap();

    let pid_dir = proc_dir.path().join("5555");
    fs::create_dir_all(&pid_dir).unwrap();

    fs::write(pid_dir.join("statm"), "20000 500 200 50 0 20 0\n").unwrap();
    let stat_content = "5555 (worker) S 1 5555 5555 0 -1 4194304 10 20 0 0 300 100 0 0 20 0 1 0 1000 10000 500\n";
    fs::write(pid_dir.join("stat"), stat_content).unwrap();
    // In real Linux environments, /proc/<pid>/cgroup contains the relative cgroup path
    fs::write(pid_dir.join("cgroup"), "0::/user.slice/user-1000.slice/app.service\n").unwrap();

    let telemetry = collect_cgroup_telemetry_resilient_with_proc(
        cgroup_dir.path(),
        "worker.service",
        Some(5555),
        proc_dir.path(),
    );

    let is_synthetic = telemetry.is_synthetic();
    assert!(
        is_synthetic,
        "is_synthetic() must return true on procfs fallback when /proc/<pid>/cgroup exists"
    );
}

#[test]
fn test_adversarial_flat_container_layouts_and_prefixes() {
    let dir = tempdir().unwrap();

    // 1. Flat container: /sys/fs/cgroup/webapp.service
    let flat_dir = dir.path().join("webapp.service");
    fs::create_dir_all(&flat_dir).unwrap();
    let res = locate_unit_cgroup(dir.path(), "webapp.service").unwrap();
    assert_eq!(res, flat_dir);

    // 2. Nested user slice: /sys/fs/cgroup/user.slice/user-app.service
    let user_dir = dir.path().join("user.slice/user-app.service");
    fs::create_dir_all(&user_dir).unwrap();
    let res = locate_unit_cgroup(dir.path(), "user-app.service").unwrap();
    assert_eq!(res, user_dir);

    // 3. Docker container: /sys/fs/cgroup/docker/my-db.scope
    let docker_dir = dir.path().join("docker/my-db.scope");
    fs::create_dir_all(&docker_dir).unwrap();
    let res = locate_unit_cgroup(dir.path(), "my-db.scope").unwrap();
    assert_eq!(res, docker_dir);

    // 4. Unit with leading slash
    let res = locate_unit_cgroup(dir.path(), "/webapp.service").unwrap();
    assert_eq!(res, flat_dir);

    // 5. Explicitly prefixed unit
    let res = locate_unit_cgroup(dir.path(), "docker/my-db.scope").unwrap();
    assert_eq!(res, docker_dir);
}

#[test]
fn test_adversarial_cgroup_psi_resilient_missing_and_corrupt() {
    let dir = tempdir().unwrap();
    let missing_cgroup = dir.path().join("nonexistent_cgroup");

    // Missing cgroup directory should fall back to synthetic zero without panic
    let psi = collect_cgroup_psi_resilient(&missing_cgroup, Some("cgroup.service".into()));
    assert!(psi.is_synthetic());
    assert_eq!(psi.unit.as_deref(), Some("cgroup.service"));
    assert_eq!(psi.cpu, PsiRecord::zero());
    assert_eq!(psi.memory, PsiRecord::zero());
    assert_eq!(psi.io, PsiRecord::zero());
}
