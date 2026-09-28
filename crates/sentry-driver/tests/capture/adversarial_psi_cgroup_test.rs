//! Adversarial stress test harness for PSI and cgroup telemetry drivers.
//!
//! Asserts error boundaries, numeric overflow handling, malformed lines,
//! and extreme inputs across /proc/pressure and cgroups v2 parsers.

use sentry_driver::cgroup::{read_cgroup_cpu, read_cgroup_io, read_cgroup_memory};
use sentry_driver::psi::{parse_psi_line, parse_psi_record};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_adversarial_psi_line_parser_boundary_and_malformed() {
    // Missing whitespace separator
    assert!(parse_psi_line("some").is_err());
    assert!(parse_psi_line("").is_err());
    assert!(parse_psi_line("   \t\n  ").is_err());

    // Invalid line prefix
    assert!(parse_psi_line("none avg10=0.00 avg60=0.00 avg300=0.00 total=0").is_err());
    assert!(parse_psi_line("cpu avg10=0.00 avg60=0.00 avg300=0.00 total=0").is_err());

    // Incomplete fields: missing total
    let err = parse_psi_line("some avg10=0.00 avg60=0.00 avg300=0.00");
    assert!(err.is_err(), "Missing total must error");

    // Incomplete fields: missing avg10
    let err = parse_psi_line("some avg60=0.00 avg300=0.00 total=100");
    assert!(err.is_err(), "Missing avg10 must error");

    // Non-numeric float
    let err = parse_psi_line("some avg10=corrupted avg60=0.00 avg300=0.00 total=0");
    assert!(err.is_err());

    // Non-numeric total
    let err = parse_psi_line("some avg10=0.00 avg60=0.00 avg300=0.00 total=-1");
    assert!(err.is_err());

    let err = parse_psi_line("some avg10=0.00 avg60=0.00 avg300=0.00 total=99999999999999999999");
    assert!(err.is_err());

    // Extra unknown tokens interleaved (should be ignored gracefully)
    let ok = parse_psi_line("some avg10=1.23 extra=data avg60=4.56 foo=bar avg300=7.89 total=999");
    let (kind, line) = ok.expect("extra tokens should be ignored");
    assert_eq!(kind, "some");
    assert_eq!(line.avg10, 1.23);
    assert_eq!(line.avg60, 4.56);
    assert_eq!(line.avg300, 7.89);
    assert_eq!(line.total_usec, 999);
}

#[test]
fn test_adversarial_psi_extreme_numeric_values() {
    // Maximum u64 total
    let text = format!("some avg10=100.00 avg60=99.99 avg300=95.00 total={}", u64::MAX);
    let (kind, line) = parse_psi_line(&text).expect("u64::MAX total should parse");
    assert_eq!(kind, "some");
    assert_eq!(line.total_usec, u64::MAX);

    // Record parser with only full line (missing mandatory 'some' line)
    let only_full = "full avg10=0.00 avg60=0.00 avg300=0.00 total=0\n";
    assert!(parse_psi_record(only_full).is_err());

    // Record parser with empty content
    assert!(parse_psi_record("").is_err());
    assert!(parse_psi_record("\n\n\t   \n").is_err());
}

#[test]
fn test_adversarial_cgroup_memory_extreme_and_corrupt() {
    let dir = tempdir().unwrap();

    // 1. Corrupt memory.current (non-numeric)
    fs::write(dir.path().join("memory.current"), "not_a_number\n").unwrap();
    let res = read_cgroup_memory(dir.path());
    assert!(res.is_err(), "Non-numeric memory.current must error");

    // 2. Overflow memory.current (> u64::MAX)
    fs::write(dir.path().join("memory.current"), "18446744073709551616\n").unwrap();
    let res = read_cgroup_memory(dir.path());
    assert!(res.is_err(), "Overflow memory.current must error");

    // 3. Negative memory.current
    fs::write(dir.path().join("memory.current"), "-1000\n").unwrap();
    let res = read_cgroup_memory(dir.path());
    assert!(res.is_err(), "Negative memory.current must error");

    // 4. Extreme valid u64::MAX memory.current and "max" memory.max
    fs::write(dir.path().join("memory.current"), format!("{}\n", u64::MAX)).unwrap();
    fs::write(dir.path().join("memory.max"), "max\n").unwrap();
    let (cur, max, _) = read_cgroup_memory(dir.path()).unwrap();
    assert_eq!(cur, Some(u64::MAX));
    assert_eq!(max, None); // "max" maps to None (unlimited)

    // 5. memory.max with numeric value
    fs::write(dir.path().join("memory.max"), "4294967296\n").unwrap();
    let (_, max, _) = read_cgroup_memory(dir.path()).unwrap();
    assert_eq!(max, Some(4294967296));

    // 6. Malformed memory.events (partial lines, unknown keys, invalid counts)
    let malformed_events = "\
oom 5
oom_kill not_an_int
unknown_future_metric 12345
truncated_line_without_value
low 1000
";
    fs::write(dir.path().join("memory.events"), malformed_events).unwrap();
    let (_, _, events) = read_cgroup_memory(dir.path()).unwrap();
    assert_eq!(events.oom, 5);
    assert_eq!(events.oom_kill, 0); // Invalid parsed as fallback 0
    assert_eq!(events.low, 1000);
}

#[test]
fn test_adversarial_cgroup_cpu_and_io_resilience() {
    let dir = tempdir().unwrap();

    // cpu.stat with empty file, partial tokens, duplicate keys
    let cpu_content = "\
usage_usec 9999999999
user_usec abc
system_usec 123456
nr_throttled
extra_kernel_key 999
";
    fs::write(dir.path().join("cpu.stat"), cpu_content).unwrap();
    let cpu = read_cgroup_cpu(dir.path()).unwrap();
    assert_eq!(cpu.usage_usec, 9999999999);
    assert_eq!(cpu.user_usec, 0); // malformed fallback
    assert_eq!(cpu.system_usec, 123456);

    // io.stat with corrupted device lines
    let io_content = "\
8:0 rbytes=1024 wbytes=2048 rios=10 wios=20 dbytes=0 dios=0
malformed_line_no_tokens
bad_device_no_colon rbytes=100
8:notanumber rbytes=200
:999 rbytes=300
9:0 corrupted_token_no_equals rbytes=4096 wbytes=invalid
";
    fs::write(dir.path().join("io.stat"), io_content).unwrap();
    let io_metrics = read_cgroup_io(dir.path()).unwrap();
    assert_eq!(io_metrics.len(), 2);
    assert_eq!(io_metrics[0].device, "8:0");
    assert_eq!(io_metrics[0].rbytes, 1024);
    assert_eq!(io_metrics[1].device, "9:0");
    assert_eq!(io_metrics[1].rbytes, 4096);
    assert_eq!(io_metrics[1].wbytes, 0); // invalid parsed as fallback 0
}
