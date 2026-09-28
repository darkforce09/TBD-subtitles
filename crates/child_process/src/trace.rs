//! What a child did, as `tracing` events: its start, each stderr line, and how it ended.
//!
//! **Role:** name a child by its program and pid, and log its life under the `child_process`
//! target, so the app's log window shows every external program as it runs.
//!
//! **Position:** used by `runner.rs` and `running.rs` when they spawn and reap, and by
//! `stream.rs` for each stderr line; the app installs the subscriber, the tools install none.
//!
//! **Signals and state:** none; each event goes to whatever subscriber the process has.
//!
//! **Invariants:** a normal start, line or exit is a debug event; a non-zero exit, signal or
//! timeout is a warning; a cancelled child is debug, since someone asked for it; stdout is never
//! logged here (PCM audio, replies, worker progress).

use std::path::Path;
use std::time::Duration;

use crate::{Run, RunError};

/// An argument longer than this, or on several lines (a prompt, a schema), is logged as its size.
const LONG_ARGUMENT: usize = 160;

/// One child, as the log names it: `ffmpeg[4242]`.
#[derive(Debug, Clone)]
pub(crate) struct Tag {
    name: String,
    pid: u32,
}

impl Tag {
    /// Log the start of `run` as `pid`, with its command line.
    pub(crate) fn started(run: &Run, pid: u32) -> Tag {
        let command_line = command_line(run);
        let name = Path::new(&run.program)
            .file_name()
            .map_or_else(|| run.program.clone(), |n| n.to_string_lossy().into_owned());
        let tag = Tag { name, pid };
        tracing::debug!(target: "child_process", "{tag} started: {command_line}");
        tag
    }

    /// Log one line the child wrote to stderr; blank lines are skipped.
    pub(crate) fn line(&self, line: &str) {
        for part in line.split('\r').map(str::trim_end) {
            if !part.is_empty() {
                tracing::debug!(target: "child_process", "{self} {part}");
            }
        }
    }

    /// Log a child that exited with `code` after `duration`.
    pub(crate) fn exited(&self, code: i32, duration: Duration) {
        let secs = duration.as_secs_f64();
        if code == 0 {
            tracing::debug!(target: "child_process", "{self} exited 0 after {secs:.2} s");
        } else {
            tracing::warn!(target: "child_process", "{self} exited {code} after {secs:.2} s");
        }
    }

    /// Log a child that ended without an exit code.
    pub(crate) fn failed(&self, error: &RunError) {
        match error {
            RunError::Cancelled { .. } => {
                tracing::debug!(target: "child_process", "{self} cancelled");
            }
            _ => tracing::warn!(target: "child_process", "{self} {error}"),
        }
    }

    /// Log a child killed because its handle was dropped before it was waited on.
    pub(crate) fn abandoned(&self) {
        tracing::debug!(target: "child_process", "{self} killed: its stream was abandoned");
    }
}

/// The command line as the log shows it: long or multi-line arguments stand as their size, and an
/// argument with spaces is quoted.
fn command_line(run: &Run) -> String {
    let mut line = run.program.clone();
    for arg in &run.args {
        line.push(' ');
        if arg.len() > LONG_ARGUMENT || arg.contains('\n') {
            line.push_str(&format!("<{} bytes>", arg.len()));
        } else if arg.is_empty() || arg.contains(char::is_whitespace) {
            line.push_str(&format!("'{arg}'"));
        } else {
            line.push_str(arg);
        }
    }
    line
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]", self.name, self.pid)
    }
}

#[cfg(test)]
#[path = "tests/trace.rs"]
mod tests;
