//! System bus endpoints: manager client, event listener, event types.

pub mod error;
pub mod event;
pub mod listener;
pub mod manager_client;

pub use error::DbusDriverError;
pub use event::{DbusEvent, UnitFailedEvent, UnitStateUpdate};
pub use listener::SystemdDbusListener;
pub use manager_client::{call_systemd_unit_method, get_service_properties};
pub use manager_client::{get_unit_properties, subscribe_manager};
