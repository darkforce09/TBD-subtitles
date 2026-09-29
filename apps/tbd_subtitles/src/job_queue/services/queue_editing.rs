//! Changing the queue: adding videos, from files or folders, taking rows out and putting them
//! back, moving waiting jobs, trying ended jobs again, and choosing the next to run and the
//! queue's one control.
//!
//! **Role:** every edit of the queue the owner can make, the choice of the next job, and which
//! of Start Queue, Pause After This Video and Resume Queue the toolbar offers.
//!
//! **Position:** called by the application's queue actions and by `queue_store` when it rebuilds
//! a queue; the toolbar reads `queue_control`.
//!
//! **Signals and state:** reads a folder's listing when a folder is added (`video_files`);
//! otherwise changes only the queue it is given.
//!
//! **Invariants:** a running job is never removed, moved or tried again; a video is never queued
//! twice while it waits or runs, by adding, restoring or trying again; only waiting full runs
//! move, among themselves; ended jobs stand newest first; the queue's control counts full runs
//! only, since correction runs start by themselves.

use std::path::{Path, PathBuf};

use job_model::StepName;

use crate::job_queue::models::queue::{JobId, JobKind, JobState, Move, Queue, QueueItem, Removed};
use crate::job_queue::services::sidebar_rows;
use crate::job_queue::services::video_files::videos_in_folder;

/// Why a job could not go back into the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// Another run of the same kind of its video waits or runs already.
    AlreadyQueued,
    /// It waits or runs itself, or is not in the queue.
    NotEnded,
}

/// The one queue button the toolbar shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QueueControl {
    /// Start Queue, disabled for the reason given when there is one.
    Start { reason: Option<&'static str> },
    /// Pause After This Video: the queue runs.
    PauseAfter,
    /// Resume Queue: the queue stops after the running video, as the reason says.
    Resume { reason: String },
}

/// Queue each video not already waiting or running, keeping the order given; a folder stands for
/// its videos (`videos_in_folder`). Returns how many were added.
pub(crate) fn add_videos(queue: &mut Queue, videos: impl IntoIterator<Item = PathBuf>) -> usize {
    let mut added = 0;
    for path in videos {
        let expanded = if path.is_dir() {
            videos_in_folder(&path)
        } else {
            vec![path]
        };
        for video in expanded {
            let queued = queue.items.iter().any(|item| {
                item.video == video && (item.state.is_waiting() || item.state.is_running())
            });
            if video.as_os_str().is_empty() || queued {
                continue;
            }
            push(queue, video, JobKind::Full);
            added += 1;
        }
    }
    added
}

/// Queue a run of `kind` for `video` at the end, and return its id.
pub(crate) fn push(queue: &mut Queue, video: PathBuf, kind: JobKind) -> JobId {
    let id = queue.next_id;
    queue.next_id += 1;
    queue.items.push(QueueItem {
        id,
        video,
        kind,
        state: JobState::Waiting,
        keep_settings: false,
        rerun: Vec::new(),
        corrections: 0,
    });
    id
}

/// Take row `id` out of the list with the correction runs folded into it, unless something on
/// it runs; a selected row passes the selection to the row now in its place, or the one before.
pub(crate) fn remove(queue: &mut Queue, id: JobId) -> Option<Removed> {
    let rows = sidebar_rows::rows(queue);
    let at = rows.iter().position(|row| row.id == id)?;
    let row = &rows[at];
    if !row.removable {
        return None;
    }
    let mut taken: Vec<(usize, JobId)> = std::iter::once(row.id)
        .chain(row.folded.iter().copied())
        .filter_map(|job| {
            let index = queue.items.iter().position(|item| item.id == job)?;
            Some((index, job))
        })
        .collect();
    taken.sort_unstable();
    let mut items: Vec<(usize, QueueItem)> = taken
        .iter()
        .rev()
        .map(|&(index, _)| (index, queue.items.remove(index)))
        .collect();
    items.sort_by_key(|(_, item)| (item.id != id, item.id));
    if queue.selected == Some(id) {
        let rows = sidebar_rows::rows(queue);
        queue.selected = rows
            .get(at)
            .or_else(|| at.checked_sub(1).and_then(|before| rows.get(before)))
            .map(|row| row.id);
    }
    Some(Removed { items })
}

