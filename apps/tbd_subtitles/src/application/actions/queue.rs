//! The queue's actions: adding, selecting, removing and restoring rows, moving waiting jobs,
//! trying ended jobs again, opening and showing a video, starting and pausing the queue, and
//! cancelling a running job.
//!
//! **Role:** turn each `JobQueueEvent` into a change of the queue, tell the owner what happened
//! in a toast where the window shows nothing else (a file the desktop opens too), keep
//! `queue.json` in step, and let the runner start what may start next.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply`; uses `job_queue::services`;
//! `runner.rs` starts the jobs.
//!
//! **Signals and state:** the queue, the row removed last, the toasts and the running jobs'
//! cancel tokens; writes `queue.json` after every change.
//!
//! **Invariants:** a cancel reaches only a running job, through its lane's token; only the row
//! removed last comes back with Undo, and an older Undo toast goes when a row is removed; Try
//! Again and Run Again start a job at once when its lane is idle without turning the queue on,
//! and say so in red when it could not start; Run Again queues nothing when the settings saved
//! now are the ones the job ran with; a video is never put back while another run of it waits or
//! runs; a video Fix It is fixing is not removed, tried again or run again; the queue is written
//! after every event.

use std::time::{Duration, Instant};

use job_model::StepName;

use crate::application::background::Opening;
use crate::application::{Action, TbdSubtitlesApp};
use crate::core::steps::{STAGES, stage_of};
use crate::core::toast::ToastKind;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobId, JobKind, JobState, QueueItem, Removed};
use crate::job_queue::services::queue_editing::{self, Refusal};
use crate::job_queue::services::queue_store;
use crate::line_review::events::ReviewEvent;
use crate::settings::services::job_settings;

use super::runner::recorded_settings;

/// How long the toast that offers Undo shows.
const UNDO_SHOWN: Duration = Duration::from_secs(6);

impl TbdSubtitlesApp {
    pub(crate) fn apply_queue(&mut self, event: JobQueueEvent) {
        if let JobQueueEvent::Remove(id)
        | JobQueueEvent::TryAgain(id, _)
        | JobQueueEvent::RunAgain(id) = &event
            && self.video_fixing(*id)
        {
            let why = "Fix It is fixing this video. Stop it, or wait until it ends.";
            return self.toast(ToastKind::Error, why);
        }
        match event {
            JobQueueEvent::AddVideos => self.choose_for_queue(false),
            JobQueueEvent::AddFolder => self.choose_for_queue(true),
            JobQueueEvent::Select(id) => self.select(Some(id)),
            JobQueueEvent::Remove(id) => self.remove_row(id),
            JobQueueEvent::Undo(id) => self.undo(id),
            JobQueueEvent::Move(id, to) => queue_editing::move_job(&mut self.queue, id, to),
            JobQueueEvent::MoveBefore(id, before) => {
                queue_editing::move_before(&mut self.queue, id, before);
            }
            JobQueueEvent::Cancel(id) => self.cancel(id),
            JobQueueEvent::TryAgain(id, rerun) => self.try_again(id, rerun),
            JobQueueEvent::RunAgain(id) => self.run_again(id),
            JobQueueEvent::CheckLines(id) => {
                self.select(Some(id));
                self.open_review(None);
            }
            JobQueueEvent::Reveal(path) => self.open_with_desktop(Opening::Reveal, &path),
            JobQueueEvent::OpenVideo(path) => self.open_with_desktop(Opening::Play, &path),
            JobQueueEvent::Copied => self.toast(ToastKind::Success, "Subtitle path copied"),
            JobQueueEvent::Start => {
                self.queue.running = true;
                self.queue.pausing = false;
            }
            JobQueueEvent::Pause => self.pause(),
        }
        self.save_queue();
        self.start_next();
        self.refresh_report(false);
        self.close_review_unless_finished();
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

    /// Show `text` in a toast for the usual time.
    pub(crate) fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(kind, text, Instant::now());
    }

    /// Show job `id` on the right, closing a review of another job; another job ends the moment
    /// Fix It just finished.
    fn select(&mut self, id: Option<JobId>) {
        if self.queue.selected != id {
            self.just_fixed = None;
        }
        self.queue.selected = id;
        if self
            .review
            .as_ref()
            .is_some_and(|(job, _)| Some(*job) != id)
        {
            self.apply_review(ReviewEvent::Close);
        }
    }

    /// Take row `id` out of the list and offer Undo in a toast.
    fn remove_row(&mut self, id: JobId) {
        let Some(removed) = queue_editing::remove(&mut self.queue, id) else {
            return;
        };
        let name = removed.job().map(QueueItem::name).unwrap_or_default();
        self.removed = Some(removed);
        // Only the row removed last can come back: an older Undo goes with its toast.
        self.toasts.dismiss(|toast| {
            matches!(
                toast.action,
                Some((_, Action::Queue(JobQueueEvent::Undo(_))))
            )
        });
        self.toasts.push_with(
            ToastKind::Info,
            format!("Removed {name}. Its files stay on disk."),
            Some(("Undo".to_string(), Action::Queue(JobQueueEvent::Undo(id)))),
            UNDO_SHOWN,
            Instant::now(),
        );
        self.select(self.queue.selected);
    }

