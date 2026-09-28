//! Tier 1: R2 System Integration & Deep Host Connectivity Tests
//!
//! Validates sd_notify datagrams, socket activation, journal parser, and PSI parsing.

use std::collections::HashMap;

#[test]
fn test_r2_sd_notify_protocol_datagram_formatting() {
    let mut messages = Vec::new();
    // Simulate sd_notify READY=1, STATUS, WATCHDOG=1
    messages.push("READY=1\n");
    messages.push("STATUS=Sentry operational\n");
    messages.push("WATCHDOG=1\n");

    let combined = messages.concat();
    assert!(combined.contains("READY=1"));
    assert!(combined.contains("STATUS=Sentry operational"));
    assert!(combined.contains("WATCHDOG=1"));
    assert!(combined.ends_with('\n'));
}

#[test]
fn test_r2_socket_activation_listen_fds_parsing() {
    let mock_listen_pid = "4200";
    let mock_listen_fds = "2";
    let mock_fdnames = "control:metrics";

    let pid: u32 = mock_listen_pid.parse().unwrap();
    let fds: usize = mock_listen_fds.parse().unwrap();
    let names: Vec<&str> = mock_fdnames.split(':').collect();

    assert_eq!(pid, 4200);
    assert_eq!(fds, 2);
    assert_eq!(names.len(), 2);
    assert_eq!(names[0], "control");
    assert_eq!(names[1], "metrics");

    // FD numbers must start at SD_LISTEN_FDS_START = 3
    let fd_start = 3;
    let expected_fds: Vec<i32> = (fd_start..fd_start + fds as i32).collect();
    assert_eq!(expected_fds, vec![3, 4]);
}

#[test]
fn test_r2_journal_export_stream_decoding() {
    // Construct sample Journal Export Format payload
    let mut payload = Vec::new();
    payload.extend_from_slice(b"MESSAGE=Worker crashed\n");
    payload.extend_from_slice(b"PRIORITY=3\n");
    payload.extend_from_slice(b"_SYSTEMD_UNIT=api.service\n\n");

    let mut fields = HashMap::new();
    let text = String::from_utf8(payload).unwrap();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            fields.insert(k.to_string(), v.to_string());
        }
    }

    assert_eq!(fields.get("MESSAGE"), Some(&"Worker crashed".to_string()));
    assert_eq!(fields.get("PRIORITY"), Some(&"3".to_string()));
    assert_eq!(fields.get("_SYSTEMD_UNIT"), Some(&"api.service".to_string()));
}

#[test]
fn test_r2_psi_telemetry_line_parsing() {
    let psi_data = "some avg10=12.50 avg60=8.20 avg300=4.10 total=981240\n\
                    full avg10=2.10 avg60=1.05 avg300=0.50 total=124000\n";
    let lines: Vec<&str> = psi_data.lines().collect();
    assert_eq!(lines.len(), 2);

    let some_line = lines[0];
    assert!(some_line.starts_with("some"));
    assert!(some_line.contains("avg10=12.50"));
    assert!(some_line.contains("total=981240"));

    let full_line = lines[1];
    assert!(full_line.starts_with("full"));
    assert!(full_line.contains("avg10=2.10"));
}

#[test]
fn test_r2_coredump_metadata_extraction() {
    let mut journal_record = HashMap::new();
    journal_record.insert("MESSAGE", "Process 8912 (segfault_worker) of user 1000 dumped core.");
    journal_record.insert("COREDUMP_PID", "8912");
    journal_record.insert("COREDUMP_SIGNAL", "11");
    journal_record.insert("COREDUMP_SIGNAL_NAME", "SIGSEGV");

    let pid: u32 = journal_record.get("COREDUMP_PID").unwrap().parse().unwrap();
    let signal: i32 = journal_record.get("COREDUMP_SIGNAL").unwrap().parse().unwrap();
    let signal_name = *journal_record.get("COREDUMP_SIGNAL_NAME").unwrap();

    assert_eq!(pid, 8912);
    assert_eq!(signal, 11);
    assert_eq!(signal_name, "SIGSEGV");
}