/// Put a removed row back where it was, and select it; returns its job. Refused when a run of
/// the same kind of its video waits or runs in the queue meanwhile.
pub(crate) fn restore(queue: &mut Queue, removed: Removed) -> Result<JobId, Refusal> {
    let Some(job) = removed.job() else {
        return Err(Refusal::NotEnded);
    };
    if queued_elsewhere(queue, job.id, &job.video, job.kind) {
        return Err(Refusal::AlreadyQueued);
    }
    let id = job.id;
    let mut items = removed.items;
    items.sort_by_key(|(index, _)| *index);
    for (index, item) in items {
        let at = index.min(queue.items.len());
        queue.items.insert(at, item);
    }
    queue.selected = Some(id);
    Ok(id)
}

/// Whether a job other than `id` runs `video` as a `kind` run, or waits to.
fn queued_elsewhere(queue: &Queue, id: JobId, video: &Path, kind: JobKind) -> bool {
    queue.items.iter().any(|item| {
        item.id != id
            && item.kind == kind
            && item.video == video
            && (item.state.is_waiting() || item.state.is_running())
    })
}

fn waiting_full(item: &QueueItem) -> bool {
    item.state.is_waiting() && item.kind == JobKind::Full
}

/// Move waiting full run `id` among the waiting full runs; other jobs keep their places.
pub(crate) fn move_job(queue: &mut Queue, id: JobId, to: Move) {
    let waiting: Vec<JobId> = queue
        .items
        .iter()
        .filter(|item| waiting_full(item))
        .map(|item| item.id)
        .collect();
    let Some(rank) = waiting.iter().position(|&job| job == id) else {
        return;
    };
    let before = match to {
        Move::Up if rank > 0 => Some(waiting[rank - 1]),
        Move::Top if rank > 0 => Some(waiting[0]),
        Move::Down if rank + 2 < waiting.len() => Some(waiting[rank + 2]),
        Move::Down if rank + 1 < waiting.len() => None,
        _ => return,
    };
    move_before(queue, id, before);
}

/// Move waiting full run `id` to just before waiting full run `before`, or after the last one
/// when `before` is none; other jobs keep their places. Returns whether it moved.
pub(crate) fn move_before(queue: &mut Queue, id: JobId, before: Option<JobId>) -> bool {
    if before == Some(id) || before.is_some_and(|b| !queue.get(b).is_some_and(waiting_full)) {
        return false;
    }
    let Some(from) = queue
        .items
        .iter()
        .position(|item| item.id == id && waiting_full(item))
    else {
        return false;
    };
    let item = queue.items.remove(from);
    let to = match before {
        Some(before) => queue.items.iter().position(|other| other.id == before),
        None => queue
            .items
            .iter()
            .rposition(waiting_full)
            .map(|last| last + 1),
    };
    queue
        .items
        .insert(to.unwrap_or(from).min(queue.items.len()), item);
    to.is_some_and(|to| to != from)
}

/// Put ended job `id` back to waiting, first in line among the waiting runs of its kind, with
/// `rerun` and the steps after it to run again when one is named; it resumes after the steps it
/// kept, with its own settings. Refused when it waits or runs already, or another run of the same
/// kind of its video does.
pub(crate) fn try_again(
    queue: &mut Queue,
    id: JobId,
    rerun: Option<StepName>,
) -> Result<(), Refusal> {
    let Some(item) = queue.get(id) else {
        return Err(Refusal::NotEnded);
    };
    if item.state.is_waiting() || item.state.is_running() {
        return Err(Refusal::NotEnded);
    }
    if queued_elsewhere(queue, id, &item.video, item.kind) {
        return Err(Refusal::AlreadyQueued);
    }
    let Some(item) = queue.get_mut(id) else {
        return Err(Refusal::NotEnded);
    };
    item.state = JobState::Waiting;
    if let Some(step) = rerun
        && !item.rerun.contains(&step)
    {
        item.rerun.push(step);
    }
    first_in_line(queue, id);
    Ok(())
}

