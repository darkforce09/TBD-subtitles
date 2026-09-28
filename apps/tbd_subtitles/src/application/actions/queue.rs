//! The queue's actions: adding and editing jobs, starting and pausing the queue, and cancelling a
//! running job.
//!
//! **Role:** turn each `JobQueueEvent` into a change of the queue, keep `queue.json` in step, and
//! let the runner start what may start next.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply`; uses `job_queue::services`;
//! `runner.rs` starts the jobs.
//!
//! **Signals and state:** the queue and the running jobs' cancel tokens; writes `queue.json`
//! after every change.
//!
//! **Invariants:** a cancel reaches only a running job, through its lane's token; the queue is
//! written after every event.

use crate::application::TbdSubtitlesApp;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::JobState;
use crate::job_queue::services::{queue_editing, queue_store};

impl TbdSubtitlesApp {
    pub(crate) fn apply_queue(&mut self, event: JobQueueEvent) {
        match event {
            JobQueueEvent::AddVideos => self.choose_for_queue(false),
            JobQueueEvent::AddFolder => self.choose_for_queue(true),
            JobQueueEvent::Select(id) => {
                self.queue.selected = Some(id);
                self.page = crate::application::Page::Jobs;
                if self.review.as_ref().is_some_and(|(job, _)| *job != id) {
                    self.apply_review(crate::line_review::events::ReviewEvent::Close);
                }
            }
            JobQueueEvent::Remove(id) => {
                queue_editing::remove(&mut self.queue, id);
            }
            JobQueueEvent::Move(id, to) => queue_editing::move_job(&mut self.queue, id, to),
            JobQueueEvent::Cancel(id) => {
                let lanes = [&self.cancel, &self.review_cancel];
                if let Some((_, token)) = lanes
                    .into_iter()
                    .flatten()
                    .find(|(running, _)| *running == id)
                {
                    token.cancel();
                    if let Some(item) = self.queue.get_mut(id)
                        && let JobState::Running(progress) = &mut item.state
                    {
                        progress.cancelling = true;
                    }
                }
            }
            JobQueueEvent::Retry(id) => {
                queue_editing::retry(&mut self.queue, id);
            }
            JobQueueEvent::Start => self.queue.running = true,
            JobQueueEvent::Pause => self.queue.running = false,
        }
        self.save_queue();
        self.start_next();
        self.refresh_report(false);
    }

    /// Whether a model or runtime archive a job needs is missing.
    pub(crate) fn models_missing(&self) -> bool {
        self.settings.items.iter().any(|item| !item.present)
    }

    pub(crate) fn save_queue(&self) {
        if let Err(error) = queue_store::save(&self.env.queue_path, &self.queue) {
            tracing::warn!(%error, "the queue could not be saved");
        }
    }
}
