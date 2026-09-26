//! The thread that runs jobs, one at a time, and sends back what each reports.
//!
//! **Role:** own one long-lived thread that runs each job it is handed through the pipeline's
//! `run_job` (or a stand-in in the tests), forwarding every progress event and the outcome.
//!
//! **Position:** started by the application when the window opens; handed jobs by the
//! application's queue actions, which also cancel them through each job's `CancelToken`.
//!
//! **Signals and state:** a command channel in and an event channel out; the runner thread
//! starts the job's worker processes, which die with it.
//!
//! **Invariants:** one job runs at a time; the thread lives as long as the window, so a worker it
//! starts is never killed by its parent thread ending early (`PR_SET_PDEATHSIG`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use pipeline::progress::Progress;
use pipeline::{JobOptions, JobOutcome, PipelineError};

use crate::core::background::Wake;
use crate::job_queue::models::queue::JobId;

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

/// Start the runner thread; `wake` runs after each event it sends.
pub(crate) fn start(run: RunJob, wake: Wake) -> JobRunner {
    let (commands, inbox) = channel::<Command>();
    let (send, events) = channel();
    std::thread::Builder::new()
        .name("job-runner".to_string())
        .spawn(move || {
            for command in inbox {
                let id = command.id;
                let sink = |event: Progress| {
                    let _ = send.send(RunnerEvent::Progress(id, event));
                    wake();
                };
                let outcome = run(&command.video, &command.options, &sink);
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
