//! How the probe opens a database: read-write or read-only, under which sharing mode, and how
//! the outcome of one open reads in the output.
//!
//! **Role:** the one place that calls redb's open functions, so `hold`, `attempt` and `matrix`
//! open a file the same way.
//!
//! **Position:** used by `attempt`, `holder`, `matrix` and `inplace`; depends on `redb`.
//!
//! **Signals and state:** none; an open is timed with a monotonic clock.
//!
//! **Invariants:** a read-write open is `Builder::create` (it creates a missing file), a
//! read-only open is `Builder::open_read_only`; both carry the same sharing mode; a failed open
//! keeps redb's Display and Debug texts verbatim.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use clap::ValueEnum;
use redb::{Builder, Database, DatabaseError, ReadOnlyDatabase, TableDefinition};

/// The table every holder keeps its counter in.
pub(crate) const COUNTER_TABLE: TableDefinition<&str, u64> = TableDefinition::new("probe");

/// The key of the counter row in [`COUNTER_TABLE`].
pub(crate) const COUNTER_KEY: &str = "counter";

/// Whether an open may write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum OpenMode {
    /// `Builder::create`: read and write, creating the file when missing.
    #[value(name = "rw")]
    ReadWrite,
    /// `Builder::open_read_only`: read only; the file must exist.
    #[value(name = "ro")]
    ReadOnly,
}

impl OpenMode {
    /// The name the command line and the output use.
    pub(crate) fn name(self) -> &'static str {
        match self {
            OpenMode::ReadWrite => "rw",
            OpenMode::ReadOnly => "ro",
        }
    }
}

/// How processes share one database file: redb's `ConcurrencyMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Sharing {
    /// redb's `ExclusiveWriter`, its default: the writer locks the whole file; read-only
    /// handles share it only with each other.
    #[value(name = "exclusive-writer")]
    Exclusive,
    /// One writing process; read-only processes may follow its commits.
    SingleWriter,
    /// Many writing processes, one write transaction at a time.
    MultiWriter,
}

impl Sharing {
    /// The sharing mode an open uses when none is named: the experimental multi-writer when the
    /// `multiprocess` feature is built in, redb's default exclusive writer otherwise.
    pub(crate) fn build_default() -> Sharing {
        if cfg!(feature = "multiprocess") {
            Sharing::MultiWriter
        } else {
            Sharing::Exclusive
        }
    }

    /// The named mode, or the build default; refuses a shared mode this build cannot open.
    pub(crate) fn resolve(named: Option<Sharing>) -> anyhow::Result<Sharing> {
        let sharing = named.unwrap_or_else(Sharing::build_default);
        if sharing != Sharing::Exclusive && !cfg!(feature = "multiprocess") {
            anyhow::bail!(
                "--sharing {} needs a build with the `multiprocess` feature",
                sharing.name()
            );
        }
        Ok(sharing)
    }

    /// The name the command line and the output use.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Sharing::Exclusive => "exclusive-writer",
            Sharing::SingleWriter => "single-writer",
            Sharing::MultiWriter => "multi-writer",
        }
    }
}

/// A redb builder for the sharing mode, counting repair callbacks into `repair_calls`.
pub(crate) fn builder(sharing: Sharing, repair_calls: Option<Arc<AtomicUsize>>) -> Builder {
    let mut builder = Builder::new();
    #[cfg(feature = "multiprocess")]
    builder.set_concurrency_mode(match sharing {
        Sharing::Exclusive => redb::ConcurrencyMode::ExclusiveWriter,
        Sharing::SingleWriter => redb::ConcurrencyMode::SingleWriter,
        Sharing::MultiWriter => redb::ConcurrencyMode::MultiWriter,
    });
    #[cfg(not(feature = "multiprocess"))]
    debug_assert_eq!(sharing, Sharing::Exclusive);
    if let Some(calls) = repair_calls {
        builder.set_repair_callback(move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
        });
    }
    builder
}

/// An open database handle of either kind.
pub(crate) enum Handle {
    ReadWrite(Database),
    ReadOnly(ReadOnlyDatabase),
}

/// How one open went.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OpenOutcome {
    /// The open returned a handle after `ms` milliseconds.
    Opened { ms: f64 },
    /// The open returned an error after `ms` milliseconds.
    Failed {
        ms: f64,
        display: String,
        debug: String,
    },
}

impl OpenOutcome {
    /// The outcome of a failed open, with the error's own texts.
    pub(crate) fn failed(ms: f64, error: &DatabaseError) -> OpenOutcome {
        OpenOutcome::Failed {
            ms,
            display: error.to_string(),
            debug: format!("{error:?}"),
        }
    }

    /// One line: `opened in <ms> ms` or `failed in <ms> ms: <Display> | <Debug>`.
    pub(crate) fn render(&self) -> String {
        match self {
            OpenOutcome::Opened { ms } => format!("opened in {ms:.3} ms"),
            OpenOutcome::Failed { ms, display, debug } => {
                format!("failed in {ms:.3} ms: {display} | {debug}")
            }
        }
    }

    /// The error texts of a failed open, `None` for an open that succeeded.
    pub(crate) fn error_text(&self) -> Option<String> {
        match self {
            OpenOutcome::Opened { .. } => None,
            OpenOutcome::Failed { display, debug, .. } => Some(format!("{display} | {debug}")),
        }
    }
}

/// Opens `path` once, timing the call; the handle comes back only when the open succeeded.
pub(crate) fn open(
    path: &Path,
    mode: OpenMode,
    sharing: Sharing,
    repair_calls: Option<Arc<AtomicUsize>>,
) -> (OpenOutcome, Option<Handle>) {
    let builder = builder(sharing, repair_calls);
    let started = Instant::now();
    let opened = match mode {
        OpenMode::ReadWrite => builder.create(path).map(Handle::ReadWrite),
        OpenMode::ReadOnly => builder.open_read_only(path).map(Handle::ReadOnly),
    };
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    match opened {
        Ok(handle) => (OpenOutcome::Opened { ms }, Some(handle)),
        Err(error) => (OpenOutcome::failed(ms, &error), None),
    }
}

#[cfg(test)]
#[path = "tests/open_mode.rs"]
mod tests;
