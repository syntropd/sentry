//! Reads `user.coredump.*` extended attributes from coredump files via `rustix`.
//!
//! Employs dynamic attribute enumeration via `listxattr` and two-pass sizing
//! via `getxattr` to handle large attribute values (up to 64 KiB) without `ERANGE` errors.

use rustix::fs::{getxattr, listxattr};
use sentry_core::error::CoredumpError;
use sentry_core::models::CoredumpXattrs;
use std::path::Path;

/// Maximum attribute size supported (64 KiB).
pub const MAX_XATTR_VALUE_SIZE: usize = 65536;

/// Reads `user.coredump.*` extended attributes from the specified coredump file.
pub fn read_coredump_xattrs(path: &Path) -> Result<CoredumpXattrs, CoredumpError> {
    if !path.exists() {
        return Err(CoredumpError::FileNotFound(path.to_path_buf()));
    }

    let mut xattrs = CoredumpXattrs::default();
    let names = list_coredump_xattr_names(path);

    if names.is_empty() {
        // Fallback for filesystems that do not support listxattr or report 0 attributes
        populate_known_xattrs(path, &mut xattrs);
        return Ok(xattrs);
    }

    for name in names {
        if let Some(suffix) = name.strip_prefix("user.coredump.") {
            if let Some(val) = read_xattr_two_pass(path, &name) {
                apply_xattr_field(&mut xattrs, suffix, &name, val);
            }
        }
    }

    Ok(xattrs)
}

/// Dynamically discovers all attribute names on the file.
fn list_coredump_xattr_names(path: &Path) -> Vec<String> {
    // Pass 1: determine required buffer size
    let needed = match listxattr(path, &mut []) {
        Ok(sz) => sz,
        Err(_) => 4096, // Fallback initial guess if query with empty slice fails
    };

    if needed == 0 {
        return Vec::new();
    }

    let alloc_size = needed.min(MAX_XATTR_VALUE_SIZE);
    let mut buf = vec![0u8; alloc_size];
    match listxattr(path, &mut buf) {
        Ok(sz) => parse_null_separated_strings(&buf[..sz]),
        Err(rustix::io::Errno::RANGE) => {
            // Buffer was undersized; retry with maximum bounded capacity
            let mut big_buf = vec![0u8; MAX_XATTR_VALUE_SIZE];
            if let Ok(sz) = listxattr(path, &mut big_buf) {
                parse_null_separated_strings(&big_buf[..sz])
            } else {
                Vec::new()
            }
        }
        Err(_) => Vec::new(),
    }
}

/// Reads an extended attribute value using two-pass sizing up to 64 KiB.
pub fn read_xattr_two_pass(path: &Path, name: &str) -> Option<String> {
    // Pass 1: query needed size
    let needed = match getxattr(path, name, &mut []) {
        Ok(sz) => sz,
        Err(rustix::io::Errno::RANGE) => 1024,
        Err(_) => return None,
    };

    if needed == 0 {
        return Some(String::new());
    }

    // Pass 2: allocate and fetch value
    let alloc_size = needed.min(MAX_XATTR_VALUE_SIZE);
    let mut buf = vec![0u8; alloc_size];
    match getxattr(path, name, &mut buf) {
        Ok(sz) => decode_xattr_string(&buf[..sz.min(alloc_size)]),
        Err(rustix::io::Errno::RANGE) => {
            // Retry with maximum bounded capacity if size changed or was underreported
            let mut max_buf = vec![0u8; MAX_XATTR_VALUE_SIZE];
            getxattr(path, name, &mut max_buf)
                .ok()
                .and_then(|sz| decode_xattr_string(&max_buf[..sz]))
        }
        Err(_) => None,
    }
}

/// Converts raw attribute bytes to an ANSI/UTF-8 sanitized string.
fn decode_xattr_string(bytes: &[u8]) -> Option<String> {
    std::str::from_utf8(bytes)
        .ok()
        .map(|s| s.trim_end_matches('\0').to_string())
}

/// Splits a buffer of null-separated C-strings into Rust Strings.
fn parse_null_separated_strings(buf: &[u8]) -> Vec<String> {
    buf.split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .filter_map(|s| std::str::from_utf8(s).ok().map(|str_val| str_val.to_string()))
        .collect()
}

/// Maps attribute key/value into the corresponding `CoredumpXattrs` field.
fn apply_xattr_field(xattrs: &mut CoredumpXattrs, suffix: &str, full_name: &str, val: String) {
    match suffix {
        "pid" => xattrs.pid = val.parse::<u32>().ok(),
        "signal" => xattrs.signal = val.parse::<i32>().ok(),
        "comm" => xattrs.comm = Some(val),
        "exe" => xattrs.exe = Some(val),
        "unit" => xattrs.unit = Some(val),
        "uid" => xattrs.uid = val.parse::<u32>().ok(),
        "gid" => xattrs.gid = val.parse::<u32>().ok(),
        "hostname" => xattrs.hostname = Some(val),
        "rlimit" => xattrs.rlimit = Some(val),
        "timestamp" => xattrs.timestamp = val.parse::<u64>().ok(),
        "proc_status" => xattrs.proc_status = Some(val),
        "cmdline" => xattrs.cmdline = Some(val),
        _ => {
            xattrs.extra.insert(suffix.to_string(), val.clone());
            xattrs.extra.insert(full_name.to_string(), val);
        }
    }
}

/// Queries standard well-known attributes when dynamic enumeration is unavailable.
fn populate_known_xattrs(path: &Path, xattrs: &mut CoredumpXattrs) {
    let known = [
        "pid", "signal", "comm", "exe", "unit", "uid", "gid", "hostname", "rlimit",
        "timestamp", "proc_status", "cmdline",
    ];
    for &k in &known {
        let full = format!("user.coredump.{k}");
        if let Some(val) = read_xattr_two_pass(path, &full) {
            apply_xattr_field(xattrs, k, &full, val);
        }
    }
}