    /// Put the row of job `id` back, if it is the row removed last and its video is not in the
    /// queue again meanwhile.
    fn undo(&mut self, id: JobId) {
        let last = self
            .removed
            .as_ref()
            .and_then(Removed::job)
            .map(|job| job.id);
        if last != Some(id) {
            return;
        }
        let Some(removed) = self.removed.take() else {
            return;
        };
        let name = removed.job().map(QueueItem::name).unwrap_or_default();
        match queue_editing::restore(&mut self.queue, removed) {
            Ok(id) => self.select(Some(id)),
            Err(refusal) => self.refused(&name, refusal),
        }
    }

    /// Tell the owner why a job did not go back into the queue.
    fn refused(&mut self, name: &str, refusal: Refusal) {
        if refusal == Refusal::AlreadyQueued {
            self.toast(ToastKind::Info, format!("{name} is already in the list."));
        }
    }

    fn cancel(&mut self, id: JobId) {
        let full = self
            .cancel
            .as_ref()
            .filter(|(running, _)| *running == id)
            .map(|(_, token)| token);
        if let Some(token) = full.or_else(|| self.review_lanes.token(id)) {
            token.cancel();
            if let Some(item) = self.queue.get_mut(id)
                && let JobState::Running(progress) = &mut item.state
            {
                progress.cancelling = true;
            }
        }
    }

    /// Stop the queue once the running full run ends.
    fn pause(&mut self) {
        self.queue.running = false;
        let running = self
            .queue
            .items
            .iter()
            .find(|item| item.kind == JobKind::Full && item.state.is_running())
            .map(QueueItem::short_name);
        self.queue.pausing = running.is_some();
        if let Some(name) = running {
            self.toast(
                ToastKind::Info,
                format!("The queue pauses after {name} finishes."),
            );
        }
    }

    /// Put failed or cancelled job `id` first in line, and start it when its lane is idle.
    fn try_again(&mut self, id: JobId, rerun: Option<StepName>) {
        let Some(item) = self.queue.get(id) else {
            return;
        };
        let name = item.short_name();
        let stage = resume_stage(&item.state, rerun);
        match queue_editing::try_again(&mut self.queue, id, rerun) {
            Ok(()) => self.start_requeued(
                id,
                format!("Trying {name} again from {stage}."),
                format!("{name} runs next, from {stage}."),
            ),
            Err(refusal) => self.refused(&name, refusal),
        }
    }

    /// Put finished job `id` first in line with the settings saved now, and start it when its
    /// lane is idle; unless those are the settings it ran with, which the owner is told.
    fn run_again(&mut self, id: JobId) {
        let Some(item) = self.queue.get(id) else {
            return;
        };
        let name = item.short_name();
        let saved = &self.settings.saved;
        let current = job_settings::job_settings(saved).ok();
        let recorded = job_settings::work_root(saved)
            .ok()
            .and_then(|root| recorded_settings(&root, &item.video));
        if current.is_some() && current == recorded {
            self.toast(
                ToastKind::Info,
                format!("Nothing to run again: {name} was made with the current settings."),
            );
            return;
        }
        match queue_editing::run_again(&mut self.queue, id) {
            Ok(()) => self.start_requeued(
                id,
                format!("Running {name} again with the current settings."),
                format!("{name} runs next, with the current settings."),
            ),
            Err(refusal) => self.refused(&name, refusal),
        }
    }

    /// Start job `id`, just put first in line, when it can start now, and say which happened:
    /// it started, it could not start (in red), it runs next because the queue runs or it is a
    /// correction run, or it waits for Start Queue or for the models.
    fn start_requeued(&mut self, id: JobId, started: String, next: String) {
        let Some(item) = self.queue.get(id) else {
            return;
        };
        let (name, kind) = (item.short_name(), item.kind);
        if self.models_missing() {
            self.toast(ToastKind::Info, format!("{name} waits for the models."));
            return;
        }
        if self.start_now(id) {
            self.toast(ToastKind::Working, started);
            return;
        }
        match self.queue.get(id).map(|item| &item.state) {
            Some(JobState::Failed(failure)) => {
                let text = format!("{name} could not start: {}", failure.message);
                self.toast(ToastKind::Error, text);
            }
            _ if kind == JobKind::Review || self.queue.running => {
                self.toast(ToastKind::Info, next);
            }
            _ => self.toast(
                ToastKind::Info,
                format!("{name} is first in line. Press Start Queue to run it."),
            ),
        }
    }
}

/// The stage a job tried again resumes in: the step named, the step it failed at, the first
/// step it did not keep, or the last stage for a finished job.
fn resume_stage(state: &JobState, rerun: Option<StepName>) -> &'static str {
    let step = match (rerun, state) {
        (Some(step), _) => Some(step),
        (None, JobState::Failed(failure)) => Some(failure.step.unwrap_or(StepName::ALL[0])),
        (None, JobState::Cancelled { kept_steps }) => StepName::ALL.get(*kept_steps).copied(),
        _ => None,
    };
    step.map_or(STAGES[STAGES.len() - 1].title, |step| stage_of(step).title)
}
