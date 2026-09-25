//! `stack-spike`: measures each piece of the ML stack on one video, from Rust.
//!
//! **Role:** the stack spike harness. `fetch` downloads the pinned model files and the CUDA 13
//! runtime; `run` measures each stack item in a worker process of its own (wall time, speed
//! against realtime, peak VRAM, peak RAM and quality notes); `report` prints the results.
//!
//! **Position:** a repository tool, run by a developer on the host; calls `inference`,
//! `media_io` and `stages` for the pieces under test and `child_process` for its workers.
//!
//! **Signals and state:** writes the models and runtime folders under
//! `~/.local/share/tbd-subtitles/` and the spike work folder; reads the video only.
//!
//! **Invariants:** the video and its folder are never written; a measurement that could not run
//! is reported as not run, never as a number.

mod context;
mod fetch;
mod items;
mod measure;
mod report;
mod wav;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::context::Context;
use crate::items::Item;

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
    /// Measure items, each in a worker process of its own.
    Run(RunArgs),
    /// Run one item in this process (started by `run`).
    #[command(hide = true)]
    Worker(WorkerArgs),
    /// Print the recorded results as Markdown tables.
    Report(VideoArgs),
}

#[derive(Args)]
struct VideoArgs {
    /// The video under test; it is only read.
    #[arg(long)]
    video: PathBuf,
    /// The work folder (default: `<data home>/tbd-subtitles/work/spike-<video name>`).
    #[arg(long)]
    work: Option<PathBuf>,
}

#[derive(Args)]
struct RunArgs {
    /// The items to measure, in order (default: every item).
    items: Vec<Item>,
    #[command(flatten)]
    video: VideoArgs,
}

#[derive(Args)]
struct WorkerArgs {
    item: Item,
    #[command(flatten)]
    video: VideoArgs,
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Fetch(args) => fetch::run(&args),
        Command::Run(args) => {
            let ctx = Context::new(&args.video.video, args.video.work.as_deref())?;
            let items = if args.items.is_empty() {
                Item::ALL.to_vec()
            } else {
                args.items
            };
            for item in items {
                println!("{}: measuring", item.name());
                let result = measure::measure(&ctx, item)?;
                println!(
                    "{}: {:?} in {:.1} s ({:.1}× realtime){}",
                    result.item,
                    result.status,
                    result.wall_s,
                    result.x_realtime,
                    result.reason.map(|r| format!(": {r}")).unwrap_or_default()
                );
            }
            Ok(())
        }
        Command::Worker(args) => {
            let ctx = Context::new(&args.video.video, args.video.work.as_deref())?;
            measure::worker::run(&ctx, args.item)
        }
        Command::Report(args) => report::run(&Context::new(&args.video, args.work.as_deref())?),
    }
}
