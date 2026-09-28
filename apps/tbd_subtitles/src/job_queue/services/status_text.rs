//! Where a video stands, in the owner's words: the line under a row's name, the line under the
//! detail pane's title, and when a waiting or retried job starts.
//!
//! **Role:** write the status line of a sidebar row from its job's state ("Settling the words ·
//! about 4 min left", "Waiting · 2nd in line", "Failed at Hear the speech", "Updating subtitles ·
//! 2 corrections", "Subtitles ready · 38 to check", "Needs attention · 1 problem"); the detail
//! pane's line of a job that has not finished ("25:59 video · running for 10 min 00 s"); and when
//! a waiting job, or an ended one tried again, starts.
//!
//! **Position:** called by the sidebar row for each row it draws, and by the queue's job cards
//! and the application's detail header for the selected job.
//!
//! **Signals and state:** none; reads the row, its job, the queue, the step rates and a finished
//! job's summary (`job_report::models::summary`).
//!
//! **Invariants:** a place in line past the first shows only while the queue runs, since a paused
//! queue starts nothing after the next video; a job tried again is said to start at once only
//! when its lane is idle, its video runs nothing else and every model is on disk; a finished
//! job's verdict comes from its files when they were read, so a row finished in an earlier window
//! shows its real verdict.

use std::time::Instant;

use crate::core::format;
use crate::core::steps::stage_of;
use crate::job_queue::models::progress::{JobProgress, Rates};
use crate::job_queue::models::queue::{Failure, JobKind, JobState, Queue, QueueItem};
use crate::job_queue::models::sidebar::SidebarRow;
use crate::job_queue::services::time_left;
use crate::job_report::models::summary::RowSummary;

/// The status line of `row`, whose job is `item`; a finished job's `summary`, when its files
/// were read, gives its verdict and its lines to check.
pub(crate) fn status(
    row: &SidebarRow,
    item: &QueueItem,
    queue: &Queue,
    rates: &Rates,
    summary: Option<&RowSummary>,
    now: Instant,
) -> String {
    match &item.state {
        JobState::Running(progress) => running(progress, rates, now),
        JobState::Waiting => format!("Waiting{}", place_words(row.place, queue)),
        JobState::Failed(failure) => failed(failure),
        JobState::Cancelled { kept_steps } => cancelled(*kept_steps),
        JobState::Finished(_) | JobState::FinishedBefore => {
            match (row.fold, summary, &item.state) {
                (Some(fold), _, _) => format!(
                    "Updating subtitles · {}",
                    format::plural(fold.corrections, "correction")
                ),
                (None, Some(summary), _) if !summary.passes() => needs_attention(summary.problems),
                (None, Some(summary), _) if summary.to_check > 0 => {
                    format!("Subtitles ready · {} to check", summary.to_check)
                }
                (None, Some(summary), _) if summary.flagged > 0 => {
                    "Subtitles ready · all checked".to_string()
                }
                (None, None, JobState::Finished(result)) if !result.failures.is_empty() => {
                    needs_attention(result.failures.len())
                }
                _ => "Subtitles ready".to_string(),
            }
        }
    }
}

/// Whether finished job `item` fails the quality check: by its `summary` when its files were
/// read, else by the run's own result.
pub(crate) fn fails_the_check(item: &QueueItem, summary: Option<&RowSummary>) -> bool {
    match (summary, &item.state) {
        (Some(summary), _) => !summary.passes(),
        (None, JobState::Finished(result)) => !result.failures.is_empty(),
        _ => false,
    }
}

/// "Needs attention · 2 problems".
fn needs_attention(problems: usize) -> String {
    format!("Needs attention · {}", format::plural(problems, "problem"))
}

/// The line under the detail pane's title for `item`: a running job's length and time so far, a
/// waiting job's place, where a failed job failed, what a cancelled one kept; "Subtitles ready"
/// for a finished job, whose line the report gives.
pub(crate) fn detail_line(item: &QueueItem, queue: &Queue, now: Instant) -> String {
    match &item.state {
        JobState::Running(progress) => {
            let so_far = format::duration(
                now.saturating_duration_since(progress.started)
                    .as_secs_f64(),
            );
            match progress.duration_s {
                Some(length) => {
                    format!("{} video · running for {so_far}", format::length(length))
                }
                None => format!("Running for {so_far}"),
            }
        }
        JobState::Waiting => {
            let place = place_words(Some(place_in_line(queue, item)), queue);
            let place = place.strip_prefix(" · ").unwrap_or("Waiting");
            format!("Length known once it starts · {place}")
        }
        JobState::Failed(failure) => failed(failure),
        JobState::Cancelled { kept_steps } => cancelled(*kept_steps),
        JobState::Finished(_) | JobState::FinishedBefore => "Subtitles ready".to_string(),
    }
}

/// A waiting job's place among the waiting runs of its kind, from 1.
pub(crate) fn place_in_line(queue: &Queue, item: &QueueItem) -> usize {
    queue
        .items
        .iter()
        .filter(|other| other.kind == item.kind && other.state.is_waiting())
        .position(|other| other.id == item.id)
        .map_or(1, |at| at + 1)
}

/// When waiting job `item`, `place`th in line, starts: a correction run as soon as its video is
/// free, a full run by the queue.
pub(crate) fn waiting_start(
    queue: &Queue,
    item: &QueueItem,
    place: usize,
    models_missing: bool,
) -> &'static str {
    if models_missing {
        "It can start once the models are on disk."
    } else if item.kind == JobKind::Review {
        "It starts as soon as the video is free."
    } else if !queue.running {
        "Press Start Queue to begin."
    } else if place == 1 {
        "It starts when the current video finishes."
    } else {
        "It starts when the videos before it finish."
    }
}

/// When ended job `item` starts if it is tried again now: at once when its lane is idle and its
/// video runs nothing else, else next while the queue runs (a correction run always), else when
/// Start Queue is pressed; never before the models are on disk.
pub(crate) fn try_again_start(
    queue: &Queue,
    item: &QueueItem,
    models_missing: bool,
) -> &'static str {
    if models_missing {
        return "It waits until the models are on disk.";
    }
    let busy = queue.items.iter().any(|other| {
        other.id != item.id
            && other.state.is_running()
            && (other.kind == item.kind || other.video == item.video)
    });
    if !busy {
        "It starts at once."
    } else if item.kind == JobKind::Review || queue.running {
        "It runs next, when the current video finishes."
    } else {
        "It goes first in line and waits for Start Queue."
    }
}

/// " · next in line" for the first in line, " · 3rd in line" while the queue runs, else nothing.
fn place_words(place: Option<usize>, queue: &Queue) -> String {
    match place {
        Some(1) => " · next in line".to_string(),
        Some(place) if queue.running => format!(" · {} in line", format::ordinal(place)),
        _ => String::new(),
    }
}

/// "Failed at Hear the speech", or "Failed" before the first step.
fn failed(failure: &Failure) -> String {
    match failure.step {
        Some(step) => format!("Failed at {}", stage_of(step).title),
        None => "Failed".to_string(),
    }
}

/// "Cancelled · 9 finished steps kept".
fn cancelled(kept_steps: usize) -> String {
    format!(
        "Cancelled · {} kept",
        format::plural(kept_steps, "finished step")
    )
}

/// A running job: what its stage is doing and the time left, or that it is stopping.
fn running(progress: &JobProgress, rates: &Rates, now: Instant) -> String {
    if progress.cancelling {
        return "Stopping…".to_string();
    }
    let Some(step) = progress.shown_step() else {
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
