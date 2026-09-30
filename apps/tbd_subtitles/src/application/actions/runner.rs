//! Running the queue's jobs: starting the next job of each lane with its options, and folding the
//! job runners' events in.
//!
//! **Role:** hand the next waiting full run to the full runner when no full run runs, and waiting
//! review runs to the review lanes while one is idle; build the job's options, empty its steps to
//! run again once the pipeline has recorded them, and record how each job ends: finished,
//! cancelled with the steps it kept, or failed at a step with the steps it kept, moving it ahead
//! of the jobs that ended before it; tell the desktop when a full run finished or failed while
//! the window is away; tell the open line review when a review run of its video
//! starts and ends, and Fix It when one ends, once the summaries and the report are read again;
//! once they are, start Fix It on each full run that finished well when the owner asked for Fix
//! It after each job.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply` (through the queue and review
//! actions) and before each frame; uses `job_queue::services` and the settings.
//!
//! **Signals and state:** the queue, the running jobs' cancel tokens, the step rates, and the
//! summaries of the rows of a video whose run ended; writes `queue.json` after every change.
//!
//! **Invariants:** at most one full run runs at once, on its own runner, and up to four review
//! runs, each on its own review lane (`REVIEW_LANES`), never two of the same video; no job starts
//! while a model is missing; a job started at once (Try Again, Run Again) leaves the queue off,
//! and a pause ends once the full lane is idle; a run of a video never starts while another run
//! of the same video runs or Fix It fixes it, and a waiting full run holds its lane meanwhile; a
//! job takes the settings saved now until it has started once, and its own `job.json` settings
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
use crate::job_queue::services::{
    job_notice, progress_tracking, queue_editing, time_left, video_files,
};
use crate::settings::models::machine::ItemKind;
use crate::settings::services::job_settings;

impl TbdSubtitlesApp {
    /// Hand the next job of each lane to its runner: the first waiting full run when the queue
    /// runs, no full run does, and no review run or Fix It of its video does; then, while a
    /// review lane is idle, the first waiting review run whose video no run and no Fix It holds.
    pub(crate) fn start_next(&mut self) {
        if self.cancel.is_none() {
            self.queue.pausing = false;
        }
        if self.queue.running && self.cancel.is_none() {
            match queue_editing::next_waiting(&self.queue, JobKind::Full) {
                Some(id)
                    if self.video_running(id, JobKind::Review)
                        || self.video_fixing(id)
                        || self.models_missing_for(id) => {}
                Some(id) => self.start(id),
                None => self.queue.running = false,
            }
        }
        // Each pass starts or fails one waiting run, so the loop ends.
        while self.review_lanes.free() {
            let startable = |id| {
                !self.video_running(id, JobKind::Full)
                    && !self.video_running(id, JobKind::Review)
                    && !self.video_fixing(id)
                    && !self.models_missing_for(id)
            };
            let Some(id) = queue_editing::first_startable(&self.queue, JobKind::Review, startable)
            else {
                break;
            };
            self.start(id);
        }
    }

