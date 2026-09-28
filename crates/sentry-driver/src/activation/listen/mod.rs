//! `LISTEN_FDS`/`LISTEN_PID` parsing and activated-socket modeling.

pub mod model;
pub mod name_parser;
pub mod parser;
pub mod pid_validator;

pub use model::ActivatedSocket;
pub use name_parser::parse_listen_fdnames;
pub use parser::{parse_listen_fds, SD_LISTEN_FDS_START};
pub use pid_validator::validate_listen_pid;
