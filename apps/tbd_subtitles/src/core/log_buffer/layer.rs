//! The `tracing` layer that fills the log window: each event becomes a line or, for a model
//! call, a kept call, with the video and step it belongs to.
//!
//! **Role:** remember the `video` and `step` fields of each span as it opens; for each event, take
//! its own `video`, `step` and `call` fields, else the nearest span's context; read a worker's
//! line back (`worker_line`); route `model_exchange` events to the calls, every other to the
//! lines.
//!
//! **Position:** installed by `logging::initialise` for the window's run, over the registry.
//!
//! **Signals and state:** each span's context lives in the registry's extensions until it
//! closes; everything else goes to the [`LogBuffer`].
//!
//! **Invariants:** a model call never becomes a line; an event's own context wins over its
//! spans'; an exchange that is not valid JSON is dropped, never shown half.

use std::fmt::Write as _;
use std::sync::Arc;

use inference::llm::call_log::EXCHANGE_TARGET;
use job_model::model_call::ModelExchange;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use super::{Fresh, LogBuffer, worker_line};

/// The `tracing` layer that copies every event it is shown into a [`LogBuffer`].
pub(crate) struct ConsoleLayer {
    buffer: Arc<LogBuffer>,
}

impl ConsoleLayer {
    pub(crate) fn new(buffer: Arc<LogBuffer>) -> ConsoleLayer {
        ConsoleLayer { buffer }
    }
}

/// The video and step a span is about.
#[derive(Debug, Clone, Default)]
struct SpanContext {
    video: Option<String>,
    step: Option<String>,
}

impl<S> Layer<S> for ConsoleLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, context: Context<'_, S>) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        if (fields.video.is_some() || fields.step.is_some())
            && let Some(span) = context.span(id)
        {
            span.extensions_mut().insert(SpanContext {
                video: fields.video,
                step: fields.step,
            });
        }
    }

    fn on_event(&self, event: &Event<'_>, context: Context<'_, S>) {
        let metadata = event.metadata();
        let mut fields = Fields::default();
        event.record(&mut fields);
        let (mut video, mut step) = (fields.video.take(), fields.step.take());
        if let Some(scope) = context.event_scope(event) {
            for span in scope {
                if let Some(around) = span.extensions().get::<SpanContext>() {
                    step = step.or_else(|| around.step.clone());
                    video = video.or_else(|| around.video.clone());
                }
            }
        }
        if metadata.target() == EXCHANGE_TARGET {
            let call = fields
                .exchange
                .and_then(|json| serde_json::from_str::<ModelExchange>(&json).ok());
            if let Some(call) = call {
                self.buffer.push_call(call, video, step);
            }
            return;
        }
        let mut fresh = Fresh {
            level: *metadata.level(),
            target: metadata.target().to_string(),
            message: fields.text(),
            video,
            step,
            call: fields.call,
        };
        if fresh.target == "child_process"
            && let Some(line) = worker_line::read(&fresh.message)
        {
            fresh.level = line.level;
            fresh.target = line.target;
            fresh.message = line.message;
            fresh.call = line.call;
        }
        self.buffer.push(fresh);
    }
}

/// An event's or a span's fields: the message, the context and call fields, and the rest.
#[derive(Default)]
struct Fields {
    message: String,
    video: Option<String>,
    step: Option<String>,
    call: Option<String>,
    exchange: Option<String>,
    rest: Vec<(&'static str, String)>,
}

impl Fields {
    /// The message followed by the other fields as `key=value`.
    fn text(&self) -> String {
        let mut text = self.message.clone();
        for (name, value) in &self.rest {
            if !text.is_empty() {
                text.push(' ');
            }
            let _ = write!(text, "{name}={value}");
        }
        text
    }

    fn keep(&mut self, field: &Field, value: String) {
        match field.name() {
            "message" => self.message = value,
            "video" => self.video = Some(value),
            "step" => self.step = Some(value),
            "call" => self.call = Some(value),
            "exchange" => self.exchange = Some(value),
            name => self.rest.push((name, value)),
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.keep(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.keep(field, format!("{value:?}"));
    }
}

#[cfg(test)]
#[path = "tests/layer.rs"]
mod tests;
