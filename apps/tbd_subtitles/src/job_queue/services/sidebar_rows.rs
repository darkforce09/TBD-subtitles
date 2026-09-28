//! The sidebar's rows, built from the queue: one row per video, in the sections Now, Up Next and
//! Done.
//!
//! **Role:** fold each correction run into its video's row, give each row its section, its place
//! in line, and whether it can be removed or dragged.
//!
//! **Position:** read by the sidebar each frame (and through it by the application's shortcuts,
//! for the order of the rows), and by `queue_editing` when a row leaves the list.
//!
//! **Signals and state:** none; reads the queue.
//!
//! **Invariants:** every job is on exactly one row, as the row's job or folded into it; a
//! correction run that failed or was cancelled keeps a row of its own, so it can be tried again;
//! rows are in section order, and in queue order within a section, where ended jobs stand
//! newest first.

use std::collections::HashMap;

use crate::core::format;
use crate::job_queue::models::queue::{JobId, JobKind, JobState, Queue, QueueItem};
use crate::job_queue::models::sidebar::{ReviewFold, Section, SidebarRow};

/// The rows of the sidebar: the running jobs, then the waiting ones, then the ended ones.
pub(crate) fn rows(queue: &Queue) -> Vec<SidebarRow> {
    let mut folded: HashMap<JobId, Vec<&QueueItem>> = HashMap::new();
    let mut own = Vec::new();
    for item in &queue.items {
        match host(queue, item) {
            Some(host) => folded.entry(host).or_default().push(item),
            None => own.push(item),
        }
    }
    let mut rows: Vec<SidebarRow> = own
        .into_iter()
        .map(|item| row(item, folded.get(&item.id).map_or(&[][..], Vec::as_slice)))
        .collect();
    rows.sort_by_key(|row| Section::ALL.iter().position(|s| *s == row.section));
    let mut place = 0;
    for row in &mut rows {
        let full = queue
            .get(row.id)
            .is_some_and(|item| item.kind == JobKind::Full);
        if row.section == Section::UpNext && full {
            place += 1;
            row.place = Some(place);
        }
    }
    rows
}

/// The full run whose row correction run `item` folds into: the newest finished full run of its
/// video (the first in the queue, where ended jobs stand newest first), else its last full run.
/// None for a full run, for a correction run with no full run of its video, and for one that
/// failed or was cancelled.
fn host(queue: &Queue, item: &QueueItem) -> Option<JobId> {
    let ended = matches!(item.state, JobState::Failed(_) | JobState::Cancelled { .. });
    if item.kind != JobKind::Review || ended {
        return None;
    }
    let full = || {
        queue
            .items
            .iter()
            .filter(|other| other.kind == JobKind::Full && other.video == item.video)
    };
    full()
        .find(|other| finished(&other.state))
        .or_else(|| full().next_back())
        .map(|other| other.id)
}

fn finished(state: &JobState) -> bool {
    matches!(state, JobState::Finished(_) | JobState::FinishedBefore)
}

/// The row of `item`, with the correction runs `folded` into it.
fn row(item: &QueueItem, folded: &[&QueueItem]) -> SidebarRow {
    let section = match item.state {
        JobState::Running(_) => Section::Now,
        JobState::Waiting => Section::UpNext,
        _ => Section::Done,
    };
    let name = match (item.kind, item.corrections) {
        (JobKind::Full, _) => item.name(),
        // A correction run read from a queue file with no count says "corrections" alone.
        (JobKind::Review, 0) => format!("{} · corrections", item.name()),
        (JobKind::Review, n) => format!("{} · {}", item.name(), format::plural(n, "correction")),
    };
    let pending: Vec<&&QueueItem> = folded
        .iter()
        .filter(|run| run.state.is_waiting() || run.state.is_running())
        .collect();
    let fold = (!pending.is_empty()).then(|| ReviewFold {
        corrections: pending.iter().map(|run| run.corrections.max(1)).sum(),
        running: pending
            .iter()
            .find(|run| run.state.is_running())
            .map(|run| run.id),
    });
    SidebarRow {
        id: item.id,
        name,
        video: item.video.clone(),
        section,
        folded: folded.iter().map(|run| run.id).collect(),
        fold,
        place: None,
        removable: !item.state.is_running() && !pending.iter().any(|run| run.state.is_running()),
        draggable: item.state.is_waiting() && item.kind == JobKind::Full,
    }
}

#[cfg(test)]
#[path = "tests/sidebar_rows.rs"]
mod tests;
