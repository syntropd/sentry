//! Activated-fd inspection: socket probing, disambiguation, hardening.

pub mod disambiguate;
pub mod error;
pub mod fd_flags;
pub mod socket_inspector;

pub use disambiguate::disambiguate_socket;
pub use error::ActivationError;
pub use fd_flags::harden_activated_fd;
pub use socket_inspector::{get_socket_bound_path, is_socket, is_unix_stream_listener};
