//! Fix It's actions: what the Overview and the sidebar show about it, starting it on the selected
//! video, Stop, what happens once a run ends, and its finish once its changes are in the
//! subtitles.
//!
//! **Role:** start one Fix It run with the model and processes saved now, keep the queue from
//! running the video meanwhile, and when the run ends mark the lines it changed and queue the
//! correction run that times them; once that run ended (at once when nothing changed), say what
//! Fix It did in a toast with See Changes, mark the job just fixed for its result card, and tell
//! the desktop when the window is away.
//!
//! **Position:** called by the report's actions (Fix It, Stop), by the runner's lane rules, the
//! queue's refusals and a review run's end, by `feature_views` for the steps shown, and before
//! each frame (`poll_fix`); uses `job_report::services::fix_it`.
//!
//! **Signals and state:** the one run under way, in `Pending`; the runs whose changes wait for
//! their correction run (`fix_followups`); the job just fixed; the desktop's attention asked for.
//!
//! **Invariants:** one Fix It runs at a time; it never starts while a run of its video runs or a
//! correction run of it waits; no run of the video starts while it is fixed; a run that ended in
//! changes queues exactly one correction run of the video carrying them and says nothing until
//! a correction run of the video ends, then finishes once; a run with no change finishes at once;
//! a failed or stopped correction run finishes nothing; a stopped or failed Fix It queues
//! nothing; the desktop is told only while the window is unfocused or minimized.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::application::{Action, TbdSubtitlesApp};
use crate::core::format::plural;
use crate::core::toast::ToastKind;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobId, JobKind, QueueItem};
use crate::job_queue::services::queue_editing;
use crate::job_report::events::{LinesToCheck, ReportEvent};
use crate::job_report::models::finding_group::LineGroup;
use crate::job_report::models::fixing::{FixView, UPDATING_STEP, step_of};
use crate::job_report::models::report::JobReport;
use crate::job_report::services::fix_it;
use crate::line_review::services::review_editing;
use crate::settings::models::claude_models;
use crate::settings::services::job_settings;

/// How long the toast that says Fix It finished shows.
const FINISHED_SHOWN: Duration = Duration::from_secs(8);

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

    /// Each video Fix It is fixing, with its step of four: the run under way at its pass, and
    /// each run whose correction run waits or runs at the last step.
    pub(crate) fn fix_steps(&self) -> Vec<(PathBuf, usize)> {
        let mut steps = Vec::new();
        if let Some((_, fixing)) = &self.pending.fix {
            let stage = fixing
                .progress
                .map_or(pipeline::fix_it::FixStage::Reading, |p| p.stage);
            steps.push((fixing.video.clone(), step_of(stage)));
        }
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
            item.video == video
                && item.kind == JobKind::Review
                && (item.state.is_running() || item.state.is_waiting())
        });
        pending.then_some(followup)
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
            calls: inference::llm::call_gate::CallGate::new(saved.language_model.processes.max(1))
                .seat(),
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

    /// A Fix It run of job `job` ended with `outcome`: with changes, their correction run is
    /// queued and the run finishes when it ends; with none, the run finishes now.
    fn fix_ended(
        &mut self,
        job: JobId,
        fixing: fix_it::Fixing,
        outcome: Result<pipeline::fix_it::FixOutcome, pipeline::PipelineError>,
    ) {
        let video = fixing.video.clone();
        let mut finished = false;
        match outcome {
            Ok(outcome) => {
                let changed = outcome.changed.len();
                let followup = FixFollowup {
                    job,
                    model: fixing.model.clone(),
                    changed,
                    kept_yours: outcome.kept_yours.len(),
                    failed_calls: outcome.record.failed_calls.len(),
                };
                self.fix_followups.insert(video.clone(), followup);
                if changed > 0 {
                    self.mark_fixed(job, &outcome.changed);
                    queue_editing::queue_review(&mut self.queue, video.clone(), changed);
                    self.save_queue();
                } else {
                    finished = true;
                }
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
        if finished {
            self.fix_finished(&video);
        }
    }

    /// A correction run of `video` ended, `ok` or not: a Fix It run waiting for it finishes, or,
    /// when it failed or was stopped, is forgotten.
    pub(crate) fn fix_run_ended(&mut self, video: &Path, ok: bool) {
        if ok {
            self.fix_finished(video);
        } else {
            self.fix_followups.remove(video);
        }
    }

    /// Fix It's run of `video` is over and its changes are in the subtitles: say what it did in a
    /// toast, with See Changes when it changed lines; mark the job just fixed; and, while the
    /// window is away, notify the desktop and ask for its attention.
    fn fix_finished(&mut self, video: &Path) {
        let Some(followup) = self.fix_followups.remove(video) else {
            return;
        };
        let name = self
            .queue
            .get(followup.job)
            .map_or_else(|| video.display().to_string(), QueueItem::short_name);
        let left = self.summaries.get(&followup.job).map_or(0, |s| s.problems);
        let words = finished_words(&name, &followup, left);
        let see = (followup.changed > 0).then(|| {
            let action = Action::SeeFixChanges(followup.job);
            ("See Changes".to_string(), action)
        });
        let (kind, now) = (ToastKind::Success, Instant::now());
        self.toasts
            .push_with(kind, words.toast, see, FINISHED_SHOWN, now);
        self.just_fixed = Some((video.to_path_buf(), self.presence.time));
        if self.presence.away {
            (self.env.notify)(&words.title, &words.body);
            self.attention = true;
        }
    }

    /// Select job `job` and open Check Lines on the lines Claude changed.
    pub(crate) fn see_fix_changes(&mut self, job: JobId) {
        if self.queue.get(job).is_none() {
            return;
        }
        self.apply_queue(JobQueueEvent::Select(job));
        let group = LinesToCheck::Group(LineGroup::ChangedByFixIt);
        self.apply_report(ReportEvent::CheckLines(group));
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

/// What the window says when Fix It finished: the toast, and the desktop notification's title
/// and body.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FinishedWords {
    toast: String,
    title: String,
    body: String,
}

/// The words for `followup` finishing on the video called `name`, with `left` problems left.
fn finished_words(name: &str, followup: &FixFollowup, left: usize) -> FinishedWords {
    let (mut toast, title, body) = if followup.changed > 0 {
        let lines = plural(followup.changed, "line");
        let body = match left {
            0 => format!("Claude changed {lines}. The subtitles are ready."),
            1 => format!("Claude changed {lines}; 1 problem is left for you."),
            n => format!("Claude changed {lines}; {n} problems are left for you."),
        };
        (
            format!("{name} is fixed: {body}"),
            format!("{name} is fixed"),
            body,
        )
    } else if followup.failed_calls > 0 {
        let body = "Nothing was changed.".to_string();
        (
            format!("Claude changed nothing in {name}."),
            format!("Claude checked {name}"),
            body,
        )
    } else {
        (
            format!("Claude checked {name}: every line was already right."),
            format!("Claude checked {name}"),
            "Every line was already right.".to_string(),
        )
    };
    if followup.kept_yours > 0 {
        let lines = plural(followup.kept_yours, "line");
        toast.push_str(&format!(" {lines} kept your own correction."));
    }
    if followup.failed_calls > 0 {
        let calls = plural(followup.failed_calls, "call");
        toast.push_str(&format!(" {calls} failed; Fix It again to ask them again."));
    }
    FinishedWords { toast, title, body }
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
