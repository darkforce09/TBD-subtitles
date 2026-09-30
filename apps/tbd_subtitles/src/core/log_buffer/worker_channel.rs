//! A worker's model calls, sent to the job runner through the worker channel.
//!
//! **Role:** send each `model_exchange` event of a worker process to the job runner as one
//! `ModelCall` frame holding the call's JSON.
//!
//! **Position:** installed by `logging::initialise` for the `worker` subcommand, filtered to the
//! `model_exchange` target; the frames go through `worker_channel::worker`.
//!
//! **Signals and state:** none of its own; the worker channel's lock keeps a call's frame whole
//! beside the progress frames.
//!
//! **Invariants:** one call is one frame; a call made before the channel is installed, or after
//! the runner stopped reading, is dropped, never written elsewhere.

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// The layer that sends a worker's model calls to the job runner.
pub(crate) struct WorkerChannelLayer;

impl<S: Subscriber> Layer<S> for WorkerChannelLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut exchange = Exchange(None);
        event.record(&mut exchange);
        if let Some(json) = exchange.0 {
            worker_channel::worker::model_call(&json);
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
