//! The desktop notice for a full run that has just ended: its subtitles are ready, with the
//! quality check's verdict, or it failed, and where.
//!
//! **Role:** word the title and body of the notice for a job that finished or failed.
//!
//! **Position:** called by the application when a job ends; the application shows the notice.
//!
//! **Signals and state:** none; reads the job it is given.
//!
//! **Invariants:** only a full run that finished or failed in this window gives a notice; a body
//! is at most `BODY_LIMIT` characters.

use crate::core::format;
use crate::core::steps::step_title;
use crate::job_queue::models::queue::{JobKind, JobResult, JobState, QueueItem};

/// The most characters a notice's body holds; a longer one ends in "…".
const BODY_LIMIT: usize = 200;

/// A desktop notice's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) title: String,
    pub(crate) body: String,
}

/// The notice for `item`, a job that has just ended: none for a correction run, or a job that
/// waits, runs, was cancelled or finished in an earlier window.
pub(crate) fn ended_notice(item: &QueueItem) -> Option<Notice> {
    if item.kind != JobKind::Full {
        return None;
    }
    let name = item.name();
    match &item.state {
        JobState::Finished(result) => Some(Notice {
            title: format!("Subtitles ready: {name}"),
            body: quality_check(result),
        }),
        JobState::Failed(failure) => {
            let body = match failure.step {
                Some(step) => format!("At {}: {}", step_title(step), failure.message),
                None => failure.message.clone(),
            };
            Some(Notice {
                title: format!("{name} failed"),
                body: capped(&body),
            })
        }
        JobState::Waiting
        | JobState::Running(_)
        | JobState::FinishedBefore
        | JobState::Cancelled { .. } => None,
    }
}

/// "The quality check passed.", or "Quality check: 2 layout rule(s) broken; 12 lines to check."
fn quality_check(result: &JobResult) -> String {
    if result.failures.is_empty() {
        return "The quality check passed.".to_string();
    }
    let problems = result.failures.join(", ");
    let body = match result.findings {
        0 => format!("Quality check: {problems}."),
        n => format!(
            "Quality check: {problems}; {} to check.",
            format::plural(n, "line")
        ),
    };
    capped(&body)
}

/// `text` cut to `BODY_LIMIT` characters, its last one "…" when cut.
fn capped(text: &str) -> String {
    if text.chars().count() <= BODY_LIMIT {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(BODY_LIMIT - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
#[path = "tests/job_notice.rs"]
mod tests;
