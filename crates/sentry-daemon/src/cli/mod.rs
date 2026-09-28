//! Command-line interface definitions, argument parsing, and shell completions.

pub mod args;
pub mod completions;
pub mod exit_codes;
pub mod help;
pub mod parser;

pub use args::{CliArgs, Command};
pub use completions::generate_completions;
pub use exit_codes::{EX_CONFIG, EX_DATAERR, EX_IOERR, EX_NOPERM};
pub use exit_codes::{EX_OK, EX_SOFTWARE, EX_UNAVAILABLE, EX_USAGE};
pub use help::{format_global_help, format_subcommand_help};
pub use parser::parse_cli_args;
