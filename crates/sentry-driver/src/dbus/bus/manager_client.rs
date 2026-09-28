//! Pure Rust D-Bus client for systemd manager methods.

use crate::dbus::error::DbusDriverError;
use zbus::zvariant::OwnedObjectPath;
use zbus::Connection;

/// Subscribes to systemd signal broadcasts via `org.freedesktop.systemd1.Manager.Subscribe`.
pub async fn subscribe_manager(conn: &Connection) -> Result<(), DbusDriverError> {
    conn.call_method(
        Some("org.freedesktop.systemd1"),
        "/org/freedesktop/systemd1",
        Some("org.freedesktop.systemd1.Manager"),
        "Subscribe",
        &(),
    )
    .await
    .map_err(|e| DbusDriverError::SubscriptionFailed(e.to_string()))?;

    Ok(())
}

/// Invokes a unit lifecycle method (e.g. `RestartUnit`, `ReloadUnit`, `ResetFailedUnit`).
///
/// Returns the job object path as a String for job-spawning methods, or an empty string for `ResetFailedUnit`.
pub async fn call_systemd_unit_method(
    conn: &Connection,
    method: &str,
    unit: &str,
    mode: &str,
) -> Result<String, DbusDriverError> {
    if method == "ResetFailedUnit" {
        conn.call_method(
            Some("org.freedesktop.systemd1"),
            "/org/freedesktop/systemd1",
            Some("org.freedesktop.systemd1.Manager"),
            method,
            &(unit,),
        )
        .await?;
        Ok(String::new())
    } else {
        let reply = conn
            .call_method(
                Some("org.freedesktop.systemd1"),
                "/org/freedesktop/systemd1",
                Some("org.freedesktop.systemd1.Manager"),
                method,
                &(unit, mode),
            )
            .await?;

        let job_path: OwnedObjectPath = reply.body().deserialize()?;
        Ok(job_path.as_str().to_string())
    }
}

/// Retrieves all properties from `org.freedesktop.systemd1.Unit` interface for a unit.
pub async fn get_unit_properties(
    conn: &Connection,
    unit: &str,
) -> Result<std::collections::HashMap<String, zbus::zvariant::OwnedValue>, DbusDriverError> {
    let path = crate::dbus::path_escape::unit_name_to_object_path(unit)?;
    let reply = conn
        .call_method(
            Some("org.freedesktop.systemd1"),
            &path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &("org.freedesktop.systemd1.Unit",),
        )
        .await?;
    let props: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
        reply.body().deserialize()?;
    Ok(props)
}

/// Retrieves all properties from `org.freedesktop.systemd1.Service` interface for a unit.
pub async fn get_service_properties(
    conn: &Connection,
    unit: &str,
) -> Result<std::collections::HashMap<String, zbus::zvariant::OwnedValue>, DbusDriverError> {
    let path = crate::dbus::path_escape::unit_name_to_object_path(unit)?;
    let reply = conn
        .call_method(
            Some("org.freedesktop.systemd1"),
            &path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &("org.freedesktop.systemd1.Service",),
        )
        .await?;
    let props: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
        reply.body().deserialize()?;
    Ok(props)
}

