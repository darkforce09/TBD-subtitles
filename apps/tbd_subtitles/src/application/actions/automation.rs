//! What the window does without being asked: videos handed over by later starts, videos found in
//! the watch folders, the queue started for them, the desktop told when a job ends while the
//! window is away, and Dolphin's "Generate subtitles" entry written as the window opens.
//!
//! **Role:** queue every video from any source and remember it in the queued history; take each
//! hand-off (queue, start, bring the window forward); queue each watch-folder video that was
//! never queued, has no subtitles and is not in the queue; start the queue for them unless the
//! owner paused it; start the queue once the models it waits for are on disk; notify the desktop
//! of each full run that ended while the window is away; write the service menu and say how that
//! went.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply` for `QueueVideos`, by
//! `application::launch`, by `poll` before each frame and by `runner::poll_runner`; uses
//! `core::single_instance`, `core::service_menu` and `job_queue::services`.
//!
//! **Signals and state:** the hand-off receiver, the folder watcher, the queued history (written
//! to `queued_videos.json` whenever it grows), whether the owner paused the queue, and whether the
//! window is to be minimized or brought forward; notifies the desktop through the environment.
//!
//! **Invariants:** automation never starts the queue after the owner pressed Pause in this window
//! until the owner presses Start; a watch folder never queues a video queued before, one with a
//! subtitle file, or one that waits or runs; the watcher always watches the saved watch folders;
//! the desktop is notified only while the window is away, and only for a full run that finished
//! or failed.

use std::io;
use std::path::{Path, PathBuf};

use crate::application::{HandOffs, TbdSubtitlesApp};
use crate::core::format;
use crate::core::service_menu::{self, Installed};
use crate::core::single_instance::HandOff;
use crate::core::toast::ToastKind;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobKind, Queue};
use crate::job_queue::services::job_notice::Notice;
use crate::job_queue::services::queued_history::{self, QueuedHistory};
use crate::job_queue::services::{queue_editing, video_files};
use crate::settings::models::page::RightClickEntry;

/// The notice's body when videos are queued but a model they need is missing.
const MODELS_MISSING: &str = "Download the missing models in Settings to start them.";

impl TbdSubtitlesApp {
    /// Take on how the window was launched: the later starts' hand-offs, whose wake is the
    /// window's; whether the window minimizes at its first frame; whether the queue starts.
    pub(crate) fn automate(&mut self, start: bool, minimized: bool, hand_offs: Option<HandOffs>) {
        if let Some((receiver, wake)) = hand_offs {
            if wake.set(self.env.wake.clone()).is_err() {
                tracing::debug!("the hand-offs already wake the window");
            }
            self.hand_offs = Some(receiver);
        }
        self.minimize_once = minimized;
        if start {
            let waiting = self
                .queue
                .items
                .iter()
                .filter(|item| item.kind == JobKind::Full && item.state.is_waiting())
                .count();
            self.auto_start(waiting);
        }
    }

    /// Queue `videos` (a folder as its videos) and remember each video queued; how many were.
    pub(crate) fn queue_videos(&mut self, videos: Vec<PathBuf>) -> usize {
        let first = self.queue.next_id;
        let added = queue_editing::add_videos(&mut self.queue, videos);
        if added == 0 {
            return 0;
        }
        tracing::info!(added, "videos queued");
        self.save_queue();
        let queued = self
            .queue
            .items
            .iter()
            .filter(|item| item.id >= first)
            .map(|item| item.video.clone());
        if self.history.record(queued) {
            save_history(&self.env.history_path, &self.history);
        }
        added
    }

    /// Fold in what later starts and the watch folders sent, keep the watcher on the saved
    /// watch folders, and start the queue that waited for the models once they are on disk.
    pub(crate) fn poll_automation(&mut self) {
        let saved = &self.settings.saved.watch_folders;
        if self.watcher.folders() != saved.as_slice() {
            tracing::info!(folders = saved.len(), "watching the saved watch folders");
            self.watcher.set_folders(saved);
        }
        let mut hand_offs = Vec::new();
        if let Some(receiver) = &self.hand_offs {
            hand_offs.extend(receiver.try_iter());
        }
        for hand_off in hand_offs {
            self.take_hand_off(hand_off);
        }
        let found: Vec<PathBuf> = self.watcher.found.try_iter().flatten().collect();
        if !found.is_empty() {
            self.queue_found(found);
        }
        let waits = self.queue.running
            && self.cancel.is_none()
            && queue_editing::next_waiting(&self.queue, JobKind::Full).is_some();
        if waits && !self.models_missing() {
            self.start_next();
        }
    }

