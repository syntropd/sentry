//! Match rule definitions for systemd D-Bus signals.

use zbus::MatchRule;

/// Builds the match rule for `org.freedesktop.systemd1.Manager` signals.
pub fn build_manager_match_rule() -> MatchRule<'static> {
    MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.systemd1")
        .unwrap()
        .interface("org.freedesktop.systemd1.Manager")
        .unwrap()
        .build()
}

/// Builds the match rule for `PropertiesChanged` signals on units under path namespace.
pub fn build_unit_properties_match_rule() -> MatchRule<'static> {
    MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.systemd1")
        .unwrap()
        .interface("org.freedesktop.DBus.Properties")
        .unwrap()
        .member("PropertiesChanged")
        .unwrap()
        .path_namespace("/org/freedesktop/systemd1/unit")
        .unwrap()
        .build()
}
