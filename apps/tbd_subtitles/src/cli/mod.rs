//! The command line: videos alone, `gui`, `process`, `fix`, `dump` and `worker`.
//!
//! **Role:** declares the subcommands with clap and dispatches each to its runner.
//!
//! **Position:** called by `main`; `window_command` opens the window, or hands its videos to the
//! window already open, for videos alone, `gui` and `process --enqueue`; `process_command` runs
//! each video's job through `pipeline`; `fix_command` runs Fix It on a finished job and the
//! correction run after it; `dump_command` prints a job database's rows as JSON;
//! `worker_command` runs one step of a job for the job runner.
//!
//! **Signals and state:** reads the process arguments; starts logging, to the log file too when
//! the window opens; no state.
//!
//! **Invariants:** running with no subcommand opens the window, with the videos given, as a
//! desktop launcher expects; only the starts that concern the window claim the single instance;
//! `worker` refuses the steps that belong to the ggml worker binary.

mod dump_command;
mod fix_command;
mod process_command;
mod window_command;
mod worker_command;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use job_model::StepName;

use crate::core::logging::LogRun;

/// Generates English subtitles for local videos.
#[derive(Debug, Parser)]
#[command(
    name = "tbd-subtitles",
    version,
    about,
    args_conflicts_with_subcommands = true
)]
pub(crate) struct Cli {
    /// Videos to put in the window's queue, as `gui` does.
    videos: Vec<PathBuf>,
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
    /// Generate subtitles for the given videos and folders without a window, or queue them in
    /// the window with `--enqueue`.
    Process(process_command::ProcessArgs),
    /// Fix a finished video's flagged lines with a stronger `claude` model, without a window.
    Fix(fix_command::FixArgs),
    /// Print the rows of a job's database as JSON: one row pretty-printed, or a whole table as
    /// JSON Lines.
    Dump(dump_command::DumpArgs),
    /// Run one step of a job in this process; started by the job runner.
    Worker {
        /// The step to run.
        #[arg(value_parser = worker_command::parse_step)]
        step: StepName,
        /// The job's work directory.
        job_dir: PathBuf,
    },
}

/// Parse the command line and run the chosen subcommand; its exit code.
pub(crate) fn run() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    // The window's starts claim the single instance before logging, which empties the window's
    // log file only for the window that opens.
    if let Some(request) = window_request(&cli) {
        return window_command::run(request);
    }
    let run = match cli.command {
        Some(Command::Worker { .. }) => LogRun::Worker,
        _ => LogRun::Command,
    };
    crate::core::logging::initialise(run);
    dispatch(cli)
}

/// What a start that concerns the window asks for: videos alone, `gui`, `process --enqueue`.
fn window_request(cli: &Cli) -> Option<window_command::WindowRequest> {
    match &cli.command {
        None => Some(window_command::WindowRequest::open(cli.videos.clone())),
        Some(Command::Gui { videos }) => Some(window_command::WindowRequest::open(videos.clone())),
        Some(Command::Process(args)) if args.enqueue => {
            Some(window_command::WindowRequest::enqueue(args.videos.clone()))
        }
        Some(Command::Process(_) | Command::Fix(_) | Command::Dump(_) | Command::Worker { .. }) => {
            None
        }
    }
}

/// Run a subcommand that never opens the window.
fn dispatch(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        None | Some(Command::Gui { .. }) => {
            anyhow::bail!("the window opens through `window_command`, not `dispatch`")
        }
        Some(Command::Process(args)) => process_command::run(&args),
        Some(Command::Fix(args)) => fix_command::run(&args).map(|()| ExitCode::SUCCESS),
        Some(Command::Dump(args)) => dump_command::run(&args),
        Some(Command::Worker { step, job_dir }) => {
            worker_command::run(step, &job_dir).map(|()| ExitCode::SUCCESS)
        }
    }
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
