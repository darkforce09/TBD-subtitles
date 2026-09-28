//! Log lines to stderr, filtered by `RUST_LOG` (default `info`), coloured only on a terminal; the
//! window's run also writes them to a log file.
//!
//! **Role:** install the one global subscriber, and name the window's log file.
//!
//! **Position:** called by `cli::run` once it knows the subcommand.
//!
//! **Signals and state:** reads `RUST_LOG`, `XDG_STATE_HOME` and `HOME`; truncates the log file
//! when the window starts.
//!
//! **Invariants:** a launcher that drops stderr (Gear Lever) still leaves the window's log on
//! disk; a log file that cannot be opened leaves stderr alone, never stops the app.

use std::fs::{self, File};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt as _;

/// Install the global subscriber, also writing to `file` when given; call once, first thing.
pub(crate) fn initialise(file: Option<&Path>) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let opened = file.and_then(|path| match open(path) {
        Ok(opened) => Some(opened),
        Err(error) => {
            eprintln!(
                "tbd-subtitles: the log file {} cannot be written: {error}",
                path.display()
            );
            None
        }
    });
    let stderr_is_terminal = std::io::stderr().is_terminal();
    match opened {
        Some(opened) => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr.and(opened))
            .with_ansi(false)
            .with_target(false)
            .init(),
        None => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .with_ansi(stderr_is_terminal)
            .with_target(false)
            .init(),
    }
}

/// `$XDG_STATE_HOME/tbd-subtitles/tbd-subtitles.log`, or under `~/.local/state`; `None` with
/// neither set.
pub(crate) fn window_log_path() -> Option<PathBuf> {
    log_path(
        std::env::var_os("XDG_STATE_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

fn log_path(
    state_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let state = state_home
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            home.filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".local").join("state"))
        })?;
    Some(state.join("tbd-subtitles").join("tbd-subtitles.log"))
}

/// The log file, emptied, its folder made first; shared by every thread that logs.
fn open(path: &Path) -> std::io::Result<std::sync::Mutex<File>> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    File::create(path).map(std::sync::Mutex::new)
}

#[cfg(test)]
#[path = "tests/logging.rs"]
mod tests;
