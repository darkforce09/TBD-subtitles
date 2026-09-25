//! `stack-spike-ggml`: the stack spike's worker for the ggml models.
//!
//! **Role:** run one ggml item (Whisper over the chunk plan, or the Qwen3 forced aligner) in this
//! process and write the same worker report `stack-spike` writes for its own items.
//!
//! **Position:** started by `stack-spike run` for the ggml items, with the CUDA runtime on
//! `LD_LIBRARY_PATH`; calls `inference::ggml::crispasr` and `stages`.
//!
//! **Signals and state:** reads the work folder's chunk plan and audio; writes the item's output
//! and `results/<item>.worker.json` there.
//!
//! **Invariants:** this binary never loads ONNX Runtime, and `stack-spike` never links ggml.

mod align;
mod items;
mod report;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "stack-spike-ggml", about = "The stack spike's ggml worker")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run one ggml item in this process (started by `stack-spike run`).
    Worker {
        item: items::Item,
        /// The video under test; only its name is used here.
        #[arg(long)]
        video: PathBuf,
        /// The work folder `stack-spike` measures in.
        #[arg(long)]
        work: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let Command::Worker {
        item,
        video: _,
        work,
    } = Cli::parse().command;
    let outcome = items::run(item, &work)?;
    report::write(&work, item.name(), outcome)
}
