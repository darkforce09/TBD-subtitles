//! Worker diagnostics on stderr and model exchanges through the worker channel.
//!
//! **Role:** log diagnostics to stderr and send full model calls to the job runner as
//! `ModelCall` frames.
//!
//! **Position:** initialized by the local-model worker before pipeline dispatch.
//!
//! **Signals and state:** reads `RUST_LOG`; installs one global tracing subscriber.
//!
//! **Invariants:** exchanges never reach stderr; each exchange is one `ModelCall` frame beside the
//! pipeline's progress frames.

use inference::llm::call_log::EXCHANGE_TARGET;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt as _};
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, fmt};

const DETAIL: &str = "info,tbd_subtitles_llm=debug,job_model=debug,child_process=debug,\
media_io=debug,subtitle_formats=debug,inference=debug,stages=debug,pipeline=debug";

pub(crate) fn initialise() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DETAIL));
    let stderr = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(true)
        .with_filter(filter)
        .with_filter(filter_fn(|metadata| metadata.target() != EXCHANGE_TARGET));
    tracing_subscriber::registry()
        .with(stderr)
        .with(
            WorkerChannelLayer
                .with_filter(filter_fn(|metadata| metadata.target() == EXCHANGE_TARGET)),
        )
        .init();
}

struct WorkerChannelLayer;

impl<S: Subscriber> Layer<S> for WorkerChannelLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut exchange = Exchange(None);
        event.record(&mut exchange);
        if let Some(json) = exchange.0 {
            worker_channel::worker::model_call(&json);
        }
    }
}

struct Exchange(Option<String>);

impl Visit for Exchange {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "exchange" {
            self.0 = Some(value.to_owned());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "exchange" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}
