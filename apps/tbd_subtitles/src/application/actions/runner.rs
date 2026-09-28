//! Running the queue's jobs: starting the next job of each lane with its options, and folding the
//! job runners' events in.
//!
//! **Role:** hand the next waiting job of each lane to its runner when nothing in that lane runs,
//! build the job's options, empty its steps to run again once the pipeline has recorded them, and
//! record how each job ends: finished, cancelled with the steps it kept, or failed at a step with
//! the steps it kept.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply` (through the queue and review
//! actions) and before each frame; uses `job_queue::services` and the settings.
//!
//! **Signals and state:** the queue, the running jobs' cancel tokens, the step rates; writes
//! `queue.json` after every change.
//!
//! **Invariants:** at most one full run and one review run run at once, each on its own runner;
//! no job starts while a model is missing; a run of a video never starts while a run of the
//! other kind of the same video runs, and a waiting full run holds its lane meanwhile; a job
//! takes the settings saved now until it has started once, and its own `job.json` settings
//! after that, as a review run always does.

use std::path::Path;
use std::time::Instant;

use job_model::job::{JobRecord, JobSettings};
use pipeline::progress::Progress;
use pipeline::workers::Binaries;
use pipeline::{CancelToken, JobOptions};

use crate::application::TbdSubtitlesApp;
use crate::job_queue::models::progress::JobProgress;
use crate::job_queue::models::queue::{Failure, JobId, JobKind, JobResult, JobState};
use crate::job_queue::services::job_runner::{Command, RunnerEvent};
use crate::job_queue::services::{progress_tracking, queue_editing, time_left};
use crate::settings::services::job_settings;

impl TbdSubtitlesApp {
    /// Hand the next job of each lane to its runner: the first waiting full run when the queue
    /// runs, no full run does and no review run of its video does; the first waiting review run
    /// when no review run does and no full run of its video does.
    pub(crate) fn start_next(&mut self) {
        if self.models_missing() {
            return;
        }
        if self.queue.running && self.cancel.is_none() {
            match queue_editing::next_waiting(&self.queue, JobKind::Full) {
                Some(id) if self.video_running(id, JobKind::Review) => {}
                Some(id) => self.cancel = self.start(id),
                None => self.queue.running = false,
            }
        }
        if self.review_cancel.is_none()
            && let Some(id) = queue_editing::next_waiting(&self.queue, JobKind::Review)
            && !self.video_running(id, JobKind::Full)
        {
            self.review_cancel = self.start(id);
        }
    }

    /// Whether a `kind` run of job `job`'s video is running now.
    pub(crate) fn video_running(&self, job: JobId, kind: JobKind) -> bool {
        let Some(video) = self.queue.get(job).map(|item| &item.video) else {
            return false;
        };
        self.queue
            .items
            .iter()
            .any(|item| &item.video == video && item.kind == kind && item.state.is_running())
    }

    /// Start job `id` on its lane's runner; the job and its cancel token when it started. From
    /// now on the job keeps its own settings; its steps to run again stay until the pipeline has
    /// recorded them (`poll_runner`).
    fn start(&mut self, id: JobId) -> Option<(JobId, CancelToken)> {
        let token = CancelToken::new();
        let options = self.options(id, token.clone());
        let item = self.queue.get_mut(id)?;
        let options = match options {
            Ok(options) => options,
            Err(error) => {
                item.state = JobState::Failed(Failure {
                    step: None,
                    message: format!("{error:#}"),
                    kept_steps: 0,
                });
                self.save_queue();
                return None;
            }
        };
        item.state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
        item.keep_settings = true;
        let kind = item.kind;
        let command = Command {
            id,
            video: item.video.clone(),
            options,
        };
        let runner = match kind {
            JobKind::Full => &self.runner,
            JobKind::Review => &self.review_runner,
        };
        let started = runner.run(command);
        self.save_queue();
        match started {
            Ok(()) => Some((id, token)),
            Err(message) => {
                if let Some(item) = self.queue.get_mut(id) {
                    item.state = JobState::Failed(Failure {
                        step: None,
                        message,
                        kept_steps: 0,
                    });
                }
                None
            }
        }
    }

