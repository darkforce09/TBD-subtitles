//! The thread that runs jobs, one at a time, and sends back what each reports.
//!
//! **Role:** own one long-lived thread that runs each job it is handed through the pipeline's
//! `run_job` (or a stand-in in the tests), forwarding every progress event and the outcome.
//!
//! **Position:** started by the application when the window opens, once for full runs and four
//! times through `review_lanes` for correction runs; handed jobs by the application's queue
//! actions, which also cancel them through each job's `CancelToken`.
//!
//! **Signals and state:** a command channel in and an event channel out; the runner thread
//! starts the job's worker processes, which die with it. Each event and each end is also logged
//! (`progress_log`) in a span naming the video; a worker's model call goes to the log window only.
//!
//! **Invariants:** one job runs at a time; the thread lives as long as the window, so a worker it
//! starts is never killed by its parent thread ending early (`PR_SET_PDEATHSIG`).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use pipeline::progress::Progress;
use pipeline::{JobOptions, JobOutcome, PipelineError};

use crate::core::background::Wake;
use crate::job_queue::models::queue::JobId;
use crate::job_queue::services::progress_log::{self, ProgressLog};
use crate::job_queue::services::video_files;

/// How a job is run: the pipeline's `run_job`, or a stand-in in the tests.
pub(crate) type RunJob = Arc<
    dyn Fn(&Path, &JobOptions, &(dyn Fn(Progress) + Sync)) -> Result<JobOutcome, PipelineError>
        + Send
        + Sync,
>;

/// The pipeline's own runner.
pub(crate) fn pipeline_runner() -> RunJob {
    Arc::new(|video, options, progress| pipeline::run_job(video, options, progress))
}

/// A job handed to the runner.
pub(crate) struct Command {
    pub(crate) id: JobId,
    pub(crate) video: PathBuf,
    pub(crate) options: JobOptions,
}

/// What the runner sends back.
#[derive(Debug)]
pub(crate) enum RunnerEvent {
    Progress(JobId, Progress),
    Ended(JobId, Result<JobOutcome, PipelineError>),
}

/// The running thread's two channels.
pub(crate) struct JobRunner {
    commands: Sender<Command>,
    pub(crate) events: Receiver<RunnerEvent>,
}

impl JobRunner {
    /// Hand the runner a job; it starts when the one before it ends.
    pub(crate) fn run(&self, command: Command) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "the job runner has stopped".to_string())
    }
}

/// Start the runner thread, named `name`; `wake` runs after each event it sends.
pub(crate) fn start(name: &str, run: RunJob, wake: Wake) -> JobRunner {
    let (commands, inbox) = channel::<Command>();
    let (send, events) = channel();
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            for command in inbox {
                let id = command.id;
                let name = command
                    .video
                    .file_stem()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let log = Mutex::new(ProgressLog::default());
                // Everything the job logs, its steps' programs too, is under its video.
                let job_span = tracing::info_span!("job", video = name.as_str());
                let _in_job = job_span.enter();
                let sink = |event: Progress| {
                    if let Progress::ModelCall { call, .. } = &event {
                        progress_log::emit_call(call);
                        return;
                    }
                    let line = log
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .describe(&event);
                    if let Some(line) = line {
                        progress_log::emit(&name, &line);
                    }
                    let _ = send.send(RunnerEvent::Progress(id, event));
                    wake();
                };
                let outcome = run(&command.video, &command.options, &sink);
                let localized = outcome
                    .as_ref()
                    .ok()
                    .and_then(|done| video_files::localized_video(&done.work_dir));
                let end = progress_log::describe_end(&outcome, localized.as_deref());
                progress_log::emit(&name, &end);
                let _ = send.send(RunnerEvent::Ended(id, outcome));
                wake();
            }
        })
        .map(|_| ())
        .unwrap_or_else(|error| tracing::error!(%error, "the job runner could not start"));
    JobRunner { commands, events }
}

#[cfg(test)]
#[path = "tests/job_runner.rs"]
mod tests;
