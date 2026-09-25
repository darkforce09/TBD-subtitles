//! Log lines to stderr, filtered by `RUST_LOG` (default `info`), coloured only on a terminal.

use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;

/// Install the global subscriber; call once, first thing in `main`.
pub(crate) fn initialise() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_target(false)
        .init();
}
