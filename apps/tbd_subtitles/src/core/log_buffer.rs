//! The log lines kept in memory for the log window, and the `tracing` layer that fills them.
//!
//! **Role:** [`LogBuffer`] holds the newest [`CAPACITY`] lines of everything logged in this
//! process; [`ConsoleLayer`] turns each `tracing` event into a [`LogLine`] and pushes it.
//!
//! **Position:** installed by `logging::initialise` for the window's run; read by the log window
//! through the application's environment.
//!
//! **Signals and state:** one mutex over a ring of lines and the next sequence number; any
//! thread may push; the window copies the lines newer than the ones it already has.
//!
//! **Invariants:** sequence numbers only grow, even across a clear; the oldest line leaves first
//! once the ring is full; a poisoned lock never stops logging.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// How many lines the window keeps; older lines stay in the log file.
pub(crate) const CAPACITY: usize = 20_000;

/// One logged event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogLine {
    /// Its place in the whole run, counting from 0.
    pub(crate) seq: u64,
    /// Time since the log started.
    pub(crate) elapsed: Duration,
    pub(crate) level: Level,
    /// The module or program that logged it.
    pub(crate) target: String,
    /// The message, then any other fields as `key=value`.
    pub(crate) message: String,
}

/// The newest lines logged in this process.
#[derive(Debug)]
pub(crate) struct LogBuffer {
    started: Instant,
    ring: Mutex<Ring>,
}

#[derive(Debug, Default)]
struct Ring {
    lines: VecDeque<LogLine>,
    next: u64,
}

impl Default for LogBuffer {
    fn default() -> LogBuffer {
        LogBuffer::new()
    }
}

impl LogBuffer {
    /// An empty log whose clock starts now.
    pub(crate) fn new() -> LogBuffer {
        LogBuffer {
            started: Instant::now(),
            ring: Mutex::new(Ring::default()),
        }
    }

    /// Add one line, dropping the oldest when full.
    pub(crate) fn push(&self, level: Level, target: &str, message: String) {
        let elapsed = self.started.elapsed();
        let mut ring = self.lock();
        let seq = ring.next;
        ring.next += 1;
        if ring.lines.len() == CAPACITY {
            ring.lines.pop_front();
        }
        ring.lines.push_back(LogLine {
            seq,
            elapsed,
            level,
            target: target.to_string(),
            message,
        });
    }

    /// Forget every line kept so far.
    pub(crate) fn clear(&self) {
        self.lock().lines.clear();
    }

    /// The lines kept whose sequence number is `seq` or later.
    pub(crate) fn since(&self, seq: u64) -> Vec<LogLine> {
        let ring = self.lock();
        let start = ring.lines.partition_point(|line| line.seq < seq);
        ring.lines.range(start..).cloned().collect()
    }

    fn lock(&self) -> MutexGuard<'_, Ring> {
        self.ring
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The `tracing` layer that copies every event it is shown into a [`LogBuffer`].
pub(crate) struct ConsoleLayer {
    buffer: Arc<LogBuffer>,
}

impl ConsoleLayer {
    pub(crate) fn new(buffer: Arc<LogBuffer>) -> ConsoleLayer {
        ConsoleLayer { buffer }
    }
}

impl<S: Subscriber> Layer<S> for ConsoleLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let metadata = event.metadata();
        self.buffer
            .push(*metadata.level(), metadata.target(), event_text(event));
    }
}

/// The event's message followed by its other fields as `key=value`.
fn event_text(event: &Event<'_>) -> String {
    let mut fields = Fields::default();
    event.record(&mut fields);
    let mut text = fields.message;
    for (name, value) in fields.rest {
        if !text.is_empty() {
            text.push(' ');
        }
        let _ = write!(text, "{name}={value}");
    }
    text
}

#[derive(Default)]
struct Fields {
    message: String,
    rest: Vec<(&'static str, String)>,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.keep(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.keep(field, format!("{value:?}"));
    }
}

impl Fields {
    fn keep(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = value;
        } else {
            self.rest.push((field.name(), value));
        }
    }
}

#[cfg(test)]
#[path = "tests/log_buffer.rs"]
mod tests;