    /// Start job `id` now when its lane is idle and every model is on disk, without turning the
    /// queue on; whether it runs.
    pub(crate) fn start_now(&mut self, id: JobId) -> bool {
        if self.models_missing_for(id) {
            return false;
        }
        match self.queue.get(id).map(|item| item.kind) {
            Some(JobKind::Full) => {
                let held = self.video_running(id, JobKind::Review) || self.video_fixing(id);
                if self.cancel.is_none() && !held {
                    self.start(id);
                }
            }
            Some(JobKind::Review) => self.start_next(),
            None => return false,
        }
        self.queue
            .get(id)
            .is_some_and(|item| item.state.is_running())
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

    /// Start job `id` on its lane's runner, which holds it with its cancel token: the full
    /// runner, or an idle review lane. Either way the job no longer waits: it runs, or it failed
    /// to start. From now on the job keeps its own settings; its steps to run again stay until
    /// the pipeline has recorded them (`poll_runner`).
    fn start(&mut self, id: JobId) {
        let token = CancelToken::new();
        let options = self.options(id, token.clone());
        let Some(item) = self.queue.get_mut(id) else {
            return;
        };
        let options = match options {
            Ok(options) => options,
            Err(error) => {
                item.state = JobState::Failed(Failure::new(None, format!("{error:#}"), Vec::new()));
                let (kind, video) = (item.kind, item.video.clone());
                queue_editing::newest_ended_first(&mut self.queue, id);
                self.save_queue();
                if kind == JobKind::Review {
                    self.review_run_started(&video);
                    self.review_run_ended(&video, false);
                }
                return;
            }
        };
        let mut progress = JobProgress::new(Instant::now());
        progress.idle = time_left::idle_steps(&options.settings);
        item.state = JobState::Running(Box::new(progress));
        item.keep_settings = true;
        let kind = item.kind;
        let command = Command {
            id,
            video: item.video.clone(),
            options,
        };
        let video = command.video.clone();
        let started = match kind {
            JobKind::Full => {
                let started = self.runner.run(command);
                if started.is_ok() {
                    self.cancel = Some((id, token));
                }
                started
            }
            JobKind::Review => self.review_lanes.run(id, command, token),
        };
        self.save_queue();
        if kind == JobKind::Review {
            self.review_run_started(&video);
        }
        if let Err(message) = started {
            if let Some(item) = self.queue.get_mut(id) {
                item.state = JobState::Failed(Failure::new(None, message, Vec::new()));
            }
            queue_editing::newest_ended_first(&mut self.queue, id);
            if kind == JobKind::Review {
                self.review_run_ended(&video, false);
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
        Ok(JobOptions {
            work_root,
            settings: self.settings_for(id)?,
            rerun: item.rerun.clone(),
            binaries: Binaries::beside_current_exe()?,
            cancel,
            gpu_lock: self.env.gpu_lock.clone(),
            models_dir: job_settings::models_dir(saved)?,
        })
    }

    /// A resume or correction uses its recorded settings, regardless of the new-job defaults.
    fn settings_for(&self, id: JobId) -> anyhow::Result<JobSettings> {
        let saved = &self.settings.saved;
        let item = self
            .queue
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("no job {id}"))?;
        let own = if item.keep_settings || item.kind == JobKind::Review {
            recorded_settings(&job_settings::work_root(saved)?, &item.video)
        } else {
            None
        };
        own.map_or_else(|| job_settings::job_settings(saved), Ok)
    }

    /// The shared runtime and this job's models, rather than every new job's model choice.
    pub(crate) fn models_missing_for(&self, id: JobId) -> bool {
        let Ok(settings) = self.settings_for(id) else {
            // Starting reports malformed settings as an actionable failure, not a model wait.
            return false;
        };
        if self
            .settings
            .items
            .iter()
            .any(|item| item.kind == ItemKind::Runtime && !item.present)
        {
            return true;
        }
        pipeline::models::required(&settings)
            .into_iter()
            .any(|model| {
                self.settings
                    .items
                    .iter()
                    .find(|item| item.kind == ItemKind::Model && item.id == model)
                    .map_or_else(
                        || {
                            !inference::model_store::is_complete(
                                &self.settings.models_folder,
                                model,
                            )
                        },
                        |item| !item.present,
                    )
            })
    }
}

/// The settings `video`'s job ran with, from its `job.json`.
pub(crate) fn recorded_settings(work_root: &Path, video: &Path) -> Option<JobSettings> {
    let video = std::fs::canonicalize(video).ok()?;
    let path = work_root
        .join(pipeline::work_dir::job_id(&video))
        .join("job.json");
    let record: JobRecord = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    Some(record.settings)
}

/// Fold what the full runner and the review lanes sent into the queue, and start the next jobs
/// when one ended.
pub(crate) fn poll_runner(app: &mut TbdSubtitlesApp) {
    let mut events = Vec::new();
    while let Ok(event) = app.runner.events.try_recv() {
        events.push(event);
    }
    app.review_lanes.drain(&mut events);
    let (mut ended, mut recorded) = (false, false);
    let mut ended_videos = Vec::new();
    let mut reviewed = Vec::new();
    let mut finished_full = Vec::new();
    let mut notices = Vec::new();
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
                    let finished = job.map_or_else(Vec::new, JobProgress::finished_steps);
                    let kept_steps = finished.len();
                    let step = job.and_then(|job| job.failed_step().or(job.current_step()));
                    if item.kind == JobKind::Review {
                        reviewed.push((item.video.clone(), outcome.is_ok()));
                    } else if outcome.is_ok() {
                        finished_full.push(id);
                    }
                    item.state = match outcome {
                        Ok(outcome) => JobState::Finished(JobResult {
                            localized: video_files::localized_video(&outcome.work_dir),
                            subtitles: outcome.subtitles,
                            work_dir: outcome.work_dir,
                            failures: outcome.qc.failures(),
                            findings: outcome.qc.findings.len(),
                            wall_s,
                        }),
                        Err(error) if error.is_cancelled() => JobState::Cancelled { kept_steps },
                        Err(error) => {
                            JobState::Failed(Failure::new(step, error.to_string(), finished))
                        }
                    };
                    notices.extend(job_notice::ended_notice(item));
                    ended_videos.push(item.video.clone());
                }
                queue_editing::newest_ended_first(&mut app.queue, id);
                if app
                    .cancel
                    .as_ref()
                    .is_some_and(|(running, _)| *running == id)
                {
                    app.cancel = None;
                }
                app.review_lanes.release(id);
                ended = true;
            }
        }
    }
    if recorded && !ended {
        app.save_queue();
    }
    // The runs that ended settle their lines before the next run marks the lines it takes.
    for (video, ok) in &reviewed {
        app.review_run_ended(video, *ok);
    }
    if ended {
        app.notify_ended(notices);
        if let Ok(root) = job_settings::work_root(&app.settings.saved) {
            app.rates = time_left::from_history(&root);
        }
        app.save_queue();
        app.start_next();
        for video in &ended_videos {
            app.refresh_summaries(Some(video));
        }
        app.refresh_report(true);
        app.refresh_review();
        if app
            .text
            .job
            .and_then(|id| app.queue.get(id))
            .is_some_and(|item| ended_videos.contains(&item.video))
        {
            app.refresh_text();
        }
        // Fix It finishes once the subtitles hold its changes, so its words count what is left.
        for (video, ok) in &reviewed {
            app.fix_run_ended(video, *ok);
        }
        // The summaries are fresh, so Fix It after each job knows which have lines to fix.
        app.fix_after_run(&finished_full);
    }
}