/// Put ended job `id` back to waiting, first in line, to run with the settings saved now: only
/// the steps those settings change run again. Refused as `try_again` is.
pub(crate) fn run_again(queue: &mut Queue, id: JobId) -> Result<(), Refusal> {
    try_again(queue, id, None)?;
    if let Some(item) = queue.get_mut(id) {
        item.keep_settings = false;
    }
    Ok(())
}

/// Move ended job `id` before every other ended job, so the Done section lists the newest
/// first.
pub(crate) fn newest_ended_first(queue: &mut Queue, id: JobId) {
    let ended = |item: &QueueItem| !item.state.is_waiting() && !item.state.is_running();
    let Some(from) = queue
        .items
        .iter()
        .position(|item| item.id == id && ended(item))
    else {
        return;
    };
    let item = queue.items.remove(from);
    let first = queue.items.iter().position(ended).unwrap_or(from);
    queue.items.insert(first.min(from), item);
}

/// Move waiting job `id` before the first other waiting job of its kind.
fn first_in_line(queue: &mut Queue, id: JobId) {
    let Some(from) = queue.items.iter().position(|item| item.id == id) else {
        return;
    };
    let item = queue.items.remove(from);
    let first = queue
        .items
        .iter()
        .position(|other| other.state.is_waiting() && other.kind == item.kind);
    queue.items.insert(first.unwrap_or(from), item);
}

/// The queue button the toolbar shows: Pause After This Video while the queue runs, Resume Queue
/// once paused with a full run still running (both whether or not a model is missing), else
/// Start Queue, disabled with the reason when a model is missing or no full run waits.
pub(crate) fn queue_control(queue: &Queue, models_missing: bool) -> QueueControl {
    if queue.running {
        return QueueControl::PauseAfter;
    }
    let running = queue
        .items
        .iter()
        .find(|item| item.kind == JobKind::Full && item.state.is_running());
    if let Some(job) = running
        && queue.pausing
    {
        return QueueControl::Resume {
            reason: format!("Pauses after {}", job.short_name()),
        };
    }
    if models_missing {
        return QueueControl::Start {
            reason: Some("Download the models first"),
        };
    }
    let reason = if queue.items.iter().any(waiting_full) {
        None
    } else if queue.items.is_empty() {
        Some("Add videos to start")
    } else {
        Some("Nothing is waiting")
    };
    QueueControl::Start { reason }
}

/// The first waiting job of `kind` that `startable` lets start.
pub(crate) fn first_startable(
    queue: &Queue,
    kind: JobKind,
    startable: impl Fn(JobId) -> bool,
) -> Option<JobId> {
    queue
        .items
        .iter()
        .find(|item| item.state.is_waiting() && item.kind == kind && startable(item.id))
        .map(|item| item.id)
}

/// The first waiting job of `kind`, which runs next in its lane: full runs one after another
/// while the queue runs, review runs as soon as they are queued.
pub(crate) fn next_waiting(queue: &Queue, kind: JobKind) -> Option<JobId> {
    queue
        .items
        .iter()
        .find(|item| item.state.is_waiting() && item.kind == kind)
        .map(|item| item.id)
}

/// Queue a review run of `video` carrying `corrections` more corrections, unless one already
/// waits, which then carries them; returns the waiting run's id.
pub(crate) fn queue_review(queue: &mut Queue, video: PathBuf, corrections: usize) -> JobId {
    let waiting = queue.items.iter().position(|item| {
        item.kind == JobKind::Review && item.video == video && item.state.is_waiting()
    });
    let at = match waiting {
        Some(at) => at,
        None => {
            push(queue, video, JobKind::Review);
            queue.items.len() - 1
        }
    };
    let item = &mut queue.items[at];
    item.corrections += corrections;
    item.id
}

#[cfg(test)]
#[path = "tests/queue_editing.rs"]
mod tests;
