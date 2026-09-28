//! Pure Rust systemd D-Bus listener and client via `zbus`.

pub mod bus;
pub mod codec;

pub use bus::error;
pub use bus::event;
pub use bus::listener;
pub use bus::manager_client;
pub use codec::match_rules;
pub use codec::path_escape;
pub use codec::path_unescape;
pub use codec::property_decoder;

pub use bus::error::DbusDriverError;
pub use bus::event::{DbusEvent, UnitFailedEvent, UnitStateUpdate};
pub use bus::listener::SystemdDbusListener;
pub use bus::manager_client::{
    call_systemd_unit_method, get_service_properties, get_unit_properties, subscribe_manager,
};
pub use codec::match_rules::{build_manager_match_rule, build_unit_properties_match_rule};
pub use codec::path_escape::{escape_unit_name, unit_name_to_object_path};
pub use codec::path_unescape::{object_path_to_unit_name, unescape_unit_name};
pub use codec::property_decoder::{decode_unit_properties, extract_unit_failed_event};
