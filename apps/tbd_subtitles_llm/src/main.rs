//! Isolated mistral.rs visual translation worker.
//!
//! **Role:** run text translation with the local GGUF model.
//! **Position:** worker executable started by the shared pipeline runner.
//! **Signals and state:** the job record and the step's inputs as frames on stdin; the step's
//! output, progress, model calls, the measure and the end or failure as frames of the worker
//! channel on stdout.
//! **Invariants:** this worker never initializes ONNX Runtime or ggml.

mod logging;

use clap::{Parser, Subcommand};
use job_model::StepName;
use pipeline::graph::Binary;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "tbd-subtitles-llm",
    version,
    about = "Local on-screen translation worker"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Worker { step: StepName, job_dir: PathBuf },
}

fn main() -> ExitCode {
    let Command::Worker { step, job_dir } = Cli::parse().command;
    logging::initialise();
    match pipeline::tasks::worker_main(step, &job_dir, Binary::LocalLlm) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tbd-subtitles-llm: {error}");
            ExitCode::FAILURE
        }
    }
}
