//! The command line: `gui`, `process` and `worker`.
//!
//! **Role:** declares the subcommands with clap and dispatches each to its runner.
//!
//! **Position:** called by `main`; starts `application` for `gui`; `process_command` runs each
//! video's job through `pipeline`; `worker_command` runs one step of a job for the job runner.
//!
//! **Signals and state:** reads the process arguments; starts logging, to the log file too when
//! the window opens; no state.
//!
//! **Invariants:** running with no subcommand opens the window, as a desktop launcher expects;
//! `worker` refuses the steps that belong to the ggml worker binary.

mod process_command;
mod worker_command;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use job_model::StepName;

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
    Process(process_command::ProcessArgs),
    /// Run one step of a job in this process; started by the job runner.
    Worker {
        /// The step to run.
        #[arg(value_parser = worker_command::parse_step)]
        step: StepName,
        /// The job's work directory.
        job_dir: PathBuf,
    },
}

/// Parse the command line and run the chosen subcommand.
pub(crate) fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let window = matches!(cli.command, None | Some(Command::Gui { .. }));
    let log_file = window.then(crate::core::logging::window_log_path).flatten();
    crate::core::logging::initialise(log_file.as_deref());
    dispatch(cli)
}

fn dispatch(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        None => crate::application::launch(Vec::new()),
        Some(Command::Gui { videos }) => crate::application::launch(videos),
        Some(Command::Process(args)) => process_command::run(&args),
        Some(Command::Worker { step, job_dir }) => worker_command::run(step, &job_dir),
    }
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