    /// Queue a later start's videos, start the queue when it asks, and bring the window forward
    /// when it asks.
    pub(crate) fn take_hand_off(&mut self, hand_off: HandOff) {
        let HandOff {
            videos,
            start,
            raise,
        } = hand_off;
        tracing::info!(
            videos = videos.len(),
            start,
            raise,
            "a later start handed over"
        );
        let added = self.queue_videos(videos);
        if added > 0 {
            let text = format!("Added {} to the queue.", format::plural(added, "video"));
            self.toast(ToastKind::Info, text);
        }
        if start {
            self.auto_start(added);
        }
        if raise {
            self.raise = true;
        }
    }

    /// Queue the watch folders' `found` videos that were never queued, have no subtitle file and
    /// do not wait or run, and start the queue for them.
    pub(crate) fn queue_found(&mut self, found: Vec<PathBuf>) {
        let fresh: Vec<PathBuf> = found
            .into_iter()
            .filter(|video| {
                !self.history.contains(video)
                    && video_files::subtitle_file(video).is_none()
                    && !in_line(&self.queue, video)
            })
            .collect();
        if fresh.is_empty() {
            return;
        }
        let added = self.queue_videos(fresh);
        if added > 0 {
            let text = format!(
                "Added {} from the watch folders.",
                format::plural(added, "video")
            );
            self.toast(ToastKind::Info, text);
            self.auto_start(added);
        }
    }

    /// Start the queue as Start Queue does, unless the owner paused it in this window; when a
    /// model is missing and the window is away, tell the desktop the `added` videos wait for it.
    fn auto_start(&mut self, added: usize) {
        if self.paused_by_owner {
            tracing::info!("the queue stays paused until Start Queue is pressed");
            return;
        }
        self.apply_queue(JobQueueEvent::Start);
        let away = self.presence.away || self.minimize_once;
        if self.models_missing() && added > 0 && away {
            let title = format!("{} queued", format::plural(added, "video"));
            (self.env.notify)(&title, MODELS_MISSING);
            self.attention = true;
        }
    }

    /// Tell the desktop about each full run that ended, while the window is away.
    pub(crate) fn notify_ended(&mut self, notices: Vec<Notice>) {
        if !self.presence.away || notices.is_empty() {
            return;
        }
        for notice in notices {
            (self.env.notify)(&notice.title, &notice.body);
        }
        self.attention = true;
    }
}

/// Whether a run of `video` waits, runs or is busy in `queue`.
fn in_line(queue: &Queue, video: &Path) -> bool {
    queue
        .items
        .iter()
        .any(|item| item.video == video && item.state.is_queued())
}

/// The queued history kept in `path`, with every video of `queue` recorded in it.
pub(crate) fn seeded_history(path: &Path, queue: &Queue) -> QueuedHistory {
    let mut history = queued_history::load(path);
    if history.record_queue(queue) {
        save_history(path, &history);
    }
    history
}

/// Write `history` to `path`, logging a failure.
fn save_history(path: &Path, history: &QueuedHistory) {
    if let Err(error) = queued_history::save(path, history) {
        tracing::warn!(
            error = format!("{error:#}"),
            "the queued videos could not be saved"
        );
    }
}

/// Write Dolphin's "Generate subtitles" entry for the running AppImage, and say how that went.
pub(crate) fn install_right_click() -> RightClickEntry {
    right_click_entry(service_menu::install_from_env())
}

/// The entry the Settings window shows for how writing the service menu went, logged: none when
/// the app runs outside its AppImage.
pub(crate) fn right_click_entry(installed: Option<io::Result<Installed>>) -> RightClickEntry {
    match installed {
        None => {
            tracing::info!("no service menu: the app runs outside its AppImage");
            RightClickEntry::NotInstalled
        }
        Some(Ok(Installed::Written(path))) => {
            tracing::info!(file = %path.display(), "the service menu is written");
            RightClickEntry::Installed(path)
        }
        Some(Ok(Installed::Unchanged(path))) => {
            tracing::debug!(file = %path.display(), "the service menu is up to date");
            RightClickEntry::Installed(path)
        }
        Some(Err(error)) => {
            tracing::warn!(%error, "the service menu could not be written");
            RightClickEntry::Failed(error.to_string())
        }
    }
}
