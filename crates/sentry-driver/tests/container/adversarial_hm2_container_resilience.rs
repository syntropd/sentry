//! Empirical Challenger 2 Adversarial Stress Suite for HM2:
//! Unprivileged Container & Virtualized Host Fallback Resilience (R2).

use sentry_driver::cgroup::{collect_cgroup_telemetry_resilient_with_proc, extract_pid_metrics};
use sentry_driver::psi::{collect_system_psi_resilient, parse_loadavg_to_psi, read_proc_loadavg};
use std::fs::{self, Permissions};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
fn test_adversarial_malformed_loadavg_matrix() {
    let hostile_inputs = [
        "", "   ", "\n", "\t\t\n", "1.0", "1.0 2.0",
        "not_a_float 1.0 2.0 1/1 1", "1.0 garbage 2.0", "1.0 2.0 garbage",
        "-0.01 1.0 2.0", "1.0 -0.01 2.0", "1.0 2.0 -0.01",
        "NaN 1.0 2.0", "1.0 NaN 2.0", "1.0 2.0 NaN",
        "inf 1.0 2.0", "-inf 1.0 2.0", "1e308 1e308 1e308", "1e500 1e500 1e500",
        "18446744073709551616 18446744073709551616 18446744073709551616",
        "999999999999999999999999999999999999999999999999999999999999999999999999",
    ];

    for &input in &hostile_inputs {
        let panic_res = std::panic::catch_unwind(|| {
            let _ = parse_loadavg_to_psi(input);
        });
        assert!(panic_res.is_ok(), "Panic on loadavg string: {:?}", input);
    }

    let dir = tempdir().unwrap();
    let loadavg_path = dir.path().join("loadavg");
    let non_ascii = [0xFF, 0xFE, 0x00, 0x80, 0x7F, b'1', b'.', b'0'];
    fs::write(&loadavg_path, non_ascii).unwrap();
    assert!(read_proc_loadavg(&loadavg_path).is_err());

    let sys_psi = collect_system_psi_resilient(dir.path());
    assert!(sys_psi.is_synthetic());
}

#[test]
fn test_adversarial_malformed_statm_matrix() {
    let dir = tempdir().unwrap();
    let hostile_statm = [
        "", "   ", "not_an_int", "size resident",
        "100 -50 0 0 0 0 0", "-100 -200", "100.5 50.2",
        "100 18446744073709551616",
        "100 9999999999999999999999999999999999999999999999999",
        "100",
    ];

    for (i, &content) in hostile_statm.iter().enumerate() {
        let pid = 2000 + i as u32;
        let pid_dir = dir.path().join(pid.to_string());
        fs::create_dir_all(&pid_dir).unwrap();
        fs::write(pid_dir.join("statm"), content).unwrap();

        let panic_res = std::panic::catch_unwind(|| {
            let _ = extract_pid_metrics(dir.path(), pid);
            let _ = collect_cgroup_telemetry_resilient_with_proc(
                dir.path(), "svc", Some(pid), dir.path(),
            );
        });
        assert!(panic_res.is_ok(), "Panic on statm content: {:?}", content);
    }
}

#[test]
fn test_adversarial_malformed_stat_parentheses_and_comm() {
    let dir = tempdir().unwrap();
    let hostile_stats = [
        "1234 (foo (bar) baz) S 1 1234 1234 0 -1 4194304 10 20 0 0 150 250 0 0 20 0 1 0 1000 10000 500\n",
        "1235 (process)))) S 1 1235 1235 0 -1 4194304 10 20 0 0 150 250 0 0 20 0 1 0 1000 10000 500\n",
        "1236 ( ) ( ( ) ) S 1 1236 1236 0 -1 4194304 10 20 0 0 150 250 0 0 20 0 1 0 1000 10000 500\n",
        "1237 (unclosed S 1 2 3\n",
        "1238 () S 1 1238 1238 0 -1 4194304 10 20 0 0 150 250 0 0 20 0 1 0 1000 10000 500\n",
        "1239 (foo)",
        "1240 (wörker 🚀) S 1 1240 1240 0 -1 4194304 10 20 0 0 150 250 0 0 20 0 1 0 1000 10000 500\n",
        "1241 (foo) S 1 1241 1241 0 -1 4194304 10 20 0 0 not_int not_int 0 0 20 0 1 0 1000 10000 500\n",
        "1242 (foo) S 1 1242 1242 0 -1 4194304 10 20 0 0 18446744073709551615 18446744073709551615 0 0 20 0 1 0 1000 10000 500\n",
    ];

    for (i, &content) in hostile_stats.iter().enumerate() {
        let pid = 3000 + i as u32;
        let pid_dir = dir.path().join(pid.to_string());
        fs::create_dir_all(&pid_dir).unwrap();
        fs::write(pid_dir.join("stat"), content).unwrap();

        let panic_res = std::panic::catch_unwind(|| {
            let res = extract_pid_metrics(dir.path(), pid);
            if pid == 3000 {
                let m = res.expect("Should parse nested parens");
                assert_eq!(m.cpu_stat.user_usec, 150 * 10_000);
                assert_eq!(m.cpu_stat.system_usec, 250 * 10_000);
            } else if pid == 3008 {
                let m = res.expect("Should parse u64::MAX without overflow");
                assert_eq!(m.cpu_stat.usage_usec, u64::MAX);
            }
        });
        assert!(panic_res.is_ok(), "Panic on stat content: {:?}", content);
    }
}

