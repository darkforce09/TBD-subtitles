//! The command line: `gui`, `process` and `worker`.
//!
//! **Role:** declares the subcommands with clap and dispatches each to its runner.
//!
//! **Position:** called by `main`; starts `application` for `gui`; `process_command` and
//! `worker_command` check their arguments and stop with an error, because no pipeline stage is
//! built yet.
//!
//! **Signals and state:** reads the process arguments; no state.
//!
//! **Invariants:** running with no subcommand opens the window, as a desktop launcher expects;
//! `worker` accepts only stages that run in a worker process.

mod process_command;
mod worker_command;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use job_model::StageName;

/// Generates English subtitles for local videos.
#[derive(Debug, Parser)]
#[command(name = "tbd-subtitles", version, about)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Open the desktop window, with the given videos in the queue.
    Gui {
        /// Videos to put in the queue.
        videos: Vec<PathBuf>,
    },
    /// Generate subtitles for the given videos without a window.
    Process {
        /// Videos to process, one job each, in order.
        #[arg(required = true)]
        videos: Vec<PathBuf>,
    },
    /// Run one GPU stage of a job in this process; started by the job runner.
    Worker {
        /// The stage to run.
        #[arg(value_parser = parse_worker_stage)]
        stage: StageName,
        /// The job's work directory.
        job_dir: PathBuf,
    },
}

/// Parse the command line and run the chosen subcommand.
pub(crate) fn run() -> anyhow::Result<()> {
    dispatch(Cli::parse())
}

fn dispatch(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        None => crate::application::launch(Vec::new()),
        Some(Command::Gui { videos }) => crate::application::launch(videos),
        Some(Command::Process { videos }) => process_command::run(&videos),
        Some(Command::Worker { stage, job_dir }) => worker_command::run(stage, &job_dir),
    }
}

/// Accept a stage name only when that stage runs in a worker process.
fn parse_worker_stage(text: &str) -> Result<StageName, String> {
    let stage: StageName = text.parse().map_err(|error| format!("{error}"))?;
    if stage.runs_in_worker() {
        Ok(stage)
    } else {
        let workers: Vec<&str> = StageName::ALL
            .into_iter()
            .filter(|stage| stage.runs_in_worker())
            .map(StageName::as_str)
            .collect();
        Err(format!(
            "`{stage}` runs inside the job runner, not in a worker; worker stages: {}",
            workers.join(", ")
        ))
    }
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
