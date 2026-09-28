//! Decodes systemd unit properties from D-Bus `PropertiesChanged` signals.

use crate::dbus::event::{UnitFailedEvent, UnitStateUpdate};
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;

/// Decodes changed unit properties into a strongly-typed `UnitStateUpdate`.
pub fn decode_unit_properties(
    unit: String,
    iface: &str,
    changed: &HashMap<String, OwnedValue>,
) -> Option<UnitStateUpdate> {
    if iface != "org.freedesktop.systemd1.Unit" && iface != "org.freedesktop.systemd1.Service" {
        return None;
    }

    let mut update = UnitStateUpdate {
        unit,
        ..Default::default()
    };

    if let Some(val) = changed.get("ActiveState") {
        if let Ok(s) = <&str>::try_from(val) {
            update.active_state = Some(s.to_string());
        }
    }

    if let Some(val) = changed.get("SubState") {
        if let Ok(s) = <&str>::try_from(val) {
            update.sub_state = Some(s.to_string());
        }
    }

    if let Some(val) = changed.get("Result") {
        if let Ok(s) = <&str>::try_from(val) {
            update.result = Some(s.to_string());
        }
    }

    if let Some(val) = changed.get("ExecMainCode") {
        if let Ok(code) = <i32>::try_from(val) {
            update.exec_main_code = Some(code);
        }
    }

    if let Some(val) = changed.get("ExecMainStatus") {
        if let Ok(status) = <i32>::try_from(val) {
            update.exec_main_status = Some(status);
        }
    }

    if let Some(val) = changed.get("MainPID") {
        if let Ok(pid) = <u32>::try_from(val) {
            update.main_pid = Some(pid);
        }
    }

    if let Some(val) = changed.get("ControlGroup") {
        if let Ok(cg) = <&str>::try_from(val) {
            update.cgroup = Some(cg.to_string());
        }
    }

    Some(update)
}

/// Evaluates whether a `UnitStateUpdate` represents a unit failure transition.
pub fn extract_unit_failed_event(update: &UnitStateUpdate) -> Option<UnitFailedEvent> {
    let is_failed_state = update.active_state.as_deref() == Some("failed")
        || update.sub_state.as_deref() == Some("failed");

    let is_failure_result = update.result.as_ref().map_or(false, |r| {
        r != "success" && r != "done"
    });

    if is_failed_state || is_failure_result {
        Some(UnitFailedEvent {
            unit: update.unit.clone(),
            active_state: update.active_state.clone().unwrap_or_else(|| "unknown".into()),
            sub_state: update.sub_state.clone().unwrap_or_else(|| "unknown".into()),
            result: update.result.clone(),
            exec_code: update.exec_main_code,
            exec_status: update.exec_main_status,
        })
    } else {
        None
    }
}
