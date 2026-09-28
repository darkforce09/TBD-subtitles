//! `tbd-subtitles fix <video>`: Fix It without a window, then the correction run that times its
//! changes.
//!
//! **Role:** put the options over the settings file, run Fix It on the video's finished job with
//! the chosen `claude` model, print each pass and what became of the lines, then run the job
//! again with its own settings so the review step and the steps after it put the changes into the
//! subtitle file.
//!
//! **Position:** called by `cli::dispatch`; runs `pipeline::fix_it::fix_video` and
//! `pipeline::run_job`; shares the printout of `process_command`.
//!
//! **Signals and state:** reads the settings file and the job's `job.json`; prints to stderr.
//!
//! **Invariants:** an option given on the command line wins over the settings file; the
//! correction run takes the settings the job ran with, never the ones saved now; nothing runs
//! again when Fix It changed no line.

use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use job_model::job::JobRecord;
use job_model::outputs::FixVerdict;
use pipeline::fix_it::{FixOptions, FixOutcome, FixProgress, FixStage, fix_video};
use pipeline::workers::Binaries;
use pipeline::{CancelToken, JobOptions, run_job};

use super::process_command;
use crate::settings::models::app_settings::AppSettings;
use crate::settings::services::{job_settings, settings_file};

/// The options of one `fix` run.
#[derive(Debug, Args)]
pub(super) struct FixArgs {
    /// The video whose finished subtitles Fix It fixes.
    pub(super) video: PathBuf,
    /// The settings file; default: `~/.config/tbd-subtitles/settings.toml`. The options below
    /// win over it.
    #[arg(long)]
    settings: Option<PathBuf>,
    /// The folder that holds the jobs' work directories.
    #[arg(long)]
    work_root: Option<PathBuf>,
    /// The `claude` model Fix It asks; default: the settings' Fix It model (`opus`).
    #[arg(long)]
    model: Option<String>,
    /// How many `claude` calls run at once.
    #[arg(long)]
    processes: Option<usize>,
}

/// The settings file's values with these options put over them.
pub(super) fn merged(mut file: AppSettings, args: &FixArgs) -> AppSettings {
    if let Some(root) = &args.work_root {
        file.work_root = Some(root.clone());
    }
    if let Some(model) = &args.model {
        file.language_model.fix_model = model.clone();
    }
    if let Some(processes) = args.processes {
        file.language_model.processes = processes;
    }
    file
}

/// Run Fix It on the video, then the correction run when it changed a line.
pub(super) fn run(args: &FixArgs) -> anyhow::Result<()> {
    let metadata = std::fs::metadata(&args.video)
        .with_context(|| format!("cannot read the video {}", args.video.display()))?;
    anyhow::ensure!(metadata.is_file(), "{} is not a file", args.video.display());
    let path = match &args.settings {
        Some(path) => path.clone(),
        None => settings_file::default_path()?,
    };
    let chosen = merged(settings_file::load(&path)?, args);
    let options = FixOptions {
        work_root: job_settings::work_root(&chosen)?,
        model: chosen.language_model.fix_model.clone(),
        glossary_name: job_settings::glossary_name(&chosen),
        processes: chosen.language_model.processes.max(1),
        cancel: CancelToken::new(),
    };
    eprintln!("fix {} with claude {}", args.video.display(), options.model);
    let outcome = fix_video(&args.video, &options, &print)
        .with_context(|| format!("Fix It could not fix {}", args.video.display()))?;
    print_outcome(&outcome);
    if outcome.changed.is_empty() {
        return Ok(());
    }
    let record: JobRecord = pipeline::work_dir::read_json(&outcome.work_dir.join("job.json"))?;
    let job = JobOptions {
        work_root: options.work_root.clone(),
        settings: record.settings,
        rerun: Vec::new(),
        binaries: Binaries::beside_current_exe()?,
        cancel: CancelToken::new(),
        gpu_lock: pipeline::work_dir::gpu_lock_path()?,
        models_dir: job_settings::models_dir(&chosen)?,
    };
    let finished = run_job(&args.video, &job, &process_command::print)
        .with_context(|| format!("the changes did not reach {}", args.video.display()))?;
    process_command::print_outcome(&finished);
    Ok(())
}

/// One line per pass and a line per call as it ends.
fn print(step: FixProgress) {
    let pass = match step.stage {
        FixStage::Reading => "reading the whole video".to_string(),
        FixStage::Fixing(family) => format!("fixing {}", family.describe()),
        FixStage::Checking => "checking each change".to_string(),
        FixStage::Saving => "saving the changes".to_string(),
    };
    eprintln!("  {pass} {}/{}", step.done, step.total);
}

/// What the model worked out, what became of each line, and what the calls cost.
fn print_outcome(outcome: &FixOutcome) {
    let record = &outcome.record;
    let brief = &record.brief;
    eprintln!("brief:     {} · {}", brief.show, brief.episode);
    eprintln!("cast:      {}", brief.cast.join(", "));
    for line in &record.lines {
        let verdict = match &line.verdict {
            FixVerdict::Unchanged => "unchanged".to_string(),
            FixVerdict::Kept { why } => format!("kept: {why}"),
            FixVerdict::Accepted { why } => format!("accepted: {why}"),
            FixVerdict::TurnedDown { why } => format!("turned down: {why}"),
            FixVerdict::NotJudged { why } => format!("not judged: {why}"),
            FixVerdict::NotAnswered { why } => format!("not answered: {why}"),
        };
        eprintln!("  {} {verdict}", line.id);
        if line.changed() {
            eprintln!("    \"{}\" → \"{}\"", line.before_text, line.after_text);
        }
        for refused in &line.refused {
            eprintln!("    refused: {refused}");
        }
    }
    eprintln!(
        "changed:   {} line(s); {} kept your own correction",
        outcome.changed.len(),
        outcome.kept_yours.len()
    );
    eprintln!(
        "calls:     {} ({} from an earlier run, {} failed), ${:.2}",
        record.calls,
        record.cached_calls,
        record.failed_calls.len(),
        record.cost_usd
    );
    for failure in &record.failed_calls {
        eprintln!("  failed: {failure}");
    }
}
