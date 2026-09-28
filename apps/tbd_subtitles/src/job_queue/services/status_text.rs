//! The line under a row's name: where its video stands, in the owner's words.
//!
//! **Role:** write the status line of a sidebar row from its job's state: "Settling the words ·
//! about 4 min left", "Waiting · 2nd in line", "Failed at Hear the speech", "Updating subtitles ·
//! 2 corrections".
//!
//! **Position:** called by the sidebar row for each row it draws.
//!
//! **Signals and state:** none; reads the row, its job, the queue and the step rates.
//!
//! **Invariants:** a place in line past the first shows only while the queue runs, since a paused
//! queue starts nothing after the next video.

use std::time::Instant;

use crate::core::format;
use crate::core::steps::stage_of;
use crate::job_queue::models::progress::{JobProgress, Rates};
use crate::job_queue::models::queue::{JobState, Queue, QueueItem};
use crate::job_queue::models::sidebar::SidebarRow;
use crate::job_queue::services::time_left;

/// The status line of `row`, whose job is `item`.
pub(crate) fn status(
    row: &SidebarRow,
    item: &QueueItem,
    queue: &Queue,
    rates: &Rates,
    now: Instant,
) -> String {
    match &item.state {
        JobState::Running(progress) => running(progress, rates, now),
        JobState::Waiting => match row.place {
            Some(1) => "Waiting · next in line".to_string(),
            Some(place) if queue.running => {
                format!("Waiting · {} in line", format::ordinal(place))
            }
            _ => "Waiting".to_string(),
        },
        JobState::Failed(failure) => match failure.step {
            Some(step) => format!("Failed at {}", stage_of(step).title),
            None => "Failed".to_string(),
        },
        JobState::Cancelled { kept_steps } => format!(
            "Cancelled · {} kept",
            format::plural(*kept_steps, "finished step")
        ),
        JobState::Finished(_) | JobState::FinishedBefore => match (row.fold, &item.state) {
            (Some(fold), _) => format!(
                "Updating subtitles · {}",
                format::plural(fold.corrections, "correction")
            ),
            (None, JobState::Finished(result)) if !result.failures.is_empty() => format!(
                "Needs attention · {}",
                format::plural(result.failures.len(), "problem")
            ),
            _ => "Subtitles ready".to_string(),
        },
    }
}

/// A running job: what its stage is doing and the time left, or that it is stopping.
fn running(progress: &JobProgress, rates: &Rates, now: Instant) -> String {
    if progress.cancelling {
        return "Stopping…".to_string();
    }
    let Some(step) = progress.current_step() else {
        return "Starting…".to_string();
    };
    let doing = stage_of(step).doing;
    match time_left::estimate(progress, rates, now) {
        Some((left, _)) => format!("{doing} · {} left", format::about(left)),
        None => doing.to_string(),
    }
}

#[cfg(test)]
#[path = "tests/status_text.rs"]
mod tests;
