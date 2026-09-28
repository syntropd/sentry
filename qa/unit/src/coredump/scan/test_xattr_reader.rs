//! 1:1 Unit QA tests for coredump xattr reader.

use rustix::fs::{setxattr, XattrFlags};
use sentry_driver::coredump::read_coredump_xattrs;
use sentry_driver::coredump::xattr_reader::read_xattr_two_pass;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_read_coredump_xattrs_file_not_found() {
    let dir = tempdir().unwrap();
    let missing_path = dir.path().join("missing.core");
    let err = read_coredump_xattrs(&missing_path).unwrap_err();
    assert!(err.to_string().contains("Coredump file not found"));
}

#[test]
fn test_read_coredump_xattrs_plain_file() {
    let dir = tempdir().unwrap();
    let core_file = dir.path().join("dummy.core");
    fs::write(&core_file, b"ELF...").unwrap();

    let xattrs = read_coredump_xattrs(&core_file).unwrap();
    assert!(xattrs.pid.is_none());
    assert!(xattrs.signal.is_none());
    assert!(xattrs.comm.is_none());
    assert!(xattrs.exe.is_none());
    assert!(xattrs.unit.is_none());
    assert!(xattrs.uid.is_none());
    assert!(xattrs.gid.is_none());
    assert!(xattrs.hostname.is_none());
    assert!(xattrs.rlimit.is_none());
    assert!(xattrs.timestamp.is_none());
    assert!(xattrs.proc_status.is_none());
    assert!(xattrs.cmdline.is_none());
    assert!(xattrs.extra.is_empty());
}

#[test]
fn test_read_coredump_xattrs_with_extended_attributes() {
    let dir = tempdir().unwrap();
    let core_file = dir.path().join("test_crash.core");
    fs::write(&core_file, b"test_payload").unwrap();

    // Set standard and extended attributes if filesystem permits
    let attrs = [
        ("user.coredump.pid", "9876"),
        ("user.coredump.signal", "11"),
        ("user.coredump.comm", "worker_app"),
        ("user.coredump.exe", "/usr/bin/worker_app"),
        ("user.coredump.unit", "worker_app.service"),
        ("user.coredump.uid", "1001"),
        ("user.coredump.gid", "1002"),
        ("user.coredump.hostname", "sandbox-host"),
        ("user.coredump.rlimit", "18446744073709551615"),
        ("user.coredump.timestamp", "1789180209000000"),
        ("user.coredump.cmdline", "/usr/bin/worker_app --daemon"),
        ("user.coredump.container_id", "container_xyz"),
    ];

    let mut set_count = 0;
    for (name, val) in attrs {
        if setxattr(&core_file, name, val.as_bytes(), XattrFlags::empty()).is_ok() {
            set_count += 1;
        }
    }

    if set_count > 0 {
        let xattrs = read_coredump_xattrs(&core_file).unwrap();
        assert_eq!(xattrs.pid, Some(9876));
        assert_eq!(xattrs.signal, Some(11));
        assert_eq!(xattrs.comm.as_deref(), Some("worker_app"));
        assert_eq!(xattrs.exe.as_deref(), Some("/usr/bin/worker_app"));
        assert_eq!(xattrs.unit.as_deref(), Some("worker_app.service"));
        assert_eq!(xattrs.uid, Some(1001));
        assert_eq!(xattrs.gid, Some(1002));
        assert_eq!(xattrs.hostname.as_deref(), Some("sandbox-host"));
        assert_eq!(xattrs.rlimit.as_deref(), Some("18446744073709551615"));
        assert_eq!(xattrs.timestamp, Some(1789180209000000));
        assert_eq!(xattrs.cmdline.as_deref(), Some("/usr/bin/worker_app --daemon"));
        assert_eq!(xattrs.extra.get("container_id").map(|s| s.as_str()), Some("container_xyz"));
    }
}

#[test]
fn test_read_coredump_xattrs_large_attribute_two_pass() {
    let dir = tempdir().unwrap();
    let core_file = dir.path().join("large_xattr.core");
    fs::write(&core_file, b"test_payload").unwrap();

    // Create a 4096-byte proc_status attribute (far exceeding old 1024-byte limit)
    let large_status = "Name:\tworker_app\nState:\tR (running)\n".repeat(128);
    assert!(large_status.len() > 1024);

    if setxattr(
        &core_file,
        "user.coredump.proc_status",
        large_status.as_bytes(),
        XattrFlags::empty(),
    )
    .is_ok()
    {
        let val = read_xattr_two_pass(&core_file, "user.coredump.proc_status");
        assert_eq!(val, Some(large_status.clone()));

        let xattrs = read_coredump_xattrs(&core_file).unwrap();
        assert_eq!(xattrs.proc_status, Some(large_status));
    }
}

#[test]
fn test_read_xattr_two_pass_missing_attribute() {
    let dir = tempdir().unwrap();
    let core_file = dir.path().join("plain.core");
    fs::write(&core_file, b"content").unwrap();

    let val = read_xattr_two_pass(&core_file, "user.coredump.nonexistent");
    assert!(val.is_none());
}
