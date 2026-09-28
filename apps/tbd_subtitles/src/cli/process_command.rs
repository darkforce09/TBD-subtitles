//! `tbd-subtitles process <video>...`: one job per video, run to the end without a window.
//!
//! **Role:** put the options over the settings file, turn them into job settings (the glossary,
//! the models, the cut score, the output format, steps to run again), run each video's job, and
//! print each step as it starts, advances and finishes, then where the subtitles and the report
//! are.
//!
//! **Position:** called by `cli::dispatch`; runs `pipeline::run_job`; reads the settings file and
//! the glossary through `settings::services`.
//!
//! **Signals and state:** reads the settings file, the videos' metadata and a glossary file when
//! one is named; prints to stderr.
//!
//! **Invariants:** every video is checked before the first job starts; the first failed job stops
//! the run with its reason; an option given on the command line wins over the settings file.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, ValueEnum};
use job_model::StepName;
use job_model::job::{JobSettings, OutputFormat, Separator, WhisperModel};
use pipeline::progress::Progress;
use pipeline::workers::Binaries;
use pipeline::{CancelToken, JobOptions, JobOutcome, run_job};

use crate::settings::models::app_settings::AppSettings;
use crate::settings::services::{job_settings, settings_file};

/// The options of one `process` run.
#[derive(Debug, Args)]
pub(super) struct ProcessArgs {
    /// Videos to process, one job each, in order.
    #[arg(required = true)]
    pub(super) videos: Vec<PathBuf>,
    /// The settings file; default: `~/.config/tbd-subtitles/settings.toml`. The options below
    /// win over it.
    #[arg(long)]
    settings: Option<PathBuf>,
    /// The folder that holds the jobs' work directories.
    #[arg(long)]
    work_root: Option<PathBuf>,
    /// The folder the models are read from.
    #[arg(long)]
    models_dir: Option<PathBuf>,
    /// The names the language model spells right: `one_piece` (built in), `none`, or a JSON file
    /// holding an array of names.
    #[arg(long)]
    glossary: Option<String>,
    /// The audio track to use, by its position among the audio streams; default: the English one.
    #[arg(long)]
    audio_track: Option<u32>,
    /// The vocal-separation model.
    #[arg(long, value_enum)]
    separator: Option<SeparatorArg>,
    /// The second speech engine's model.
    #[arg(long, value_enum)]
    whisper: Option<WhisperArg>,
    /// The lowest scdet score that counts as a shot cut.
    #[arg(long)]
    cut_score: Option<f64>,
    /// The `claude` model the language-model steps ask.
    #[arg(long)]
    llm_model: Option<String>,
    /// The subtitle file written beside the video.
    #[arg(long, value_enum)]
    format: Option<FormatArg>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FormatArg {
    Srt,
    Vtt,
    Ass,
}

pub(super) fn parse_step(text: &str) -> Result<StepName, String> {
    text.parse().map_err(|error| format!("{error}"))
}

/// The settings file's values with these options put over them.
pub(super) fn merged(mut file: AppSettings, args: &ProcessArgs) -> AppSettings {
    if let Some(root) = &args.work_root {
        file.work_root = Some(root.clone());
    }
    if let Some(models) = &args.models_dir {
        file.models_dir = Some(models.clone());
    }
    if let Some(glossary) = &args.glossary {
        file.glossary = glossary.clone();
    }
    if let Some(separator) = args.separator {
        file.engines.separator = match separator {
            SeparatorArg::Roformer => Separator::Roformer,
            SeparatorArg::MdxNet => Separator::MdxNet,
        };
    }
    if let Some(whisper) = args.whisper {
        file.engines.whisper = match whisper {
            WhisperArg::LargeV3 => WhisperModel::LargeV3,
            WhisperArg::LargeV3Turbo => WhisperModel::LargeV3Turbo,
        };
    }
    if let Some(cut_score) = args.cut_score {
        file.cut_score = cut_score;
    }
    if let Some(model) = &args.llm_model {
        file.language_model.model = model.clone();
    }
    if let Some(format) = args.format {
        file.output_format = match format {
            FormatArg::Srt => OutputFormat::Srt,
            FormatArg::Vtt => OutputFormat::Vtt,
            FormatArg::Ass => OutputFormat::Ass,
        };
    }
    file
}

/// The job settings for `settings`, with the audio track, which only the command line names.
pub(super) fn settings(settings: &AppSettings, args: &ProcessArgs) -> anyhow::Result<JobSettings> {
    let mut job = job_settings::job_settings(settings)?;
    job.audio_track = args.audio_track;
    Ok(job)
}

/// Run every video's job, stopping at the first that fails.
pub(super) fn run(args: &ProcessArgs) -> anyhow::Result<()> {
    for video in &args.videos {
        let metadata = std::fs::metadata(video)
            .with_context(|| format!("cannot read the video {}", video.display()))?;
        anyhow::ensure!(metadata.is_file(), "{} is not a file", video.display());
    }
    let path = match &args.settings {
        Some(path) => path.clone(),
        None => settings_file::default_path()?,
    };
    let chosen = merged(settings_file::load(&path)?, args);
    let options = JobOptions {
        work_root: job_settings::work_root(&chosen)?,
        settings: settings(&chosen, args)?,
        rerun: args.rerun.clone(),
        binaries: Binaries::beside_current_exe()?,
        cancel: CancelToken::new(),
        gpu_lock: pipeline::work_dir::gpu_lock_path()?,
        models_dir: job_settings::models_dir(&chosen)?,
    };
    let missing = pipeline::models::missing(&options.models_dir, &options.settings);
    anyhow::ensure!(
        missing.is_empty(),
        "models missing from {}: {}; download them on the window's Settings page",
        options.models_dir.display(),
        missing.join(", ")
    );
    for video in &args.videos {
        let outcome = run_job(video, &options, &print)
            .with_context(|| format!("no subtitles for {}", video.display()))?;
        print_outcome(&outcome);
    }
    Ok(())
}

/// Where a finished job left its subtitles and report, and how its quality check came out.
pub(super) fn print_outcome(outcome: &JobOutcome) {
    let s = &outcome.qc.summary;
    eprintln!("subtitles: {}", outcome.subtitles.display());
    eprintln!("report:    {}", outcome.report.display());
    eprintln!(
        "qc:        {} cues, {} findings, {:.1} % within 20 cps, {}",
        s.cues,
        outcome.qc.findings.len(),
        s.cps_ok_share * 100.0,
        match outcome.qc.failures().as_slice() {
            [] => "passes".to_string(),
            reasons => format!("fails: {}", reasons.join("; ")),
        }
    );
}

/// One line per event on stderr.
pub(super) fn print(event: Progress) {
    match event {
        Progress::JobStarted {
            video,
            work_dir,
            stale,
        } => {
            eprintln!(
                "job {} (work {}; {} of {} steps to run)",
                video.display(),
                work_dir.display(),
                stale.len(),
                StepName::ALL.len()
            );
        }
        Progress::JobDuration(seconds) => eprintln!("  video {:.1} min", seconds / 60.0),
        Progress::StepFailed { step, message } => eprintln!("  ✗ {step}: {message}"),
        Progress::StepSkipped(step) => eprintln!("  = {step} (still valid)"),
        Progress::StepStarted(step) => eprintln!("  > {step}"),
        Progress::StepAdvanced { step, done, total } => {
            if done == total || done % (total / 10).max(1) == 0 {
                eprintln!("    {step} {done}/{total}");
            }
        }
        Progress::StepMessage { step, text } => eprintln!("    {step}: {text}"),
        Progress::ModelCall { .. } => {}
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
