//! Passive systemd D-Bus listener message stream loop.

use crate::dbus::error::DbusDriverError;
use crate::dbus::event::DbusEvent;
use crate::dbus::manager_client::subscribe_manager;
use crate::dbus::match_rules::{build_manager_match_rule, build_unit_properties_match_rule};
use crate::dbus::path_unescape::object_path_to_unit_name;
use crate::dbus::property_decoder::{decode_unit_properties, extract_unit_failed_event};
use futures_lite::stream::StreamExt;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::debug;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MessageStream};

/// Pure Rust passive D-Bus listener that processes systemd signals.
pub struct SystemdDbusListener {
    conn: Connection,
}

impl SystemdDbusListener {
    /// Creates a new listener connected to the system D-Bus.
    pub async fn connect_system() -> Result<Self, DbusDriverError> {
        let conn = Connection::system().await?;
        Self::new(conn).await
    }

    /// Creates a listener over an existing D-Bus connection.
    pub async fn new(conn: Connection) -> Result<Self, DbusDriverError> {
        subscribe_manager(&conn).await?;

        // Install D-Bus signal match rules via org.freedesktop.DBus.AddMatch
        let manager_rule = build_manager_match_rule().to_string();
        conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "AddMatch",
            &(manager_rule,),
        )
        .await?;

        let properties_rule = build_unit_properties_match_rule().to_string();
        conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "AddMatch",
            &(properties_rule,),
        )
        .await?;

        Ok(Self { conn })
    }

    /// Returns a reference to the active D-Bus connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Spawns the signal processing stream, forwarding events to an mpsc channel.
    pub fn spawn_event_stream(self) -> (tokio::task::JoinHandle<()>, mpsc::Receiver<DbusEvent>) {
        let (tx, rx) = mpsc::channel(256);
        let mut stream = MessageStream::from(&self.conn);

        let handle = tokio::spawn(async move {
            while let Some(msg_result) = stream.next().await {
                let msg = match msg_result {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let header = msg.header();
                let member = match header.member() {
                    Some(m) => m.as_str(),
                    None => continue,
                };
                let iface = match header.interface() {
                    Some(i) => i.as_str(),
                    None => continue,
                };

                if iface == "org.freedesktop.systemd1.Manager" {
                    Self::handle_manager_signal(member, &msg, &tx).await;
                } else if iface == "org.freedesktop.DBus.Properties" && member == "PropertiesChanged"
                {
                    Self::handle_properties_signal(&msg, &tx).await;
                }
            }
        });

        (handle, rx)
    }

    async fn handle_manager_signal(member: &str, msg: &zbus::Message, tx: &mpsc::Sender<DbusEvent>) {
        let body = msg.body();
        match member {
            "UnitNew" => {
                if let Ok((id, path)) = body.deserialize::<(String, String)>() {
                    let _ = tx.send(DbusEvent::UnitNew { id, path }).await;
                }
            }
            "UnitRemoved" => {
                if let Ok((id, path)) = body.deserialize::<(String, String)>() {
                    let _ = tx.send(DbusEvent::UnitRemoved { id, path }).await;
                }
            }
            "JobNew" => {
                if let Ok((id, job_path, unit)) = body.deserialize::<(u32, String, String)>() {
                    let _ = tx.send(DbusEvent::JobNew { id, job_path, unit }).await;
                }
            }
            "JobRemoved" => {
                if let Ok((id, job_path, unit, result)) =
                    body.deserialize::<(u32, String, String, String)>()
                {
                    let _ = tx.send(DbusEvent::JobRemoved { id, job_path, unit, result }).await;
                }
            }
            "UnitFilesChanged" => {
                let _ = tx.send(DbusEvent::UnitFilesChanged).await;
            }
            "Reloading" => {
                if let Ok(active) = body.deserialize::<bool>() {
                    let _ = tx.send(DbusEvent::Reloading(active)).await;
                }
            }
            _ => debug!("Ignored Manager signal: {member}"),
        }
    }

    async fn handle_properties_signal(msg: &zbus::Message, tx: &mpsc::Sender<DbusEvent>) {
        let header = msg.header();
        let path = match header.path() {
            Some(p) => p.as_str(),
            None => return,
        };

        let unit_name = match object_path_to_unit_name(path) {
            Ok(name) => name,
            Err(_) => return,
        };

        let body = msg.body();
        if let Ok((iface, changed, _invalidated)) =
            body.deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
        {
            if let Some(update) = decode_unit_properties(unit_name, &iface, &changed) {
                if let Some(failed) = extract_unit_failed_event(&update) {
                    let _ = tx.send(DbusEvent::UnitFailed(failed)).await;
                }
                let _ = tx.send(DbusEvent::UnitStateChanged(update)).await;
            }
        }
    }
}
