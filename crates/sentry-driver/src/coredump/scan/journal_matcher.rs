//! Matches and extracts structured `CoredumpRecord` from journal entries.

use crate::coredump::backtrace_extractor::extract_backtrace;
use crate::journal::JournalExportEntry;
use sentry_core::models::CoredumpRecord;

/// Translates a numeric POSIX termination signal to its canonical string name.
fn signal_to_name(sig: i32) -> &'static str {
    match sig {
        11 => "SIGSEGV",
        6 => "SIGABRT",
        4 => "SIGILL",
        7 => "SIGBUS",
        8 => "SIGFPE",
        9 => "SIGKILL",
        15 => "SIGTERM",
        _ => "UNKNOWN",
    }
}

/// Matches a journal export entry against systemd-coredump signatures, returning
/// a `CoredumpRecord` if crash metadata is present.
pub fn match_coredump_record(entry: &JournalExportEntry) -> Option<CoredumpRecord> {
    if !entry.is_coredump() {
        return None;
    }

    let unit = entry
        .get_str("COREDUMP_UNIT")
        .or_else(|| entry.get_str("_SYSTEMD_UNIT"))
        .unwrap_or("unknown.service")
        .to_string();

    let pid = entry
        .get_str("COREDUMP_PID")
        .or_else(|| entry.get_str("_PID"))
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let signal = entry
        .get_str("COREDUMP_SIGNAL")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);

    let signal_name = entry
        .get_str("COREDUMP_SIGNAL_NAME")
        .map(|s| s.to_string())
        .unwrap_or_else(|| signal_to_name(signal).to_string());

    let executable = entry
        .get_str("COREDUMP_EXE")
        .or_else(|| entry.get_str("_EXE"))
        .map(|s| s.to_string());

    let message = entry.message().unwrap_or_default();
    let stack_trace = extract_backtrace(&message, 30);

    let core_file = entry
        .get_str("COREDUMP_FILENAME")
        .map(|s| s.to_string());

    let timestamp_usec = entry.realtime_timestamp_usec().unwrap_or(0);
    let uid = entry
        .get_str("COREDUMP_UID")
        .or_else(|| entry.get_str("_UID"))
        .and_then(|s| s.parse::<u32>().ok());
    let gid = entry
        .get_str("COREDUMP_GID")
        .or_else(|| entry.get_str("_GID"))
        .and_then(|s| s.parse::<u32>().ok());

    Some(CoredumpRecord {
        unit,
        pid,
        signal,
        signal_name,
        executable,
        stack_trace,
        core_file,
        timestamp_usec,
        uid,
        gid,
    })
}