    /// The options of job `id`: the settings saved now for a job that has never started, its
    /// own for one that has and for a review run; the steps it runs again.
    fn options(&self, id: JobId, cancel: CancelToken) -> anyhow::Result<JobOptions> {
        let saved = &self.settings.saved;
        let work_root = job_settings::work_root(saved)?;
        let item = self
            .queue
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("no job {id}"))?;
        let own = (item.keep_settings || item.kind == JobKind::Review)
            .then(|| recorded_settings(&work_root, &item.video))
            .flatten();
        let settings = match own {
            Some(settings) => settings,
            None => job_settings::job_settings(saved)?,
        };
        Ok(JobOptions {
            work_root,
            settings,
            rerun: item.rerun.clone(),
            binaries: Binaries::beside_current_exe()?,
            cancel,
            gpu_lock: self.env.gpu_lock.clone(),
            models_dir: job_settings::models_dir(saved)?,
        })
    }
}

/// The settings `video`'s job ran with, from its `job.json`.
fn recorded_settings(work_root: &Path, video: &Path) -> Option<JobSettings> {
    let video = std::fs::canonicalize(video).ok()?;
    let path = work_root
        .join(pipeline::work_dir::job_id(&video))
        .join("job.json");
    let record: JobRecord = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    Some(record.settings)
}

/// Fold what both job runners sent into the queue, and start the next job when one ended.
pub(crate) fn poll_runner(app: &mut TbdSubtitlesApp) {
    let mut events = Vec::new();
    while let Ok(event) = app.runner.events.try_recv() {
        events.push(event);
    }
    while let Ok(event) = app.review_runner.events.try_recv() {
        events.push(event);
    }
    let (mut ended, mut recorded) = (false, false);
    for event in events {
        match event {
            RunnerEvent::Progress(id, progress) => {
                if let Some(item) = app.queue.get_mut(id)
                    && let JobState::Running(job) = &mut item.state
                {
                    // The pipeline announces the job, or a first step, only after `job.json`
                    // holds the steps to run again; until then a failure keeps them for a retry.
                    let started = matches!(
                        progress,
                        Progress::JobStarted { .. } | Progress::StepStarted(_)
                    );
                    if started && !item.rerun.is_empty() {
                        item.rerun.clear();
                        recorded = true;
                    }
                    progress_tracking::apply(job, progress, Instant::now());
                }
            }
            RunnerEvent::Ended(id, outcome) => {
                if let Some(item) = app.queue.get_mut(id) {
                    let job = match &item.state {
                        JobState::Running(job) => Some(job.as_ref()),
                        _ => None,
                    };
                    let wall_s = job.map_or(0.0, |job| job.started.elapsed().as_secs_f64());
                    let kept_steps = job.map_or(0, JobProgress::kept_steps);
                    let step = job.and_then(|job| job.failed_step().or(job.current_step()));
                    item.state = match outcome {
                        Ok(outcome) => JobState::Finished(JobResult {
                            subtitles: outcome.subtitles,
                            work_dir: outcome.work_dir,
                            failures: outcome.qc.failures(),
                            findings: outcome.qc.findings.len(),
                            wall_s,
                        }),
                        Err(error) if error.is_cancelled() => JobState::Cancelled { kept_steps },
                        Err(error) => JobState::Failed(Failure {
                            step,
                            message: error.to_string(),
                            kept_steps,
                        }),
                    };
                }
                for lane in [&mut app.cancel, &mut app.review_cancel] {
                    if lane.as_ref().is_some_and(|(running, _)| *running == id) {
                        *lane = None;
                    }
                }
                ended = true;
            }
        }
    }
    if recorded && !ended {
        app.save_queue();
    }
    if ended {
        if let Ok(root) = job_settings::work_root(&app.settings.saved) {
            app.rates = time_left::from_history(&root);
        }
        app.save_queue();
        app.start_next();
        app.refresh_report(true);
        app.refresh_review();
    }
}
