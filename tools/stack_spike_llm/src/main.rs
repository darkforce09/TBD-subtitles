//! `stack-spike-llm`: the stack spike's worker for the local language model.
//!
//! **Role:** adjudicate the work folder's diff sheet with Qwen3.5-4B through mistral.rs, check the
//! answer, and write the same worker report `stack-spike` writes for its own items.
//!
//! **Position:** started by `stack-spike run` for the `llm-local` item, with the CUDA runtime on
//! `LD_LIBRARY_PATH`; calls `inference::llm::mistral_rs` and `stages::adjudication`.
//!
//! **Signals and state:** reads `sheet.json`, `glossary.json` and `probe.json`; writes
//! `adjudicated.qwen3.5-4b.json` and `results/llm-local.worker.json`.
//!
//! **Invariants:** this binary never loads ONNX Runtime or ggml, and the other stack spike
//! binaries never link candle.

mod report;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use report::Outcome;

#[derive(Parser)]
#[command(
    name = "stack-spike-llm",
    about = "The stack spike's local language-model worker"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the local language-model item in this process (started by `stack-spike run`).
    Worker {
        /// Only `llm-local`.
        item: String,
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
    anyhow::ensure!(
        item == "llm-local",
        "stack-spike-llm runs only llm-local, not {item}"
    );
    let outcome = run(&work)?;
    report::write(&work, &item, outcome)
}

#[cfg(feature = "mistralrs")]
fn run(work: &Path) -> anyhow::Result<Outcome> {
    use std::time::Instant;

    use stages::adjudication::{self, checks, summary};
    use stages::diff_sheet::sheet::Utterance;

    let read =
        |name: &str| -> anyhow::Result<String> { Ok(std::fs::read_to_string(work.join(name))?) };
    let sheet: Vec<Utterance> = serde_json::from_str(&read("sheet.json")?)?;
    let glossary_owned: Vec<String> = serde_json::from_str(&read("glossary.json")?)?;
    let glossary: Vec<&str> = glossary_owned.iter().map(String::as_str).collect();
    let probe: job_model::outputs::ProbeResult = serde_json::from_str(&read("probe.json")?)?;
    let load = Instant::now();
    let dir = inference::model_store::models_dir()?.join("qwen3.5-4b");
    let mut model = inference::llm::mistral_rs::MistralRs::open(&dir, "Qwen3.5-4B-Q4_K_M.gguf")?;
    let load_s = load.elapsed().as_secs_f64();
    let started = Instant::now();
    let result = adjudication::adjudicate(&mut model, &sheet, &glossary, |done, all| {
        println!("batch {done}/{all}");
    });
    let process_s = started.elapsed().as_secs_f64();
    let findings = checks::check(&sheet, &result.lines, &glossary);
    let answer = serde_json::json!({
        "lines": result.lines,
        "findings": findings,
        "failed_calls": result.failed_calls,
    });
    std::fs::write(
        work.join("adjudicated.qwen3.5-4b.json"),
        serde_json::to_vec_pretty(&answer)?,
    )?;
    Ok(Outcome {
        audio_s: probe.duration_s,
        load_s,
        process_s,
        notes: summary::summarize(&sheet, &result, &findings, &glossary),
    })
}

#[cfg(not(feature = "mistralrs"))]
fn run(_work: &Path) -> anyhow::Result<Outcome> {
    anyhow::bail!("stack-spike-llm was built without the `mistralrs` feature")
}
