//! Fix It's actions: what the Overview shows about it, starting it on the selected video, Stop,
//! and what happens once a run ends.
//!
//! **Role:** start one Fix It run with the model and processes saved now, keep the queue from
//! running the video meanwhile, and when the run ends mark the lines it changed, queue the
//! correction run that times them, and say what happened in a toast.
//!
//! **Position:** called by the report's actions (Fix It, Stop), by the runner's lane rules and
//! the queue's refusals, and before each frame (`poll_fix`); uses `job_report::services::fix_it`.
//!
//! **Signals and state:** the one run under way, in `Pending`.
//!
//! **Invariants:** one Fix It runs at a time; it never starts while a run of its video runs or a
//! correction run of it waits; no run of the video starts while it is fixed; a run that ended in
//! changes queues exactly one correction run of the video carrying them; a stopped or failed run
//! queues nothing.

use crate::application::TbdSubtitlesApp;
use crate::core::toast::ToastKind;
use crate::job_queue::models::queue::{JobId, JobKind};
use crate::job_queue::services::queue_editing;
use crate::job_report::models::fixing::FixView;
use crate::job_report::models::report::JobReport;
use crate::job_report::services::fix_it;
use crate::line_review::services::review_editing;
use crate::settings::models::claude_models;
use crate::settings::services::job_settings;

impl TbdSubtitlesApp {
    /// Fix It on job `id`'s Overview, whose report is `report`.
    pub(crate) fn fix_view(&self, id: JobId, report: &JobReport) -> FixView {
        if let Some((_, fixing)) = self.pending.fix.as_ref().filter(|_| self.video_fixing(id)) {
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

    /// Why Fix It cannot start on job `id` now, if it cannot.
    fn fix_refusal(&self, id: JobId) -> Option<&'static str> {
        let Some(video) = self.queue.get(id).map(|item| &item.video) else {
            return Some("The video is not in the list.");
        };
        if self.pending.fix.is_some() {
            return Some("Fix It is fixing another video; it fixes one at a time.");
        }
        let busy = self.queue.items.iter().any(|item| {
            &item.video == video
                && (item.state.is_running()
                    || (item.kind == JobKind::Review && item.state.is_waiting()))
        });
        busy.then_some("Wait until this video's subtitles are updated.")
    }

    /// Whether Fix It runs on job `id`'s video now.
    pub(crate) fn video_fixing(&self, id: JobId) -> bool {
        let video = self.queue.get(id).map(|item| &item.video);
        self.pending
            .fix
            .as_ref()
            .is_some_and(|(_, fixing)| Some(&fixing.video) == video)
    }

    /// Start Fix It on the selected job's video with the settings saved now.
    pub(crate) fn start_fix(&mut self) {
        let Some(id) = self.queue.selected else {
            return;
        };
        if let Some(reason) = self.fix_refusal(id) {
            return self.toast(ToastKind::Error, reason);
        }
        let Some(item) = self.queue.get(id) else {
            return;
        };
        let video = item.video.clone();
        let saved = &self.settings.saved;
        let work_root = match job_settings::work_root(saved) {
            Ok(root) => root,
            Err(error) => {
                return self.toast(ToastKind::Error, format!("Fix It cannot start: {error:#}"));
            }
        };
        let options = pipeline::fix_it::FixOptions {
            work_root,
            model: saved.language_model.fix_model.clone(),
            glossary_name: job_settings::glossary_name(saved),
            processes: saved.language_model.processes.max(1),
            cancel: pipeline::CancelToken::new(),
        };
        let model = claude_models::display_name(&options.model);
        let fixing = fix_it::start(
            self.env.fix_video.clone(),
            video,
            options,
            model,
            self.env.wake.clone(),
        );
        self.pending.fix = Some((id, fixing));
    }

    /// Stop the Fix It run under way.
    pub(crate) fn stop_fix(&mut self) {
        if let Some((_, fixing)) = &mut self.pending.fix {
            fixing.stop();
        }
    }

    /// A Fix It run of job `job` ended with `outcome`.
    fn fix_ended(
        &mut self,
        job: JobId,
        fixing: fix_it::Fixing,
        outcome: Result<pipeline::fix_it::FixOutcome, pipeline::PipelineError>,
    ) {
        let video = fixing.video.clone();
        let name = self
            .queue
            .get(job)
            .map_or_else(|| video.display().to_string(), |item| item.short_name());
        match outcome {
            Ok(outcome) => {
                let changed = outcome.changed.len();
                if changed > 0 {
                    self.mark_fixed(job, &outcome.changed);
                    queue_editing::queue_review(&mut self.queue, video.clone(), changed);
                    self.save_queue();
                }
                let mut text = if changed > 0 {
                    format!(
                        "{} changed {} in {name}. Updating the subtitles.",
                        fixing.model,
                        crate::core::format::plural(changed, "line")
                    )
                } else {
                    format!("{} found nothing to change in {name}.", fixing.model)
                };
                let yours = outcome.kept_yours.len();
                if yours > 0 {
                    let lines = crate::core::format::plural(yours, "line");
                    text.push_str(&format!(" {lines} kept your own correction."));
                }
                let failed = outcome.record.failed_calls.len();
                if failed > 0 {
                    let calls = crate::core::format::plural(failed, "call");
                    text.push_str(&format!(" {calls} failed; Fix It again to ask them again."));
                }
                let kind = if changed > 0 {
                    ToastKind::Success
                } else {
                    ToastKind::Info
                };
                self.toast(kind, text);
            }
            Err(error) if error.is_cancelled() => self.toast(
                ToastKind::Info,
                "Fix It stopped. Nothing was changed; Fix It again picks up where it stopped.",
            ),
            Err(error) => self.toast(
                ToastKind::Error,
                format!("Fix It failed: {}", error.message),
            ),
        }
        self.start_next();
        self.refresh_summaries(Some(&video));
        self.refresh_report(true);
        self.refresh_review();
    }

    /// Mark `ids`, which Fix It changed in job `job`'s video, as saved for their correction run,
    /// in its open review or its closed one.
    fn mark_fixed(&mut self, job: JobId, ids: &[String]) {
        match &mut self.review {
            Some((open, session)) if *open == job => {
                review_editing::mark_fixed(&mut session.runs, ids);
            }
            _ => {
                let parked = self.parked.entry(job).or_default();
                review_editing::mark_fixed(&mut parked.runs, ids);
            }
        }
    }
}

/// Fold in what the Fix It run sent, and finish it once it ended.
pub(crate) fn poll_fix(app: &mut TbdSubtitlesApp) {
    let ended = match &mut app.pending.fix {
        Some((_, fixing)) => fixing.poll(),
        None => return,
    };
    if let Some(outcome) = ended
        && let Some((job, fixing)) = app.pending.fix.take()
    {
        app.fix_ended(job, fixing, outcome);
    }
}
