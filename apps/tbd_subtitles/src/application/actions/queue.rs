//! The queue's actions: adding and editing jobs, starting and pausing the queue, cancelling the
//! running job, and folding the job runner's events in.
//!
//! **Role:** turn each `JobQueueEvent` into a change of the queue, hand the next waiting job to
//! the runner when the queue runs and nothing else does, and record how each job ends.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply` and before each frame; uses
//! `job_queue::services` and the settings to build each job's options.
//!
//! **Signals and state:** the queue, the running job's cancel token, the step rates; writes
//! `queue.json` after every change.
//!
//! **Invariants:** at most one job runs; no job starts while a model is missing; a new job takes
//! the settings chosen now, a retry or a review run the settings its `job.json` holds.

use std::path::Path;
use std::time::Instant;

use job_model::job::{JobRecord, JobSettings};
use pipeline::workers::Binaries;
use pipeline::{CancelToken, JobOptions};

use crate::application::TbdSubtitlesApp;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::progress::JobProgress;
use crate::job_queue::models::queue::{JobId, JobKind, JobResult, JobState};
use crate::job_queue::services::job_runner::{Command, RunnerEvent};
use crate::job_queue::services::{progress_tracking, queue_editing, queue_store, time_left};
use crate::settings::services::job_settings;

impl TbdSubtitlesApp {
    pub(crate) fn apply_queue(&mut self, event: JobQueueEvent) {
        match event {
            JobQueueEvent::AddVideos => self.choose_for_queue(false),
            JobQueueEvent::AddFolder => self.choose_for_queue(true),
            JobQueueEvent::Select(id) => {
                self.queue.selected = Some(id);
                self.page = crate::application::Page::Jobs;
            }
            JobQueueEvent::Remove(id) => {
                queue_editing::remove(&mut self.queue, id);
            }
            JobQueueEvent::Move(id, to) => queue_editing::move_job(&mut self.queue, id, to),
            JobQueueEvent::Cancel(id) => {
                if let Some((running, token)) = &self.cancel
                    && *running == id
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
    }

    /// Whether a model or runtime archive a job needs is missing.
    pub(crate) fn models_missing(&self) -> bool {
        self.settings.items.iter().any(|item| !item.present)
    }

    /// Hand the first waiting job to the runner when the queue runs and no job does.
    pub(crate) fn start_next(&mut self) {
        if !self.queue.running || self.cancel.is_some() || self.models_missing() {
            return;
        }
        let Some(id) = queue_editing::next_waiting(&self.queue) else {
            self.queue.running = false;
            return;
        };
        let token = CancelToken::new();
        let options = self.options(id, token.clone());
        let Some(item) = self.queue.get_mut(id) else {
            return;
        };
        let options = match options {
            Ok(options) => options,
            Err(error) => {
                item.state = JobState::Failed(format!("{error:#}"));
                self.save_queue();
                return;
            }
        };
        item.state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
        item.ran_before = true;
        let command = Command {
            id,
            video: item.video.clone(),
            options,
        };
        match self.runner.run(command) {
            Ok(()) => self.cancel = Some((id, token)),
            Err(error) => {
                if let Some(item) = self.queue.get_mut(id) {
                    item.state = JobState::Failed(error);
                }
            }
        }
        self.save_queue();
    }

    /// The options of job `id`: the settings chosen now for a first run, the job's own for a
    /// retry or a review run.
    fn options(&self, id: JobId, cancel: CancelToken) -> anyhow::Result<JobOptions> {
        let saved = &self.settings.saved;
        let work_root = job_settings::work_root(saved)?;
        let item = self
            .queue
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("no job {id}"))?;
        let own = (item.ran_before || item.kind == JobKind::Review)
            .then(|| recorded_settings(&work_root, &item.video))
            .flatten();
        let settings = match own {
            Some(settings) => settings,
            None => job_settings::job_settings(saved)?,
        };
        Ok(JobOptions {
            work_root,
            settings,
            rerun: Vec::new(),
            binaries: Binaries::beside_current_exe()?,
            cancel,
            gpu_lock: self.env.gpu_lock.clone(),
            models_dir: job_settings::models_dir(saved)?,
        })
    }

    pub(crate) fn save_queue(&self) {
        if let Err(error) = queue_store::save(&self.env.queue_path, &self.queue) {
            tracing::warn!(%error, "the queue could not be saved");
        }
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

/// Fold what the job runner sent into the queue, and start the next job when one ended.
pub(crate) fn poll_runner(app: &mut TbdSubtitlesApp) {
    let mut ended = false;
    while let Ok(event) = app.runner.events.try_recv() {
        match event {
            RunnerEvent::Progress(id, progress) => {
                if let Some(item) = app.queue.get_mut(id)
                    && let JobState::Running(job) = &mut item.state
                {
                    progress_tracking::apply(job, progress, Instant::now());
                }
            }
            RunnerEvent::Ended(id, outcome) => {
                let started = app.queue.get(id).and_then(|item| match &item.state {
                    JobState::Running(job) => Some(job.started),
                    _ => None,
                });
                let wall_s = started.map_or(0.0, |s| s.elapsed().as_secs_f64());
                if let Some(item) = app.queue.get_mut(id) {
                    item.state = match outcome {
                        Ok(outcome) => JobState::Finished(JobResult {
                            subtitles: outcome.subtitles,
                            work_dir: outcome.work_dir,
                            failures: outcome.qc.failures(),
                            findings: outcome.qc.findings.len(),
                            wall_s,
                        }),
                        Err(error) if error.is_cancelled() => JobState::Cancelled,
                        Err(error) => JobState::Failed(error.to_string()),
                    };
                }
                if app
                    .cancel
                    .as_ref()
                    .is_some_and(|(running, _)| *running == id)
                {
                    app.cancel = None;
                }
                ended = true;
            }
        }
    }
    if ended {
        if let Ok(root) = job_settings::work_root(&app.settings.saved) {
            app.rates = time_left::from_history(&root);
        }
        app.save_queue();
        app.start_next();
    }
}
