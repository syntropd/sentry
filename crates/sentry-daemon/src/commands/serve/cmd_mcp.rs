//! Command: Run the Model Context Protocol (MCP) server over stdio.

use crate::cli::exit_codes::{EX_OK, EX_SOFTWARE};
use sentry_mcp::{serve_stdio, McpState};
use tracing::error;

/// Execute the `mcp` subcommand.
pub async fn execute_mcp() -> i32 {
    let state = McpState::default();

    match serve_stdio(state).await {
        Ok(_) => EX_OK,
        Err(e) => {
            error!("MCP server terminated with error: {}", e);
            EX_SOFTWARE
        }
    }
}
