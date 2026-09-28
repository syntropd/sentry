//! Systemd-pstore persistent storage kernel panic scanner.

pub mod scanner;

pub use scanner::{extract_panic_report, parse_pstore_file, scan_directory};
pub use scanner::{scan_pstore_reports, MAX_PSTORE_REPORTS};
