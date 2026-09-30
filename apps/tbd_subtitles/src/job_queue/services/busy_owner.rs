//! Busy jobs: whether the process that owns a job's database still runs, and putting a job back
//! in line once it does not.
//!
//! **Role:** a job that another process runs (a `tbd-subtitles process` from a terminal) ends
//! busy; this finds, at each check, the busy jobs whose owner has gone (its `/proc/<pid>` folder
//! is gone, or no owner was named and [`UNKNOWN_OWNER_WAIT`] has passed) and sets them waiting.
//!
//! **Position:** called by the application before each frame; the window asks for a frame every
//! [`CHECK_EVERY`] while a job is busy.
//!
//! **Signals and state:** reads `/proc`; changes only the queue it is given.
//!
//! **Invariants:** a busy job goes back to waiting and nowhere else, keeping its place; a job
//! whose owner still runs stays busy.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::job_queue::models::queue::{JobState, Queue};

/// How often the window checks the busy jobs' owners.
pub(crate) const CHECK_EVERY: Duration = Duration::from_secs(1);

/// How long a busy job whose owner is not named waits before it tries again.
pub(crate) const UNKNOWN_OWNER_WAIT: Duration = Duration::from_secs(5);

/// Whether process `pid` runs, by its folder under `/proc`.
pub(crate) fn process_runs(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Whether a job busy since `since` under `owner` may try again at `now`.
pub(crate) fn owner_gone(owner: Option<u32>, since: Instant, now: Instant) -> bool {
    match owner {
        Some(pid) => !process_runs(pid),
        None => now.saturating_duration_since(since) >= UNKNOWN_OWNER_WAIT,
    }
}

/// Whether any job of `queue` is busy.
pub(crate) fn any_busy(queue: &Queue) -> bool {
    queue.items.iter().any(|item| item.state.is_busy())
}

/// Set every busy job whose owner is gone at `now` waiting again; whether any was.
pub(crate) fn release_freed(queue: &mut Queue, now: Instant) -> bool {
    let mut released = false;
    for item in &mut queue.items {
        if let JobState::Busy { owner, since } = item.state
            && owner_gone(owner, since, now)
        {
            item.state = JobState::Waiting;
            released = true;
        }
    }
    released
}

#[cfg(test)]
#[path = "tests/busy_owner.rs"]
mod tests;
