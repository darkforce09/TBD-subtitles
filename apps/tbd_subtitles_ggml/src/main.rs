//! The ggml worker binary, `tbd-subtitles-ggml`.
//!
//! **Role:** run one Whisper step of a job (`asr_whisper` or `redecode_whisper`) in its own
//! process, so ggml never shares a process with ONNX Runtime.
//!
//! **Position:** started by the job runner in `tbd-subtitles` as
//! `tbd-subtitles-ggml worker <step> <job dir>`; the step's code is `pipeline::tasks`.
//!
//! **Signals and state:** reads the job's work directory and writes the step's output, its
//! measure file and `progress` lines on stdout.
//!
//! **Invariants:** only the Whisper steps run here; a build without the `crispasr` feature
//! refuses them instead of pretending to run.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use job_model::StepName;
use pipeline::graph::{self, Binary, Placement};

/// Runs the Whisper steps of a TBD-subtitles job.
#[derive(Debug, Parser)]
#[command(name = "tbd-subtitles-ggml", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run one Whisper step of a job; started by `tbd-subtitles`.
    Worker {
        /// The step to run: `asr_whisper` or `redecode_whisper`.
        #[arg(value_parser = parse_step)]
        step: StepName,
        /// The job's work directory.
        job_dir: PathBuf,
    },
}

fn parse_step(text: &str) -> Result<StepName, String> {
    let step: StepName = text.parse().map_err(|error| format!("{error}"))?;
    if graph::placement(step) != Placement::Worker(Binary::Ggml) {
        return Err(format!(
            "`{step}` runs in `tbd-subtitles`, not in the Whisper worker"
        ));
    }
    Ok(step)
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let Command::Worker { step, job_dir } = cli.command;
    anyhow::ensure!(
        cfg!(feature = "crispasr"),
        "this build has no CrispASR; build it with `--features crispasr` (the development environment runbook)"
    );
    pipeline::tasks::worker_main(step, &job_dir, Binary::Ggml)?;
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tbd-subtitles-ggml: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
