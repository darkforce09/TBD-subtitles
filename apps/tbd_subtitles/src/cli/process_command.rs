//! `tbd-subtitles process <video>...`: one job per video, run to the end without a window.
//!
//! **Role:** turn the options into job settings (the glossary, the models, the cut score, steps
//! to run again), run each video's job, and print each step as it starts, advances and finishes,
//! then where the subtitles and the report are.
//!
//! **Position:** called by `cli::dispatch`; runs `pipeline::run_job`; reads the built-in glossary
//! from `stages`.
//!
//! **Signals and state:** reads the videos' metadata and a glossary file when one is named; prints
//! to stderr.
//!
//! **Invariants:** every video is checked before the first job starts; the first failed job stops
//! the run with its reason.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, ValueEnum};
use job_model::StepName;
use job_model::job::{JobSettings, Separator, WhisperModel};
use pipeline::progress::Progress;
use pipeline::workers::Binaries;
use pipeline::{JobOptions, run_job};
use stages::adjudication::glossary;

/// The options of one `process` run.
#[derive(Debug, Args)]
pub(super) struct ProcessArgs {
    /// Videos to process, one job each, in order.
    #[arg(required = true)]
    pub(super) videos: Vec<PathBuf>,
    /// The folder that holds the jobs' work directories.
    #[arg(long)]
    work_root: Option<PathBuf>,
    /// The names the language model spells right: `one_piece` (built in), `none`, or a JSON file
    /// holding an array of names.
    #[arg(long, default_value = "one_piece")]
    glossary: String,
    /// The audio track to use, by its position among the audio streams; default: the English one.
    #[arg(long)]
    audio_track: Option<u32>,
    /// The vocal-separation model.
    #[arg(long, value_enum, default_value_t = SeparatorArg::Roformer)]
    separator: SeparatorArg,
    /// The second speech engine's model.
    #[arg(long, value_enum, default_value_t = WhisperArg::LargeV3)]
    whisper: WhisperArg,
    /// The lowest scdet score that counts as a shot cut.
    #[arg(long, default_value_t = 20.0)]
    cut_score: f64,
    /// The `claude` model the language-model steps ask.
    #[arg(long, default_value = "sonnet")]
    llm_model: String,
    /// Run these steps again even when their output is still valid (repeatable).
    #[arg(long, value_parser = parse_step)]
    rerun: Vec<StepName>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SeparatorArg {
    Roformer,
    MdxNet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum WhisperArg {
    LargeV3,
    LargeV3Turbo,
}

pub(super) fn parse_step(text: &str) -> Result<StepName, String> {
    text.parse().map_err(|error| format!("{error}"))
}

/// The job settings these options ask for.
pub(super) fn settings(args: &ProcessArgs) -> anyhow::Result<JobSettings> {
    let terms = match args.glossary.as_str() {
        "one_piece" => glossary::one_piece(),
        "none" => Vec::new(),
        path => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("cannot read the glossary {path}"))?;
            glossary::parse(&text)
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("glossary {path}"))?
        }
    };
    let mut settings = JobSettings::with_glossary(terms);
    settings.audio_track = args.audio_track;
    settings.separator = match args.separator {
        SeparatorArg::Roformer => Separator::Roformer,
        SeparatorArg::MdxNet => Separator::MdxNet,
    };
    settings.whisper = match args.whisper {
        WhisperArg::LargeV3 => WhisperModel::LargeV3,
        WhisperArg::LargeV3Turbo => WhisperModel::LargeV3Turbo,
    };
    settings.cut_score = args.cut_score;
    settings.llm_model = args.llm_model.clone();
    Ok(settings)
}

/// Run every video's job, stopping at the first that fails.
pub(super) fn run(args: &ProcessArgs) -> anyhow::Result<()> {
    for video in &args.videos {
        let metadata = std::fs::metadata(video)
            .with_context(|| format!("cannot read the video {}", video.display()))?;
        anyhow::ensure!(metadata.is_file(), "{} is not a file", video.display());
    }
    let options = JobOptions {
        work_root: match &args.work_root {
            Some(root) => root.clone(),
            None => pipeline::work_dir::default_root()?,
        },
        settings: settings(args)?,
        rerun: args.rerun.clone(),
        binaries: Binaries::beside_current_exe()?,
    };
    for video in &args.videos {
        let outcome = run_job(video, &options, &print)
            .with_context(|| format!("no subtitles for {}", video.display()))?;
        let s = &outcome.qc.summary;
        eprintln!("subtitles: {}", outcome.subtitles.display());
        eprintln!("report:    {}", outcome.report.display());
        eprintln!(
            "qc:        {} cues, {} findings, {:.1} % within 20 cps, layout {}",
            s.cues,
            outcome.qc.findings.len(),
            s.cps_ok_share * 100.0,
            if outcome.qc.has_layout_violations() {
                "rules broken (see the report)"
            } else {
                "clean"
            }
        );
    }
    Ok(())
}

/// One line per event on stderr.
fn print(event: Progress) {
    match event {
        Progress::JobStarted { video, work_dir } => {
            eprintln!("job {} (work {})", video.display(), work_dir.display());
        }
        Progress::StepSkipped(step) => eprintln!("  = {step} (still valid)"),
        Progress::StepStarted(step) => eprintln!("  > {step}"),
        Progress::StepAdvanced { step, done, total } => {
            if done == total || done % (total / 10).max(1) == 0 {
                eprintln!("    {step} {done}/{total}");
            }
        }
        Progress::StepMessage { step, text } => eprintln!("    {step}: {text}"),
        Progress::StepFinished { step, measure } => {
            let mib = |v: Option<f64>| v.map_or_else(|| "—".to_string(), |v| format!("{v:.0}"));
            eprintln!(
                "  ✓ {step} {:.1} s (RAM {} MiB, VRAM {} MiB)",
                measure.wall_s,
                mib(measure
                    .peak_ram_mib
                    .into_iter()
                    .chain(measure.peak_child_ram_mib)
                    .reduce(f64::max)),
                mib(measure.peak_vram_mib)
            );
        }
    }
}
