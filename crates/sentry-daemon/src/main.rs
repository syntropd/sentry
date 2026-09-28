//! Main executable entrypoint for `systemd-sentry`.
//!
//! Thin binary root over [`sentry_daemon::entrypoint`]: collects the process
//! arguments, runs the async supervisor/CLI dispatch, and exits with the
//! resulting sysexits status code. All behavior lives in the library so the
//! same dispatch is reusable from tests and the `sentry` alias binary.
//!
//! Exit codes follow `<sysexits.h>` via [`sentry_daemon::cli`].

use sentry_daemon::entrypoint;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exit_code = entrypoint(&args).await;
    std::process::exit(exit_code);
}
