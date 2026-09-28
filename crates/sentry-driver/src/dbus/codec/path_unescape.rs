//! Hex-unescaping implementation for systemd D-Bus unit paths.

use crate::dbus::error::DbusDriverError;

/// Decodes a hex-escaped systemd path component back into the original unit name string.
pub fn unescape_unit_name(escaped: &str) -> Result<String, DbusDriverError> {
    let bytes = escaped.as_bytes();
    let mut out_bytes = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'_' {
            if i + 2 >= bytes.len() {
                return Err(DbusDriverError::InvalidPathEscape(
                    escaped.to_string(),
                    "Truncated hex sequence following underscore".to_string(),
                ));
            }
            let hex_str = std::str::from_utf8(&bytes[i + 1..=i + 2]).map_err(|e| {
                DbusDriverError::InvalidPathEscape(escaped.to_string(), e.to_string())
            })?;
            let byte_val = u8::from_str_radix(hex_str, 16).map_err(|e| {
                DbusDriverError::InvalidPathEscape(escaped.to_string(), e.to_string())
            })?;
            out_bytes.push(byte_val);
            i += 3;
        } else {
            out_bytes.push(bytes[i]);
            i += 1;
        }
    }

    String::from_utf8(out_bytes).map_err(|e| {
        DbusDriverError::InvalidPathEscape(escaped.to_string(), e.to_string())
    })
}

/// Extracts and unescapes the unit name from a full D-Bus object path.
pub fn object_path_to_unit_name(path: &str) -> Result<String, DbusDriverError> {
    let prefix = "/org/freedesktop/systemd1/unit/";
    let escaped = path.strip_prefix(prefix).ok_or_else(|| {
        DbusDriverError::InvalidObjectPath(
            path.to_string(),
            format!("Path does not contain prefix '{prefix}'"),
        )
    })?;
    unescape_unit_name(escaped)
}
