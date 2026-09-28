//! A log line as the window writes it: the time since the window opened, the level and the
//! source in columns, then the message.
//!
//! **Role:** format each column of a line, and the shown lines as the text Copy hands over.
//!
//! **Position:** used by the log window's `ui` for every row it draws and for Copy.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** the columns have fixed widths in a monospace font, so rows line up; a source
//! is its module's last name (`pipeline::workers` → `workers`), cut to [`SOURCE_WIDTH`].

use std::fmt::Write as _;
use std::time::Duration;

use tracing::Level;

use crate::core::log_buffer::LogLine;
use crate::log_console::models::console::Console;

/// The characters of the time, level and source columns.
pub(crate) const TIME_WIDTH: usize = 9;
pub(crate) const LEVEL_WIDTH: usize = 5;
pub(crate) const SOURCE_WIDTH: usize = 14;

/// `12:34.567`, or `1:02:03.4` from an hour on; always [`TIME_WIDTH`] wide.
pub(crate) fn time(elapsed: Duration) -> String {
    let millis = elapsed.as_millis();
    let (hours, minutes) = (millis / 3_600_000, millis / 60_000 % 60);
    let (seconds, thousandths) = (millis / 1000 % 60, millis % 1000);
    let text = if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{}", thousandths / 100)
    } else {
        format!("{minutes:02}:{seconds:02}.{thousandths:03}")
    };
    format!("{text:>TIME_WIDTH$}")
}

/// `ERROR`, `WARN `, `INFO `, `DEBUG`, `TRACE`.
pub(crate) fn level(level: Level) -> String {
    format!("{:<LEVEL_WIDTH$}", level.as_str())
}

/// The module's last name, cut or padded to [`SOURCE_WIDTH`].
pub(crate) fn source(target: &str) -> String {
    let name = target.rsplit("::").next().unwrap_or(target);
    let cut: String = name.chars().take(SOURCE_WIDTH).collect();
    format!("{cut:<SOURCE_WIDTH$}")
}

/// The whole line, as Copy writes it.
pub(crate) fn line_text(line: &LogLine) -> String {
    format!(
        "{}  {}  {}  {}",
        time(line.elapsed),
        level(line.level),
        source(&line.target),
        line.message
    )
}

/// Every line the window shows, one per text line.
pub(crate) fn copy_text(console: &Console) -> String {
    let mut text = String::new();
    for line in console.shown() {
        let _ = writeln!(text, "{}", line_text(line));
    }
    text
}

#[cfg(test)]
#[path = "tests/console_text.rs"]
mod tests;