#[test]
#[cfg(unix)]
fn test_adversarial_dac_0o000_permissions_lxc_simulation() {
    let dir = tempdir().unwrap();
    let proc_dir = dir.path().join("proc");
    let cgroup_dir = dir.path().join("sys_cgroup");
    fs::create_dir_all(&proc_dir).unwrap();
    fs::create_dir_all(&cgroup_dir).unwrap();

    let locked_loadavg = proc_dir.join("loadavg");
    fs::write(&locked_loadavg, "5.0 5.0 5.0 1/100 100\n").unwrap();
    fs::set_permissions(&locked_loadavg, Permissions::from_mode(0o000)).unwrap();

    let locked_pid = proc_dir.join("7777");
    fs::create_dir_all(&locked_pid).unwrap();
    fs::write(locked_pid.join("statm"), "100 50 0 0 0 0 0\n").unwrap();
    fs::write(locked_pid.join("stat"), "7777 (svc) S 1 7777 7777 0 -1 0 0 0 0 0 10 10 0 0\n").unwrap();
    fs::set_permissions(locked_pid.join("statm"), Permissions::from_mode(0o000)).unwrap();
    fs::set_permissions(locked_pid.join("stat"), Permissions::from_mode(0o000)).unwrap();

    let locked_unit = cgroup_dir.join("system.slice/test.service");
    fs::create_dir_all(&locked_unit).unwrap();
    fs::write(locked_unit.join("cpu.stat"), "usage_usec 1000\n").unwrap();
    fs::write(locked_unit.join("memory.current"), "50000\n").unwrap();
    fs::write(locked_unit.join("io.stat"), "8:0 rbytes=100\n").unwrap();
    fs::set_permissions(locked_unit.join("cpu.stat"), Permissions::from_mode(0o000)).unwrap();
    fs::set_permissions(locked_unit.join("memory.current"), Permissions::from_mode(0o000)).unwrap();
    fs::set_permissions(locked_unit.join("io.stat"), Permissions::from_mode(0o000)).unwrap();

    let panic_res = std::panic::catch_unwind(|| {
        let psi = collect_system_psi_resilient(&proc_dir.join("pressure"));
        assert!(psi.is_synthetic());

        let cg = collect_cgroup_telemetry_resilient_with_proc(
            &cgroup_dir, "test.service", Some(7777), &proc_dir,
        );
        assert!(!cg.cgroup_path.is_empty());
    });
    assert!(panic_res.is_ok(), "Panic on 0o000 DAC restricted environment");

    // Revert permissions so tempdir can clean up cleanly
    fs::set_permissions(&locked_loadavg, Permissions::from_mode(0o644)).unwrap();
    fs::set_permissions(locked_pid.join("statm"), Permissions::from_mode(0o644)).unwrap();
    fs::set_permissions(locked_pid.join("stat"), Permissions::from_mode(0o644)).unwrap();
    fs::set_permissions(locked_unit.join("cpu.stat"), Permissions::from_mode(0o644)).unwrap();
    fs::set_permissions(locked_unit.join("memory.current"), Permissions::from_mode(0o644)).unwrap();
    fs::set_permissions(locked_unit.join("io.stat"), Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn test_adversarial_fd_leak_under_stress() {
    let dir = tempdir().unwrap();
    let proc_dir = dir.path().join("proc");
    let cgroup_dir = dir.path().join("cgroup");
    fs::create_dir_all(&proc_dir).unwrap();
    fs::create_dir_all(&cgroup_dir).unwrap();

    let get_fd_count = || -> usize {
        #[cfg(target_os = "linux")]
        {
            fs::read_dir("/proc/self/fd").map(|r| r.count()).unwrap_or(0)
        }
        #[cfg(not(target_os = "linux"))]
        {
            0
        }
    };

    let initial_fds = get_fd_count();

    for i in 0..300 {
        let _ = collect_system_psi_resilient(&proc_dir.join("pressure"));
        let _ = collect_cgroup_telemetry_resilient_with_proc(
            &cgroup_dir, "leak_test.service", Some(i % 100), &proc_dir,
        );
    }

    let final_fds = get_fd_count();
    // Allow small variance from background runtime threads, but no continuous growth
    assert!(
        final_fds <= initial_fds + 5,
        "Possible FD leak detected! Initial FDs: {}, Final FDs: {}",
        initial_fds,
        final_fds
    );
}
