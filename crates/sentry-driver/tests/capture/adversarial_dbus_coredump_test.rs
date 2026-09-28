//! Adversarial stress test harness for D-Bus escaping and coredump extraction.
//!
//! Evaluates hex escaping corner cases, corrupted object paths,
//! and malformed journal coredump records.

use sentry_driver::coredump::{extract_backtrace, match_coredump_record};
use sentry_driver::dbus::{
    escape_unit_name, object_path_to_unit_name, unescape_unit_name, unit_name_to_object_path,
};
use sentry_driver::journal::JournalExportEntry;

#[test]
fn test_adversarial_dbus_unescape_corrupt_sequences() {
    // 1. Truncated underscore at end
    assert!(unescape_unit_name("nginx_").is_err());
    assert!(unescape_unit_name("nginx_a").is_err());

    // 2. Non-hex characters following underscore
    assert!(unescape_unit_name("nginx_zz").is_err());
    assert!(unescape_unit_name("nginx_1g").is_err());
    assert!(unescape_unit_name("nginx_g1").is_err());

    // 3. Escaped byte sequence that is invalid UTF-8 (e.g. 0xFF)
    let non_utf8_escaped = "_ff";
    let res = unescape_unit_name(non_utf8_escaped);
    assert!(
        res.is_err(),
        "Escaped 0xFF is invalid UTF-8 and should be rejected"
    );

    // 4. Incomplete multi-byte UTF-8 sequence (e.g. leading 0xC3 without continuation)
    let incomplete_utf8 = "_c3";
    let res = unescape_unit_name(incomplete_utf8);
    assert!(
        res.is_err(),
        "Incomplete UTF-8 sequence should be rejected"
    );
}

#[test]
fn test_adversarial_dbus_object_path_roundtrip_stress() {
    let unit_names = [
        "systemd-journald.service",
        "user@1000.service",
        "app-test_sub-unit.mount",
        "special!@#$%^&*()_+-=[]{}|;':\",./<>?`~.service",
        "",
        "español-servício.service",
        "🦀-crab.service",
    ];

    for name in &unit_names {
        let escaped = escape_unit_name(name);
        let unescaped = unescape_unit_name(&escaped).expect("roundtrip should decode");
        assert_eq!(&unescaped, name);

        // Path conversion
        if let Ok(path) = unit_name_to_object_path(name) {
            let recovered = object_path_to_unit_name(path.as_str()).expect("path to unit name");
            assert_eq!(&recovered, name);
        }
    }
}

#[test]
fn test_adversarial_coredump_backtrace_edge_cases() {
    // 1. Message without backtrace headers
    assert_eq!(extract_backtrace("Process 123 dumped core without backtrace.", 30), None);
    assert_eq!(extract_backtrace("", 30), None);

    // 2. Corrupted frame numbering (e.g. #999999999999999999999 or non-numeric)
    let malformed_bt = "\
Stack trace of thread 1234:
#abc 0x00007fff in foo()
#99999999999999999999999 0x00007fff in bar()
#0  0x00007fff main()
";
    let frames = extract_backtrace(malformed_bt, 30);
    // Should extract valid lines without crashing
    assert!(frames.is_some());

    // 3. Huge number of frames: test max_frames limit enforcement
    let mut huge_bt = String::from("Stack trace of thread 1:\n");
    for i in 0..100 {
        huge_bt.push_str(&format!("#{i} 0x{i:08x} in func_{i}()\n"));
    }
    let frames = extract_backtrace(&huge_bt, 15).expect("should extract");
    let line_count = frames.lines().count();
    assert!(
        line_count <= 16, // 15 frames + optional truncation notice
        "Frame count {line_count} should be bounded by max_frames"
    );
}

#[test]
fn test_adversarial_coredump_record_corrupted_fields() {
    let mut entry = JournalExportEntry::new();
    // Valid MESSAGE_ID for coredump
    entry.insert("MESSAGE_ID".into(), b"fc2e22bc6ee647b6b90729ab34a250b1".to_vec());
    // Corrupt non-numeric PID, SIGNAL, UID, GID
    entry.insert("COREDUMP_PID".into(), b"not_a_pid".to_vec());
    entry.insert("COREDUMP_SIGNAL".into(), b"not_a_sig".to_vec());
    entry.insert("COREDUMP_UID".into(), b"invalid_uid".to_vec());
    entry.insert("COREDUMP_GID".into(), b"-5".to_vec());
    entry.insert("MESSAGE".into(), b"Process 123 (my_app) of user 1000 dumped core.".to_vec());

    let record = match_coredump_record(&entry).expect("should match coredump record");
    // Should fall back safely to defaults without panicking
    assert_eq!(record.pid, 0);
    assert_eq!(record.signal, 0);
    assert_eq!(record.uid, None);
    assert_eq!(record.gid, None);
    assert_eq!(record.signal_name, "UNKNOWN");
}
