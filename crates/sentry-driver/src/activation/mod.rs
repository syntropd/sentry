//! Native systemd socket activation implementation.

pub mod inspect;
pub mod listen;

pub use inspect::disambiguate;
pub use inspect::error;
pub use inspect::fd_flags;
pub use inspect::socket_inspector;
pub use listen::model;
pub use listen::name_parser;
pub use listen::parser;
pub use listen::pid_validator;

pub use inspect::disambiguate::disambiguate_socket;
pub use inspect::error::ActivationError;
pub use inspect::fd_flags::harden_activated_fd;
pub use inspect::socket_inspector::{get_socket_bound_path, is_socket, is_unix_stream_listener};
pub use listen::model::ActivatedSocket;
pub use listen::name_parser::parse_listen_fdnames;
pub use listen::parser::{parse_listen_fds, SD_LISTEN_FDS_START};
pub use listen::pid_validator::validate_listen_pid;
