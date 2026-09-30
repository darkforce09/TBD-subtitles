//! Fix It's actions: what the Overview and the sidebar show about it, starting it on one video,
//! on every finished video with lines to fix, or after a full run, Stop, what happens once a run
//! ends, and its finish once its changes are in the subtitles.
//!
//! **Role:** say what Fix It is doing on each video (its pass, waiting for a free `claude` call,
//! or updating the subtitles) and why it cannot start on one; keep the runs under way and fold in
//! what each sent before every frame. `starting` starts and stops runs; `finishing` handles a
//! run's end and its finish.
//!
//! **Position:** called by the report's actions and `Action::FixIt`/`StopFix` (Fix It, Stop), the
//! queue's Fix All, the runner's end of a full run, lane rules and a review run's end, the
//! queue's refusals, by `feature_views` for the steps shown and the Fix All count, and before each
//! frame (`poll_fix`); uses `job_report::services::fix_it`.
//!
//! **Signals and state:** every run under way, by job, in `Pending`; the one gate every run's
//! `claude` calls share (`claude_gate`); the runs whose changes wait for their correction run
//! (`fix_followups`); the job just fixed; the desktop's attention asked for.
//!
//! **Invariants:** many runs go at once, at most one per video, each on its own thread; together
//! they hold no more `claude` calls than the saved "Claude calls at once", taken in the order the
//! runs started; a run never starts while a run of its video runs or a correction run of it
//! waits; no run of the video starts while it is fixed; a run that ended in changes queues
//! exactly one correction run of the video carrying them and says nothing until a correction run
//! of the video ends, then finishes once; a run with no change finishes at once; a failed or
//! stopped correction run finishes nothing; a stopped or failed Fix It queues nothing and names
//! its video; the desktop is told only while the window is unfocused or minimized.

mod finishing;
mod starting;

use std::path::{Path, PathBuf};

use crate::application::TbdSubtitlesApp;
use crate::job_queue::models::queue::{JobId, JobKind, QueueItem};
use crate::job_report::models::fixing::{FixView, UPDATING_STEP, step_of};
use crate::job_report::models::report::JobReport;
use crate::job_report::services::fix_it::Fixing;
use crate::settings::models::claude_models;

/// A Fix It run that ended, until its changes are in the subtitles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixFollowup {
    /// The finished job Fix It ran on.
    pub(crate) job: JobId,
    /// The model as the window names it, such as "Claude Opus".
    pub(crate) model: String,
    /// The lines whose change went into the corrections.
    pub(crate) changed: usize,
    /// The lines that kept the owner's own correction.
    pub(crate) kept_yours: usize,
    /// The model calls that failed.
    pub(crate) failed_calls: usize,
}

impl TbdSubtitlesApp {
    /// Fix It on job `id`'s Overview, whose report is `report`.
    pub(crate) fn fix_view(&self, id: JobId, report: &JobReport) -> FixView {
        if let Some(fixing) = self.fix_of(id) {
            let (stage, done, total) = fixing
                .progress
                .map_or((pipeline::fix_it::FixStage::Reading, 0, 1), |p| {
                    (p.stage, p.done, p.total)
                });
            return FixView::Running {
                model: fixing.model.clone(),
                stage,
                done,
                total,
                stopping: fixing.stopping,
                waiting: fixing.waiting(),
            };
        }
        let video = self.queue.get(id).map(|item| item.video.as_path());
        if let Some(followup) = video.and_then(|video| self.fix_updating(video)) {
            return FixView::Updating {
                model: followup.model.clone(),
            };
        }
        if report.fixable == 0 {
            return FixView::Hidden;
        }
        let model = claude_models::display_name(&self.settings.saved.language_model.fix_model);
        match self.fix_refusal(id) {
            Some(reason) => FixView::Unavailable {
                model,
                reason: reason.to_string(),
            },
            None => FixView::Ready { model },
        }
    }

    /// Each video Fix It is fixing, with its step of four: every run under way at its pass, and
    /// each run whose correction run waits or runs at the last step.
    pub(crate) fn fix_steps(&self) -> Vec<(PathBuf, usize)> {
        let mut steps: Vec<(PathBuf, usize)> = self
            .pending
            .fixes
            .values()
            .map(|fixing| {
                let stage = fixing
                    .progress
                    .map_or(pipeline::fix_it::FixStage::Reading, |p| p.stage);
                (fixing.video.clone(), step_of(stage))
            })
            .collect();
        for video in self.fix_followups.keys() {
            let listed = steps.iter().any(|(fixed, _)| fixed == video);
            if !listed && self.fix_updating(video).is_some() {
                steps.push((video.clone(), UPDATING_STEP));
            }
        }
        steps
    }

    /// The Fix It run of `video` whose correction run waits or runs, if there is one.
    fn fix_updating(&self, video: &Path) -> Option<&FixFollowup> {
        let followup = self.fix_followups.get(video)?;
        let pending = self.queue.items.iter().any(|item| {
            item.video == video && item.kind == JobKind::Review && item.state.is_queued()
        });
        pending.then_some(followup)
    }

    /// Why Fix It cannot start on job `id` now, if it cannot.
    fn fix_refusal(&self, id: JobId) -> Option<&'static str> {
        let Some(video) = self.queue.get(id).map(|item| &item.video) else {
            return Some("The video is not in the list.");
        };
        if self.video_fixing(id) {
            return Some("Fix It is already fixing this video.");
        }
        let busy = self.queue.items.iter().any(|item| {
            &item.video == video
                && (item.state.is_running()
                    || (item.kind == JobKind::Review && item.state.is_queued()))
        });
        busy.then_some("Wait until this video's subtitles are updated.")
    }

    /// The Fix It run of job `id`'s video under way, if there is one.
    fn fix_of(&self, id: JobId) -> Option<&Fixing> {
        let video = &self.queue.get(id)?.video;
        self.pending
            .fixes
            .values()
            .find(|fixing| &fixing.video == video)
    }

    /// Whether Fix It runs on job `id`'s video now.
    pub(crate) fn video_fixing(&self, id: JobId) -> bool {
        self.fix_of(id).is_some()
    }

    /// Job `job`'s video as messages name it, or `video`'s path once its row is gone.
    fn fix_name(&self, job: JobId, video: &Path) -> String {
        self.queue
            .get(job)
            .map_or_else(|| video.display().to_string(), QueueItem::short_name)
    }
}

/// Fold in what every Fix It run sent, and finish each that ended.
pub(crate) fn poll_fix(app: &mut TbdSubtitlesApp) {
    let ended: Vec<_> = app
        .pending
        .fixes
        .iter_mut()
        .filter_map(|(job, fixing)| fixing.poll().map(|outcome| (*job, outcome)))
        .collect();
    for (job, outcome) in ended {
        if let Some(fixing) = app.pending.fixes.remove(&job) {
            app.fix_ended(job, fixing, outcome);
        }
    }
}
