//! `stack-spike`: measures each piece of the ML stack on one video, from Rust.
//!
//! **Role:** the stack spike harness. `fetch` downloads the pinned model files and the CUDA 13 runtime;
//! the measuring commands run each stack item in a worker process of its own and record wall
//! time, speed against realtime, peak VRAM, peak RAM and quality notes.
//!
//! **Position:** a repository tool, run by a developer on the host; calls `inference` for
//! downloads and the backends under test.
//!
//! **Signals and state:** writes the models and runtime folders under
//! `~/.local/share/tbd-subtitles/` and the spike work folder; reads the video only.
//!
//! **Invariants:** the video and its folder are never written; a measurement that could not run
//! is reported as not run, never as a number.

mod fetch;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "stack-spike",
    about = "Measure each ML stack piece on one video"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download the pinned model files and the CUDA 13 runtime that are not yet in place.
    Fetch(fetch::FetchArgs),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Fetch(args) => fetch::run(&args),
    }
}
