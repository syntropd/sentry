//! 1:1 Unit QA tests for socket activation subsystem.

use std::sync::Mutex;

/// Process-wide lock to serialize tests that mutate socket activation environment variables.
pub static ACTIVATION_ENV_LOCK: Mutex<()> = Mutex::new(());

pub mod parse;
pub mod socket;
pub mod verify;

pub use parse::test_activation_parser;
pub use parse::test_name_parser;
pub use socket::test_fd_flags;
pub use socket::test_listener_integration;
pub use socket::test_socket_inspector;
pub use verify::test_disambiguation;
pub use verify::test_model;
pub use verify::test_pid_validator;
