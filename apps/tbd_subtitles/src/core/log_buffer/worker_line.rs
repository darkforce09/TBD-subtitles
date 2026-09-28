//! A worker's own log line, read back: a worker logs to its stderr in `tracing`'s plain format,
//! and the job runner logs each stderr line again as the worker's program output.
//!
//! **Role:** turn `tbd-subtitles[4242] 2026-09-28T10:00:00.123456Z  INFO pipeline::tasks: text
//! call=4242-3` back into its level, target, message and call, so the log window shows the
//! worker's line for what it is, not as a program's debug line.
//!
//! **Position:** used by `layer` for every `child_process` line.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a line that is not in that format is left alone (`None`); the timestamp is
//! optional; a `call=` field anywhere in the text is taken out of the message.

use tracing::Level;

/// A worker's line, read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkerLine {
    pub(super) level: Level,
    pub(super) target: String,
    pub(super) message: String,
    pub(super) call: Option<String>,
}

/// Read `text`, a program's stderr line as `child_process` logs it (`name[pid] line`).
pub(super) fn read(text: &str) -> Option<WorkerLine> {
    let (_program, rest) = text.split_once(' ')?;
    let mut rest = rest.trim_start();
    if let Some((first, after)) = rest.split_once(char::is_whitespace)
        && is_timestamp(first)
    {
        rest = after.trim_start();
    }
    let (word, rest) = rest.split_once(char::is_whitespace)?;
    let level = match word {
        "ERROR" => Level::ERROR,
        "WARN" => Level::WARN,
        "INFO" => Level::INFO,
        "DEBUG" => Level::DEBUG,
        "TRACE" => Level::TRACE,
        _ => return None,
    };
    let (target, message) = rest.trim_start().split_once(": ")?;
    if target.is_empty() || target.contains(char::is_whitespace) {
        return None;
    }
    let (message, call) = take_call(message);
    Some(WorkerLine {
        level,
        target: target.to_string(),
        message,
        call,
    })
}

/// An RFC 3339 time as `tracing` writes it: `2026-09-28T10:00:00.123456Z`.
fn is_timestamp(word: &str) -> bool {
    word.len() >= 20 && word.starts_with(|c: char| c.is_ascii_digit()) && word.contains('T')
}

/// The message without its `call=` field, and the call's id.
fn take_call(message: &str) -> (String, Option<String>) {
    let words: Vec<&str> = message.split(' ').collect();
    let Some(at) = words.iter().position(|word| word.starts_with("call=")) else {
        return (message.to_string(), None);
    };
    let call = words[at].trim_start_matches("call=").to_string();
    let rest: Vec<&str> = words
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != at)
        .map(|(_, word)| *word)
        .collect();
    (rest.join(" "), Some(call).filter(|id| !id.is_empty()))
}

#[cfg(test)]
#[path = "tests/worker_line.rs"]
mod tests;
