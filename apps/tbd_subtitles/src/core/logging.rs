//! Log lines to stderr, filtered by `RUST_LOG` (default `info`), coloured only on a terminal; the
//! window's run also writes them, in more detail, to a log file and to the log window's buffer.
//!
//! **Role:** install the one global subscriber, name the window's log file, and hold the buffer
//! the log window reads.
//!
//! **Position:** called by `cli::run` once it knows the subcommand; the application reads
//! [`console`] for its log window.
//!
//! **Signals and state:** reads `RUST_LOG`, `XDG_STATE_HOME` and `HOME`; truncates the log file
//! when the window starts; one process-wide [`LogBuffer`].
//!
//! **Invariants:** a launcher that drops stderr (Gear Lever) still leaves the window's log on
//! disk; a log file that cannot be opened leaves stderr alone, never stops the app; `RUST_LOG`,
//! when set, filters every output; a model call's exchange (its prompt and answer) never reaches
//! stderr or the log file: the window keeps it in memory, and a worker sends it through the worker
//! channel.

use std::fs::{self, File};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _, fmt};

use inference::llm::call_log::EXCHANGE_TARGET;

use super::log_buffer::{ConsoleLayer, LogBuffer, WorkerChannelLayer};

/// What the log file, the log window and a worker's stderr show: debug lines from this
/// workspace, info from everything else (egui, winit, the GL driver).
pub(crate) const DETAIL: &str = "info,tbd_subtitles=debug,job_model=debug,child_process=debug,\
media_io=debug,subtitle_formats=debug,inference=debug,stages=debug,pipeline=debug";

/// Which kind of run is logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogRun {
    /// The desktop window: stderr, the log file and the log window.
    Window,
    /// A step's worker process: stderr in detail, read line by line by the job runner.
    Worker,
    /// `process` or `fix` at a terminal: stderr only.
    Command,
}

/// The lines the log window shows; empty unless the window's run installed it.
pub(crate) fn console() -> Arc<LogBuffer> {
    static CONSOLE: OnceLock<Arc<LogBuffer>> = OnceLock::new();
    CONSOLE.get_or_init(|| Arc::new(LogBuffer::new())).clone()
}

/// Install the global subscriber for `run`; call once, first thing.
pub(crate) fn initialise(run: LogRun) {
    // A worker names each line's target, so the log window can tell who wrote it.
    let stderr = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_target(run == LogRun::Worker);
    match run {
        LogRun::Command => tracing_subscriber::registry()
            .with(stderr.with_filter(text_filter("info")))
            .init(),
        LogRun::Worker => tracing_subscriber::registry()
            .with(stderr.with_filter(text_filter(DETAIL)))
            .with(WorkerChannelLayer.with_filter(exchanges_only()))
            .init(),
        LogRun::Window => {
            let file = window_log_path().and_then(|path| open_or_report(&path));
            let file = file.map(|file| {
                fmt::layer()
                    .with_writer(file)
                    .with_ansi(false)
                    .with_filter(text_filter(DETAIL))
            });
            tracing_subscriber::registry()
                .with(stderr.with_filter(text_filter("info")))
                .with(file)
                .with(ConsoleLayer::new(console()).with_filter(with_exchanges(filter(DETAIL))))
                .init();
        }
    }
}

/// `RUST_LOG` when set, `default` otherwise.
fn filter(default: &str) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default))
}

/// As [`filter`], never with a model call's exchange: text outputs show its summary line only.
fn text_filter(default: &str) -> EnvFilter {
    filter(default).add_directive(directive(EXCHANGE_TARGET, "off"))
}

/// `filter` with every model call's exchange, whatever `RUST_LOG` says.
fn with_exchanges(filter: EnvFilter) -> EnvFilter {
    filter.add_directive(directive(EXCHANGE_TARGET, "trace"))
}

/// Model calls' exchanges and nothing else.
fn exchanges_only() -> EnvFilter {
    with_exchanges(EnvFilter::new("off"))
}

fn directive(target: &str, level: &str) -> tracing_subscriber::filter::Directive {
    format!("{target}={level}")
        .parse()
        .unwrap_or_else(|_| tracing_subscriber::filter::LevelFilter::OFF.into())
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

fn open_or_report(path: &Path) -> Option<Mutex<File>> {
    open(path)
        .map_err(|error| {
            eprintln!(
                "tbd-subtitles: the log file {} cannot be written: {error}",
                path.display()
            );
        })
        .ok()
}

/// The log file, emptied, its folder made first; shared by every thread that logs.
fn open(path: &Path) -> std::io::Result<Mutex<File>> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    File::create(path).map(Mutex::new)
}

#[cfg(test)]
#[path = "tests/logging.rs"]
mod tests;
