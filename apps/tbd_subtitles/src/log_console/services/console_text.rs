//! A log line and a group header as the window writes them, and the shown lines as the text Copy
//! hands over.
//!
//! **Role:** write the time since the window opened, a group's header from its video and step,
//! and each line in full for Copy and the detail panel.
//!
//! **Position:** used by the log window's `ui` for every row it draws, its detail panel and Copy.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** the time is always nine characters (ten from the tenth hour), so rows line up
//! in the monospace font; a header names a job step by its stage and its plain title, never by
//! its file name alone.

use std::fmt::Write as _;
use std::str::FromStr as _;
use std::time::Duration;

use job_model::StepName;

use crate::core::log_buffer::LogLine;
use crate::core::steps::{stage_of, step_title};
use crate::log_console::models::activity::Activity;
use crate::log_console::models::who::Who;

/// `12:34.567`, or `1:02:03.4` from an hour on.
pub(crate) fn time(elapsed: Duration) -> String {
    let millis = elapsed.as_millis();
    let (hours, minutes) = (millis / 3_600_000, millis / 60_000 % 60);
    let (seconds, thousandths) = (millis / 1000 % 60, millis % 1000);
    let text = if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{}", thousandths / 100)
    } else {
        format!("{minutes:02}:{seconds:02}.{thousandths:03}")
    };
    format!("{text:>9}")
}

/// A step in words: `Settle the words — Language model settles the words`, `Fix It`, or the name
/// itself when it is no step the window knows.
pub(crate) fn step_words(step: &str) -> String {
    if step == "fix_it" {
        return "Fix It".to_string();
    }
    match StepName::from_str(step) {
        Ok(name) => format!("{} — {}", stage_of(name).title, step_title(name)),
        Err(_) => step.to_string(),
    }
}

/// A group's header: `Dressrosa 12 · Settle the words — Language model settles the words`.
pub(crate) fn header(video: Option<&str>, step: Option<&str>) -> String {
    match (video, step.map(step_words)) {
        (Some(video), Some(step)) => format!("{video} · {step}"),
        (Some(video), None) => video.to_string(),
        (None, Some(step)) => step,
        (None, None) => String::new(),
    }
}

/// The whole line, as Copy writes it: time, level, writer, source, where, then the message.
pub(crate) fn line_text(line: &LogLine) -> String {
    let place = header(line.video.as_deref(), line.step.as_deref());
    let place = if place.is_empty() {
        String::new()
    } else {
        format!("[{place}] ")
    };
    format!(
        "{}  {:<5}  {:<7}  {}  {place}{}",
        time(line.elapsed),
        line.level.as_str(),
        Who::of(&line.target).label(),
        line.target,
        line.message
    )
}

/// Every line the list shows, one per text line.
pub(crate) fn copy_text(activity: &Activity) -> String {
    let mut text = String::new();
    for line in activity.shown() {
        let _ = writeln!(text, "{}", line_text(line));
    }
    text
}

#[cfg(test)]
#[path = "tests/console_text.rs"]
mod tests;
