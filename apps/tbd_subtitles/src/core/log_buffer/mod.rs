//! The log lines and language-model calls kept in memory for the log window, and the `tracing`
//! layers that fill them.
//!
//! **Role:** [`LogBuffer`] holds the newest [`CAPACITY`] lines and the newest [`CALL_CAPACITY`]
//! model calls of this process, each with the video and step it belongs to; `layer` turns each
//! `tracing` event into one of them; `worker_line` reads a worker's own log line back;
//! `worker_channel` sends a worker's model calls to the job runner.
//!
//! **Position:** installed by `logging::initialise`; read by the log window through the
//! application's environment.
//!
//! **Signals and state:** one mutex over each ring and its next sequence number; any thread may
//! push; the window copies what is newer than what it already has.
//!
//! **Invariants:** sequence numbers only grow, even across a clear; the oldest item leaves first
//! once a ring is full; a poisoned lock never stops logging; a call is never a line.

mod layer;
mod worker_channel;
mod worker_line;

pub(crate) use layer::ConsoleLayer;
pub(crate) use worker_channel::WorkerChannelLayer;

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use job_model::model_call::ModelExchange;
use tracing::Level;

/// How many lines the window keeps; older lines stay in the log file.
pub(crate) const CAPACITY: usize = 20_000;
/// How many model calls the window keeps; they are never written anywhere.
pub(crate) const CALL_CAPACITY: usize = 500;

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
    /// The video it is about, by name, when it is about one.
    pub(crate) video: Option<String>,
    /// The job step it is about, such as `adjudicate`, or `fix_it`.
    pub(crate) step: Option<String>,
    /// The model call it sums up, when it does.
    pub(crate) call: Option<String>,
}

/// A line to add: what [`LogBuffer::push`] numbers and times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fresh {
    pub(crate) level: Level,
    pub(crate) target: String,
    pub(crate) message: String,
    pub(crate) video: Option<String>,
    pub(crate) step: Option<String>,
    pub(crate) call: Option<String>,
}

impl Fresh {
    /// A line about no video or step.
    #[cfg(test)]
    pub(crate) fn new(level: Level, target: &str, message: impl Into<String>) -> Fresh {
        Fresh {
            level,
            target: target.to_string(),
            message: message.into(),
            video: None,
            step: None,
            call: None,
        }
    }

    /// The same line about `video` and `step`.
    #[cfg(test)]
    pub(crate) fn about(mut self, video: &str, step: Option<&str>) -> Fresh {
        self.video = Some(video.to_string());
        self.step = step.map(str::to_string);
        self
    }
}

/// One model call, kept with when it ended and what it was for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KeptCall {
    pub(crate) seq: u64,
    pub(crate) elapsed: Duration,
    pub(crate) video: Option<String>,
    pub(crate) step: Option<String>,
    pub(crate) call: ModelExchange,
}

/// The newest lines and model calls logged in this process.
#[derive(Debug)]
pub(crate) struct LogBuffer {
    started: Instant,
    lines: Mutex<Ring<LogLine>>,
    calls: Mutex<Ring<KeptCall>>,
}

#[derive(Debug)]
struct Ring<T> {
    items: VecDeque<T>,
    next: u64,
}

impl<T> Default for Ring<T> {
    fn default() -> Ring<T> {
        Ring {
            items: VecDeque::new(),
            next: 0,
        }
    }
}

impl<T: Clone> Ring<T> {
    /// Number `make`'s item, add it, and drop the oldest past `capacity`.
    fn push(&mut self, capacity: usize, make: impl FnOnce(u64) -> T) {
        let item = make(self.next);
        self.next += 1;
        if self.items.len() == capacity {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    /// The items whose number, read by `seq_of`, is `seq` or later.
    fn since(&self, seq: u64, seq_of: impl Fn(&T) -> u64) -> Vec<T> {
        let start = self.items.partition_point(|item| seq_of(item) < seq);
        self.items.range(start..).cloned().collect()
    }
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
            lines: Mutex::new(Ring::default()),
            calls: Mutex::new(Ring::default()),
        }
    }

    /// Add one line, dropping the oldest when full.
    pub(crate) fn push(&self, fresh: Fresh) {
        let elapsed = self.started.elapsed();
        lock(&self.lines).push(CAPACITY, |seq| LogLine {
            seq,
            elapsed,
            level: fresh.level,
            target: fresh.target,
            message: fresh.message,
            video: fresh.video,
            step: fresh.step,
            call: fresh.call,
        });
    }

    /// Add one model call about `video` and `step`, dropping the oldest when full.
    pub(crate) fn push_call(
        &self,
        call: ModelExchange,
        video: Option<String>,
        step: Option<String>,
    ) {
        let elapsed = self.started.elapsed();
        lock(&self.calls).push(CALL_CAPACITY, |seq| KeptCall {
            seq,
            elapsed,
            video,
            step,
            call,
        });
    }

    /// Forget every line kept so far.
    pub(crate) fn clear(&self) {
        lock(&self.lines).items.clear();
    }

    /// Forget every model call kept so far.
    pub(crate) fn clear_calls(&self) {
        lock(&self.calls).items.clear();
    }

    /// The lines kept whose sequence number is `seq` or later.
    pub(crate) fn since(&self, seq: u64) -> Vec<LogLine> {
        lock(&self.lines).since(seq, |line| line.seq)
    }

    /// The model calls kept whose sequence number is `seq` or later.
    pub(crate) fn calls_since(&self, seq: u64) -> Vec<KeptCall> {
        lock(&self.calls).since(seq, |call| call.seq)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
#[path = "tests/log_buffer.rs"]
mod tests;
