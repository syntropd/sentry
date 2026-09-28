//! Scans `/var/lib/systemd/coredump` for matching crash dumps and metadata.
//!
//! Enriches crash records via filesystem extended attributes, with fallback to
//! bounded ELF crash header extraction for compressed (`.zst`, `.lz4`) or raw core files.

use crate::coredump::elf_extractor::extract_elf_crash_headers_from_file;
use crate::coredump::xattr_reader::read_coredump_xattrs;
use sentry_core::models::coredump::{CoredumpRecord, CoredumpXattrs, ElfCrashHeader};
use std::fs;
use std::path::Path;

/// Scan coredump directory for the most recent crash matching an executable or unit command name.
pub fn find_latest_coredump(coredump_dir: &Path, comm_match: Option<&str>) -> Option<CoredumpRecord> {
    if !coredump_dir.is_dir() {
        return None;
    }

    let entries = fs::read_dir(coredump_dir).ok()?;
    let mut latest_record: Option<(u64, CoredumpRecord)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Only inspect files containing "core" or ending in ".zst" / ".lz4"
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !filename.contains("core") && !filename.ends_with(".zst") && !filename.ends_with(".lz4") {
            continue;
        }

        let mut xattrs = read_coredump_xattrs(&path).unwrap_or_default();
        let mut elf_opt: Option<ElfCrashHeader> = None;

        // If xattrs are missing critical fields (comm, unit, or signal), fall back to ELF extraction
        if (xattrs.comm.is_none() && xattrs.unit.is_none()) || xattrs.signal.is_none() || xattrs.pid.is_none() {
            if let Ok(elf) = extract_elf_crash_headers_from_file(&path) {
                if xattrs.comm.is_none() {
                    xattrs.comm = elf.comm.clone();
                }
                if xattrs.pid.is_none() {
                    xattrs.pid = elf.pid;
                }
                if xattrs.signal.is_none() {
                    xattrs.signal = elf.signal;
                }
                if xattrs.cmdline.is_none() {
                    xattrs.cmdline = elf.cmdline.clone();
                }
                if xattrs.exe.is_none() {
                    xattrs.exe = elf.mapped_files.first().cloned();
                }
                elf_opt = Some(elf);
            }
        }

        if is_matching_crash(comm_match, &xattrs, elf_opt.as_ref()) {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0);

            let timestamp = xattrs.timestamp.unwrap_or(mtime);
            let record = build_coredump_record(&path, comm_match, xattrs, elf_opt.as_ref(), timestamp);

            if let Some((prev_time, _)) = latest_record {
                if timestamp > prev_time {
                    latest_record = Some((timestamp, record));
                }
            } else {
                latest_record = Some((timestamp, record));
            }
        }
    }

    latest_record.map(|(_, rec)| rec)
}

fn is_matching_crash(
    comm_match: Option<&str>,
    xattrs: &CoredumpXattrs,
    elf_opt: Option<&ElfCrashHeader>,
) -> bool {
    let expected = match comm_match {
        Some(exp) => exp.strip_suffix(".service").unwrap_or(exp),
        None => return true,
    };

    if let Some(actual_unit) = xattrs.unit.as_deref() {
        let act = actual_unit.strip_suffix(".service").unwrap_or(actual_unit);
        if act == expected {
            return true;
        }
    }

    if let Some(actual_comm) = xattrs.comm.as_deref() {
        if !actual_comm.is_empty()
            && (actual_comm == expected || expected.starts_with(actual_comm) || actual_comm.starts_with(expected))
        {
            return true;
        }
    }

    if let Some(elf) = elf_opt {
        if let Some(elf_comm) = elf.comm.as_deref() {
            if !elf_comm.is_empty()
                && (elf_comm == expected || expected.starts_with(elf_comm) || elf_comm.starts_with(expected))
            {
                return true;
            }
        }
    }

    false
}

fn build_coredump_record(
    path: &Path,
    comm_match: Option<&str>,
    xattrs: CoredumpXattrs,
    elf_opt: Option<&ElfCrashHeader>,
    timestamp_usec: u64,
) -> CoredumpRecord {
    let signal = xattrs.signal.or_else(|| elf_opt.and_then(|e| e.signal)).unwrap_or(0);
    let signal_name = match signal {
        11 => "SIGSEGV",
        6 => "SIGABRT",
        4 => "SIGILL",
        7 => "SIGBUS",
        8 => "SIGFPE",
        9 => "SIGKILL",
        15 => "SIGTERM",
        _ => "UNKNOWN",
    }.to_string();

    let exe = xattrs.exe.or_else(|| elf_opt.and_then(|e| e.mapped_files.first().cloned()));

    CoredumpRecord {
        unit: xattrs
            .unit
            .or_else(|| comm_match.map(|s| s.to_string()))
            .unwrap_or_default(),
        pid: xattrs.pid.or_else(|| elf_opt.and_then(|e| e.pid)).unwrap_or(0),
        signal,
        signal_name,
        executable: exe,
        stack_trace: None,
        core_file: Some(path.display().to_string()),
        timestamp_usec,
        uid: xattrs.uid,
        gid: xattrs.gid,
    }
}
