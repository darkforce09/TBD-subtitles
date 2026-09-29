//! The queue logic: editing the queue, the sidebar's rows and their status lines, running its
//! jobs, following their progress, its stages, estimating the time left, keeping the queue across
//! windows, finding videos in folders and watching the watch folders, remembering every video
//! queued, and the notice when a job ends.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the application's automation starts it")
)]
pub(crate) mod folder_watcher;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the application's automation calls it")
)]
pub(crate) mod job_notice;
pub(crate) mod job_runner;
pub(crate) mod progress_log;
pub(crate) mod progress_tracking;
pub(crate) mod queue_editing;
pub(crate) mod queue_store;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the application's automation calls it")
)]
pub(crate) mod queued_history;
pub(crate) mod review_lanes;
pub(crate) mod sidebar_rows;
pub(crate) mod stage_progress;
pub(crate) mod status_text;
pub(crate) mod time_left;
pub(crate) mod video_files;
pub(crate) mod watch_scan;
