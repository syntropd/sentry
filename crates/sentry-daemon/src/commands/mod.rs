//! Subcommand implementations for systemd-sentry CLI.

pub mod query;
pub mod serve;

pub use query::cmd_check;
pub use query::cmd_incidents;
pub use query::cmd_inspect;
pub use query::cmd_monitor;
pub use query::cmd_status;
pub use serve::cmd_daemon;
pub use serve::cmd_mcp;
pub use serve::cmd_reset;
pub use serve::cmd_setup;
pub use serve::cmd_triage;

pub use query::cmd_check::execute_check;
pub use query::cmd_incidents::execute_incidents;
pub use query::cmd_inspect::execute_inspect;
pub use query::cmd_monitor::execute_monitor;
pub use query::cmd_status::execute_status;
pub use serve::cmd_daemon::execute_daemon;
pub use serve::cmd_mcp::execute_mcp;
pub use serve::cmd_reset::execute_reset;
pub use serve::cmd_setup::execute_setup;
pub use serve::cmd_triage::execute_triage;
