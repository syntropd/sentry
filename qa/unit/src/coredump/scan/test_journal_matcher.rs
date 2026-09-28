//! 1:1 Unit QA tests for journal coredump matcher.

use sentry_driver::coredump::match_coredump_record;
use sentry_driver::journal::JournalExportEntry;

#[test]
fn test_match_coredump_record_valid() {
    let mut entry = JournalExportEntry::new();
    entry.insert(
        "MESSAGE_ID".into(),
        b"fc2e22bc6ee647b6b90729ab34a250b1".to_vec(),
    );
    entry.insert("COREDUMP_UNIT".into(), b"crashed.service".to_vec());
    entry.insert("COREDUMP_PID".into(), b"9999".to_vec());
    entry.insert("COREDUMP_SIGNAL".into(), b"11".to_vec());
    entry.insert("COREDUMP_SIGNAL_NAME".into(), b"SIGSEGV".to_vec());
    entry.insert("COREDUMP_EXE".into(), b"/usr/bin/broken".to_vec());
    entry.insert(
        "MESSAGE".into(),
        b"Stack trace of thread 9999:\n#0 0x1234 in main\n".to_vec(),
    );

    let record = match_coredump_record(&entry).expect("Must match coredump record");
    assert_eq!(record.unit, "crashed.service");
    assert_eq!(record.pid, 9999);
    assert_eq!(record.signal, 11);
    assert_eq!(record.signal_name, "SIGSEGV");
    assert_eq!(record.executable.as_deref(), Some("/usr/bin/broken"));
    assert!(record.stack_trace.as_ref().unwrap().contains("#0 0x1234 in main"));
    assert!(record.is_memory_fault());
}

#[test]
fn test_match_coredump_record_non_coredump_returns_none() {
    let mut entry = JournalExportEntry::new();
    entry.insert("MESSAGE".into(), b"Just normal logs".to_vec());
    assert!(match_coredump_record(&entry).is_none());
}
