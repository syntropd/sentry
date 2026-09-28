//! Service and mutation subcommands: daemon, mcp, setup, reset, triage.

pub mod cmd_daemon;
pub mod cmd_mcp;
pub mod cmd_reset;
pub mod cmd_setup;
pub mod cmd_triage;

pub use cmd_daemon::execute_daemon;
pub use cmd_mcp::execute_mcp;
pub use cmd_reset::execute_reset;
pub use cmd_setup::execute_setup;
pub use cmd_triage::execute_triage;
