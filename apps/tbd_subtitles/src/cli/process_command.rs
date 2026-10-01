//! `tbd-subtitles process <video or folder>...`: one job per video, run to the end without a
//! window.
//!
//! **Role:** find the videos in the folders given, put the options over the settings file, turn
//! them into job settings (the glossary, the models, the cut score, the output format, steps to
//! run again) and whether the sign library is used, run each video's job, and print each step as
//! it starts, advances and finishes, then where the subtitles and the report are, the run's wall
//! time and whole-job peak memory, and which jobs failed the quality check.
//!
//! **Position:** called by `cli::dispatch`; `window_command` uses `expand` for `--enqueue`;
//! runs `pipeline::run_job`; reads the settings file and the glossary through
//! `settings::services`, and a folder's videos through `job_queue::services::video_files`.
//!
//! **Signals and state:** reads the settings file, the videos' metadata, the folders' listings
//! and a glossary file when one is named; prints to stderr.
//!
//! **Invariants:** every path is checked before the first job starts; a video named is always
//! processed, a folder gives its videos without subtitles and fails when it has none; the first
//! failed job stops the run with its reason; the exit code is 0 when every job passed the
//! quality check and 2 when one did not; an option given on the command line wins over the
//! settings file.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Args, ValueEnum};
use job_model::StepName;
use job_model::job::{JobSettings, OutputFormat, Separator, WhisperModel};
use pipeline::progress::Progress;
use pipeline::workers::Binaries;
use pipeline::{CancelToken, JobOptions, JobOutcome, run_job};

use crate::job_queue::services::video_files;
use crate::settings::models::app_settings::AppSettings;
use crate::settings::services::{job_settings, settings_file};

/// The options that shape a job, which `--enqueue` refuses: the window's jobs take its settings.
const JOB_OPTIONS: [&str; 12] = [
    "settings",
    "work_root",
    "models_dir",
    "glossary",
    "audio_track",
    "separator",
    "whisper",
    "cut_score",
    "llm_model",
    "format",
    "rerun",
    "no_library",
];

/// The options of one `process` run.
#[derive(Debug, Args)]
pub(super) struct ProcessArgs {
    /// Videos and folders to process, one job per video, in order; a folder gives every video
    /// under it that has no subtitle file yet.
    #[arg(required = true)]
    pub(super) videos: Vec<PathBuf>,
    /// Queue the videos in the window instead and start its queue: the window already open takes
    /// them, or one opens minimized. Takes none of the options below.
    #[arg(long, conflicts_with_all = JOB_OPTIONS)]
    pub(super) enqueue: bool,
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
    /// Run without the sign library shared between episodes: no sign is looked up or recorded.
    #[arg(long)]
    pub(super) no_library: bool,
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

/// The videos `paths` name, in order and each once: a file as it is, a folder as every video
/// under it without a subtitle file; a missing path, and a folder with no such video, fail.
pub(super) fn expand(paths: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut videos: Vec<PathBuf> = Vec::new();
    for path in paths {
        let metadata = std::fs::metadata(path)
            .with_context(|| format!("cannot read the video {}", path.display()))?;
        let found = if metadata.is_dir() {
            let found = video_files::videos_under(path);
            anyhow::ensure!(
                !found.is_empty(),
                "the folder {} holds no video without subtitles",
                path.display()
            );
            found
        } else {
            anyhow::ensure!(
                metadata.is_file(),
                "{} is not a file or a folder",
                path.display()
            );
            vec![path.clone()]
        };
        for video in found {
            if !videos.contains(&video) {
                videos.push(video);
            }
        }
    }
    Ok(videos)
}

/// One job that ran to its end, and the rules its quality check found broken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Ran {
    pub(super) video: PathBuf,
    pub(super) qc_failures: Vec<String>,
}

/// The exit code of a run in which every job ran to its end, with the summary to print: 0 and
/// none when every job passed the quality check, 2 and the videos that did not otherwise.
pub(super) fn verdict(ran: &[Ran]) -> (u8, Option<String>) {
    let failed: Vec<String> = ran
        .iter()
        .filter(|job| !job.qc_failures.is_empty())
        .map(|job| name(&job.video))
        .collect();
    if failed.is_empty() {
        return (0, None);
    }
    let summary = format!(
        "{} of {} failed the quality check: {}",
        failed.len(),
        ran.len(),
        failed.join(", ")
    );
    (2, Some(summary))
}

/// The file name of `video`, or its whole path when it has none.
fn name(video: &Path) -> String {
    video.file_name().map_or_else(
        || video.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Run every video's job, stopping at the first that fails; the exit code says whether every
/// job passed the quality check.
pub(super) fn run(args: &ProcessArgs) -> anyhow::Result<ExitCode> {
    anyhow::ensure!(
        !args.enqueue,
        "`--enqueue` queues the videos in the window, which `process` does not open"
    );
    let videos = expand(&args.videos)?;
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
        library: if args.no_library {
            None
        } else {
            Some(pipeline::library::default_path()?)
        },
        models_dir: job_settings::models_dir(&chosen)?,
    };
    let missing = pipeline::models::missing(&options.models_dir, &options.settings);
    anyhow::ensure!(
        missing.is_empty(),
        "models missing from {}: {}; download them on the window's Settings page",
        options.models_dir.display(),
        missing.join(", ")
    );
    let mut ran = Vec::new();
    for video in &videos {
        let outcome = run_job(video, &options, &print)
            .with_context(|| format!("no subtitles for {}", video.display()))?;
        print_outcome(&outcome);
        ran.push(Ran {
            video: video.clone(),
            qc_failures: outcome.qc.failures(),
        });
    }
    let (code, summary) = verdict(&ran);
    if let Some(summary) = summary {
        eprintln!("{summary}");
    }
    Ok(ExitCode::from(code))
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
    let mib = |v: Option<f64>| v.map_or_else(|| "—".to_string(), |v| format!("{v:.0}"));
    eprintln!(
        "run:       {:.1} min wall time, whole-job peak RAM {} MiB",
        outcome.run.wall_s() / 60.0,
        mib(outcome.run.peak_ram_mib)
    );
}

/// One line per event on stderr.
/// The step and tenth of its work last printed, so a step that reports in steps of any size
/// prints once per tenth.
static PRINTED_TENTH: std::sync::Mutex<Option<(StepName, usize)>> = std::sync::Mutex::new(None);

/// Whether `done` of `total` reaches a tenth of `step`'s work not yet printed, or finishes it.
pub(super) fn enters_tenth(step: StepName, done: usize, total: usize) -> bool {
    let tenth = (done * 10).checked_div(total).unwrap_or(10);
    let mut printed = PRINTED_TENTH
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let new = done == total || *printed != Some((step, tenth));
    if new {
        *printed = Some((step, tenth));
    }
    new
}

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
            if enters_tenth(step, done, total) {
                eprintln!("    {step} {done}/{total}");
            }
        }
        Progress::StepMessage { step, text } => eprintln!("    {step}: {text}"),
        Progress::ModelCall { .. } => {}
        Progress::StepFinished { step, measure } => {
            let mib = |v: Option<f64>| v.map_or_else(|| "—".to_string(), |v| format!("{v:.0}"));
            eprintln!(
                "  ✓ {step} {:.1} s (RAM {} MiB, job RAM {} MiB, VRAM {} MiB)",
                measure.wall_s,
                mib(measure
                    .peak_ram_mib
                    .into_iter()
                    .chain(measure.peak_child_ram_mib)
                    .reduce(f64::max)),
                mib(measure.job_ram_mib),
                mib(measure.peak_vram_mib)
            );
        }
    }
}
