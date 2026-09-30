//! A Fix It run's end and its finish: the correction run that puts its changes into the
//! subtitles, then what Fix It did in a toast with See Changes, the job marked just fixed, and
//! the desktop told when the window is away.
//!
//! **Role:** when a run ends with changes, mark the lines it changed and queue the correction run
//! that times them; once that run ended (at once when nothing changed), say what Fix It did in a
//! toast with See Changes, mark the job just fixed for its result card, and tell the desktop when
//! the window is away; a stopped or failed run says so, naming its video, and so does a run
//! refused because another process runs the video, which is no failure.
//!
//! **Position:** part of the application's Fix It actions (`super`); `poll_fix` hands it each run
//! that ended, the runner each correction run that ended (`fix_run_ended`), and a toast's See
//! Changes comes back as `Action::SeeFixChanges`.
//!
//! **Signals and state:** the runs whose changes wait for their correction run
//! (`fix_followups`, one per video); the job just fixed; the desktop's attention asked for.
//!
//! **Invariants:** a run that ended in changes queues exactly one correction run of the video
//! carrying them and says nothing until a correction run of the video ends, then finishes once;
//! a run with no change finishes at once; a failed or stopped correction run finishes nothing; a
//! stopped or failed Fix It queues nothing; the desktop is told only while the window is
//! unfocused or minimized.

use std::path::Path;
use std::time::{Duration, Instant};

use super::FixFollowup;
use crate::application::{Action, TbdSubtitlesApp};
use crate::core::format::plural;
use crate::core::toast::ToastKind;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::JobId;
use crate::job_queue::services::queue_editing;
use crate::job_report::events::{LinesToCheck, ReportEvent};
use crate::job_report::models::finding_group::LineGroup;
use crate::job_report::services::fix_it;
use crate::line_review::services::review_editing;

/// "{name} is busy: process 4242 runs it; Fix It again once it ends", or "another process" when
/// `job.lock` names none.
fn busy_words(name: &str, owner: Option<u32>) -> String {
    let who = owner.map_or_else(
        || "another process".to_string(),
        |pid| format!("process {pid}"),
    );
    format!("{name} is busy: {who} runs it; Fix It again once it ends.")
}

/// How long the toast that says Fix It finished shows.
const FINISHED_SHOWN: Duration = Duration::from_secs(8);

impl TbdSubtitlesApp {
    /// A Fix It run of job `job` ended with `outcome`: with changes, their correction run is
    /// queued and the run finishes when it ends; with none, the run finishes now.
    pub(super) fn fix_ended(
        &mut self,
        job: JobId,
        fixing: fix_it::Fixing,
        outcome: Result<pipeline::fix_it::FixOutcome, pipeline::PipelineError>,
    ) {
        let video = fixing.video.clone();
        let name = self.fix_name(job, &video);
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
                format!(
                    "Fix It stopped on {name}. Nothing was changed; Fix It again picks up where \
                     it stopped."
                ),
            ),
            Err(error) => match error.busy_owner() {
                // Another process runs this video: nothing failed, and nothing was changed.
                Some(owner) => self.toast(ToastKind::Info, busy_words(&name, owner)),
                None => self.toast(
                    ToastKind::Error,
                    format!("Fix It failed on {name}: {}", error.message),
                ),
            },
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
        let name = self.fix_name(followup.job, video);
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
