//! Main executable entrypoint for binary alias `sentry`.
//!
//! Short-name alias of the `systemd-sentry` binary: collects the process
//! arguments, runs the shared [`sentry_daemon::entrypoint`] dispatch, and
//! exits with the resulting sysexits status code. Behavior is identical to
//! `systemd-sentry`; only the `argv[0]` name differs for interactive use.
//!
//! Exit codes follow `<sysexits.h>` via [`sentry_daemon::cli`].

use sentry_daemon::entrypoint;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exit_code = entrypoint(&args).await;
    std::process::exit(exit_code);
}
