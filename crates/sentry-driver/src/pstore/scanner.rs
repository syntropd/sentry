//! Scans persistent storage (/sys/fs/pstore, /var/lib/systemd/pstore) for panic logs.

use sentry_core::models::PstorePanicReport;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Maximum bytes read from any pstore file to prevent memory spikes.
pub const MAX_PSTORE_SCAN_BYTES: usize = 4096;

/// Scans standard pstore locations for kernel panic post-mortem records.
pub fn scan_pstore_reports() -> Vec<PstorePanicReport> {
    let mut reports = Vec::new();

    // Standard persistent storage locations
    let paths = [
        Path::new("/sys/fs/pstore"),
        Path::new("/var/lib/systemd/pstore"),
    ];

    for &dir in &paths {
        if dir.is_dir() {
            scan_directory(dir, &mut reports);
        }
    }

    reports
}

/// Maximum number of panic reports to parse in a single scan.
pub const MAX_PSTORE_REPORTS: usize = 32;

/// Scans a specific directory for pstore files (useful for tests and custom paths).
pub fn scan_directory(dir: &Path, reports: &mut Vec<PstorePanicReport>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        if reports.len() >= MAX_PSTORE_REPORTS {
            break;
        }

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if name.starts_with("dmesg-") || name.starts_with("console-") || name.starts_with("pmsg-") {
            if let Some(report) = parse_pstore_file(&path) {
                reports.push(report);
            }
        }
    }
}

/// Parses the leading bytes of a pstore file to extract panic signatures.
pub fn parse_pstore_file(path: &Path) -> Option<PstorePanicReport> {
    let Ok(mut file) = File::open(path) else {
        return None;
    };

    let mut buf = [0u8; MAX_PSTORE_SCAN_BYTES];
    let bytes_read = file.read(&mut buf).ok()?;
    if bytes_read == 0 {
        return None;
    }

    let text = String::from_utf8_lossy(&buf[..bytes_read]);
    extract_panic_report(path.to_string_lossy().as_ref(), &text)
}

/// Extracts panic summary and backtrace frames from raw text.
pub fn extract_panic_report(source_path: &str, content: &str) -> Option<PstorePanicReport> {
    let mut summary = String::new();
    let mut backtrace_snippet = Vec::new();
    let mut found_panic = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("Kernel panic - not syncing:")
            || trimmed.contains("BUG: unable to handle page fault")
            || trimmed.contains("Oops: 0000")
            || trimmed.contains("kernel BUG at")
        {
            found_panic = true;
            if summary.is_empty() {
                summary = trimmed.to_string();
            }
        }

        if found_panic
            && (trimmed.starts_with('?') || trimmed.starts_with("[<") || trimmed.contains("RIP:") || trimmed.contains("Call Trace:"))
            && backtrace_snippet.len() < 10
        {
            backtrace_snippet.push(trimmed.to_string());
        }
    }

    if found_panic {
        Some(PstorePanicReport {
            source_path: source_path.to_string(),
            summary: if summary.is_empty() { "Kernel panic detected".to_string() } else { summary },
            backtrace_snippet,
        })
    } else {
        None
    }
}
