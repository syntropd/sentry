//! 1:1 Unit QA tests for resilient PSI collection with unprivileged fallback.

use sentry_core::models::{PressureTelemetry, PsiLine, PsiRecord};
use sentry_driver::psi::{collect_cgroup_psi_resilient, collect_system_psi_resilient};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
fn test_psi_models_zero_and_synthetic() {
    let zero_line = PsiLine::zero();
    assert_eq!(zero_line.avg10, 0.0);
    assert_eq!(zero_line.avg60, 0.0);
    assert_eq!(zero_line.avg300, 0.0);
    assert_eq!(zero_line.total_usec, 0);

    let zero_rec = PsiRecord::zero();
    assert_eq!(zero_rec.some, zero_line);
    assert_eq!(zero_rec.full, Some(zero_line));

    let syn = PressureTelemetry::synthetic_zero(Some("foo.service".into()));
    assert!(syn.is_synthetic());
    assert_eq!(syn.unit.as_deref(), Some("foo.service"));
    assert_eq!(syn.cpu, PsiRecord::zero());
    assert_eq!(syn.memory, PsiRecord::zero());
    assert_eq!(syn.io, PsiRecord::zero());
}

#[test]
fn test_collect_system_psi_resilient_live() {
    let dir = tempdir().unwrap();
    let content = "some avg10=1.50 avg60=0.50 avg300=0.10 total=500\n";
    fs::write(dir.path().join("cpu"), content).unwrap();
    fs::write(dir.path().join("memory"), content).unwrap();
    fs::write(dir.path().join("io"), content).unwrap();

    let telemetry = collect_system_psi_resilient(dir.path());
    assert!(!telemetry.is_synthetic());
    assert_eq!(telemetry.cpu.some.avg10, 1.50);
}

#[test]
fn test_collect_system_psi_resilient_fallback_to_loadavg_and_meminfo() {
    let dir = tempdir().unwrap();
    let pressure_dir = dir.path().join("pressure");
    fs::create_dir_all(&pressure_dir).unwrap();

    // Mock loadavg and meminfo in parent of pressure directory.
    // Load dwarfs any real core count so the stall is always non-zero.
    fs::write(dir.path().join("loadavg"), "9999.00 8.00 6.00 5/500 1234\n").unwrap();
    fs::write(
        dir.path().join("meminfo"),
        "MemTotal:       1000000 kB\nMemAvailable:     50000 kB\n",
    )
    .unwrap();

    let telemetry = collect_system_psi_resilient(&pressure_dir);
    assert!(telemetry.is_synthetic());
    // Since load 9999.00 > online cores, CPU stall is non-zero
    assert!(telemetry.cpu.some.avg10 > 0.0);
    // Since available memory is 5% (<10%), memory stall is non-zero
    assert!(telemetry.memory.some.avg10 > 0.0);
}

#[test]
fn test_collect_system_psi_resilient_completely_missing_falls_back_to_synthetic_zero() {
    let dir = tempdir().unwrap();
    let empty_dir = dir.path().join("nonexistent_pressure");

    let telemetry = collect_system_psi_resilient(&empty_dir);
    assert!(telemetry.is_synthetic());
    assert_eq!(telemetry.io, PsiRecord::zero());
}

#[test]
#[cfg(unix)]
fn test_collect_system_psi_resilient_permission_denied_handled_gracefully() {
    let dir = tempdir().unwrap();
    let cpu_file = dir.path().join("cpu");
    fs::write(&cpu_file, "some avg10=1.00 avg60=0.00 avg300=0.00 total=0\n").unwrap();
    fs::set_permissions(&cpu_file, fs::Permissions::from_mode(0o000)).unwrap();

    let telemetry = collect_system_psi_resilient(dir.path());
    // Should not panic, should return valid telemetry (synthetic fallback)
    assert!(telemetry.is_synthetic());

    // Restore permissions so tempdir cleanup succeeds
    fs::set_permissions(&cpu_file, fs::Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn test_collect_cgroup_psi_resilient_live_and_fallback() {
    let dir = tempdir().unwrap();
    let cgroup_dir = dir.path().join("test_cgroup");
    fs::create_dir_all(&cgroup_dir).unwrap();

    let content = "some avg10=2.00 avg60=1.00 avg300=0.50 total=100\n";
    fs::write(cgroup_dir.join("cpu.pressure"), content).unwrap();
    fs::write(cgroup_dir.join("memory.pressure"), content).unwrap();
    fs::write(cgroup_dir.join("io.pressure"), content).unwrap();

    let live = collect_cgroup_psi_resilient(&cgroup_dir, Some("live.service".into()));
    assert!(!live.is_synthetic());
    assert_eq!(live.unit.as_deref(), Some("live.service"));
    assert_eq!(live.cpu.some.avg10, 2.00);

    let missing_dir = dir.path().join("missing_cgroup");
    let fallback = collect_cgroup_psi_resilient(&missing_dir, Some("fallback.service".into()));
    assert!(fallback.is_synthetic());
    assert_eq!(fallback.unit.as_deref(), Some("fallback.service"));
    assert_eq!(fallback.cpu, PsiRecord::zero());
}
