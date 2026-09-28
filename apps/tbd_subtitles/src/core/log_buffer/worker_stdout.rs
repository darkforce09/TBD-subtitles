//! A worker's model calls, sent to the job runner on the worker's stdout.
//!
//! **Role:** write each `model_exchange` event of a worker process to stdout as one line,
//! `model-call <json>`, which the job runner reads beside the `progress` lines.
//!
//! **Position:** installed by `logging::initialise` for the `worker` subcommand, filtered to the
//! `model_exchange` target.
//!
//! **Signals and state:** writes to stdout under its lock, so a call's line never interleaves
//! with a progress line.
//!
//! **Invariants:** one call is one line; nothing else is written.

use std::io::Write as _;

use job_model::model_call::WORKER_LINE_PREFIX;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// The layer that writes a worker's model calls to its stdout.
pub(crate) struct WorkerStdoutLayer;

impl<S: Subscriber> Layer<S> for WorkerStdoutLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut exchange = Exchange(None);
        event.record(&mut exchange);
        if let Some(json) = exchange.0 {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{WORKER_LINE_PREFIX}{json}");
            let _ = out.flush();
        }
    }
}

/// The `exchange` field of an event.
struct Exchange(Option<String>);

impl Visit for Exchange {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "exchange" {
            self.0 = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "exchange" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}
