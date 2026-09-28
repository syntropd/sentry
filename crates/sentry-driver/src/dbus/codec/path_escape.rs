//! Hex-escaping implementation for systemd D-Bus unit paths.

use crate::dbus::error::DbusDriverError;
use zbus::zvariant::OwnedObjectPath;

/// Escapes a systemd unit name into a D-Bus object path component according to systemd rules.
///
/// ASCII alphanumeric characters remain unchanged; all other characters are converted
/// to `_` followed by two lowercase hexadecimal digits.
pub fn escape_unit_name(unit: &str) -> String {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(unit.len() * 3);
    for b in unit.bytes() {
        if b.is_ascii_alphanumeric() {
            out.push(b as char);
        } else {
            out.push('_');
            out.push(HEX_DIGITS[(b >> 4) as usize] as char);
            out.push(HEX_DIGITS[(b & 0x0f) as usize] as char);
        }
    }
    out
}

/// Converts a unit name into an `OwnedObjectPath` under `/org/freedesktop/systemd1/unit/`.
pub fn unit_name_to_object_path(unit: &str) -> Result<OwnedObjectPath, DbusDriverError> {
    let escaped = escape_unit_name(unit);
    let full_path = format!("/org/freedesktop/systemd1/unit/{escaped}");
    OwnedObjectPath::try_from(full_path.clone()).map_err(|e| {
        DbusDriverError::InvalidObjectPath(full_path, e.to_string())
    })
}
