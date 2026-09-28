//! D-Bus wire codecs: path escaping, match rules, property decoding.

pub mod match_rules;
pub mod path_escape;
pub mod path_unescape;
pub mod property_decoder;

pub use match_rules::{build_manager_match_rule, build_unit_properties_match_rule};
pub use path_escape::{escape_unit_name, unit_name_to_object_path};
pub use path_unescape::{object_path_to_unit_name, unescape_unit_name};
pub use property_decoder::{decode_unit_properties, extract_unit_failed_event};
