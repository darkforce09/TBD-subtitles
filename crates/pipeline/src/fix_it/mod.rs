//! Fix It on a finished job: a stronger `claude` model fixes the flagged lines, and what it kept
//! becomes corrections a correction run then times.
//!
//! **Role:** read the job (refusing one that is not ready), run `stages::fix_it` with a `claude`
//! backend that the cancel token stops and whose calls take a slot at the shared call gate, keep
//! every answered call so a stopped run resumes without paying again, write `fix.json` with this run's lines and the earlier runs' answers it did not
//! ask again, and put this run's kept changes into `review.json` without touching a line the
//! owner settled.
//!
//! **Position:** called by the window's Fix It and the `fix` subcommand; uses `inputs.rs`,
//! `cache.rs` and `merge.rs`. The caller queues the correction run that times the changes.
//!
//! **Signals and state:** holds the job's `JobStore` while it runs; reads the work directory; writes `fix.json`,
//! `fix/calls/` and `review.json`; starts `claude` processes in `claude-cwd/`.
//!
//! **Invariants:** each model is `CachedModel(Gated(ClaudeCli))`, so an answer kept on disk never
//! takes a slot at the gate; a stopped or failed run changes no correction; the owner's corrections always
//! win; only this run's lines are merged; `fix.json` keeps the problems before the first run and
//! the calls and cost of every run since the job was last adjudicated, and a record from before
//! that counts for nothing; `fix/calls/` goes once a run has written its corrections.

mod cache;
mod inputs;
mod merge;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use inference::llm::LanguageModel;
use inference::llm::call_gate::{CallSeat, Gated};
use inference::llm::claude_cli::ClaudeCli;
use job_model::StepName;
use job_model::outputs::{FixBefore, FixFamily, FixRecord, LineFix};
use stages::fix_it::{self, FixFailure, Make, Pass};

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};
use crate::work_dir::{self, JobStore, WorkDir};

pub use merge::{Merged, merge};

/// How to run Fix It.
#[derive(Debug, Clone)]
pub struct FixOptions {
    /// The folder that holds every job's work directory.
    pub work_root: PathBuf,
    /// The `claude` model, such as `opus`.
    pub model: String,
    /// The glossary's name as the settings give it, such as `one_piece`.
    pub glossary_name: String,
    /// Calls at once.
    pub processes: usize,
    /// The seat this run's `claude` calls take at the shared gate.
    pub calls: CallSeat,
    /// Stops the run: no call starts and the running `claude` processes are killed.
    pub cancel: CancelToken,
}

/// Where a run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixStage {
    /// The model reads the whole video for its context.
    Reading,
    /// The model fixes one family of problems.
    Fixing(FixFamily),
    /// The judge checks each change.
    Checking,
    /// The kept changes go into the corrections.
    Saving,
}

impl FixStage {
    /// The stage's place among the three passes, from 1.
    pub fn pass(self) -> usize {
        match self {
            FixStage::Reading => 1,
            FixStage::Fixing(_) => 2,
            FixStage::Checking | FixStage::Saving => 3,
        }
    }
}

/// A step of a run: the stage and its `(done, total)` calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixProgress {
    pub stage: FixStage,
    pub done: usize,
    pub total: usize,
}

/// What a finished run left.
#[derive(Debug, Clone, PartialEq)]
pub struct FixOutcome {
    pub work_dir: PathBuf,
    pub record: FixRecord,
    /// The lines whose Fix It correction went into `review.json`.
    pub changed: Vec<String>,
    /// The lines Fix It would have changed that the owner corrected meanwhile.
    pub kept_yours: Vec<String>,
}

/// Run Fix It on `video`'s job with the `claude` CLI.
pub fn fix_video(
    video: &Path,
    options: &FixOptions,
    progress: &(dyn Fn(FixProgress) + Sync),
) -> Result<FixOutcome> {
    let video = std::fs::canonicalize(video).context(format!("cannot find {}", video.display()))?;
    let work = WorkDir::new(options.work_root.join(work_dir::job_id(&video)));
    let (model, cwd, flag, seat) = (
        options.model.clone(),
        work.claude_cwd(),
        options.cancel.flag(),
        options.calls.clone(),
    );
    let make = move || {
        let claude = ClaudeCli::new(&model, cwd.clone()).with_cancel(flag.clone());
        Box::new(Gated::new(claude, seat.clone(), flag.clone())) as Box<dyn LanguageModel + Send>
    };
    fix_job(&video, &work, options, &make, progress)
}

/// Run Fix It on the job in `work` for `video`, with models from `make`.
pub fn fix_job(
    video: &Path,
    work: &WorkDir,
    options: &FixOptions,
    make: &Make<'_>,
    progress: &(dyn Fn(FixProgress) + Sync),
) -> Result<FixOutcome> {
    // Shares the window's handle when a job of this video runs in this process.
    let _store = JobStore::open(work)?;
    let inputs = inputs::load(work, video, &options.glossary_name)?;
    let glossary = inputs.glossary();
    let episode = inputs.episode(&glossary);
    let hits = Arc::new(AtomicUsize::new(0));
    let cached = || {
        Box::new(cache::CachedModel::new(
            make(),
            work.fix_calls(),
            &options.model,
            hits.clone(),
        )) as Box<dyn LanguageModel + Send>
    };
    let flag = options.cancel.flag();
    let run = fix_it::run(
        &episode,
        &cached,
        options.processes,
        &|pass, done, total| {
            let stage = match pass {
                Pass::Reading => FixStage::Reading,
                Pass::Fixing(family) => FixStage::Fixing(family),
                Pass::Checking => FixStage::Checking,
            };
            progress(FixProgress { stage, done, total })
        },
        &flag,
    );
    let run = match run {
        Ok(run) => run,
        Err(FixFailure::Stopped) => return Err(PipelineError::cancelled("Fix It")),
        Err(FixFailure::Brief(why)) => return Err(PipelineError::new("Fix It", why)),
    };
    let usage = run.usage;
    let earlier = inputs.earlier.as_ref();
    let mut this_run = FixRecord {
        model: options.model.clone(),
        video: inputs.video_name.clone(),
        brief: run.brief,
        lines: run.lines,
        calls: usage.calls,
        cached_calls: hits.load(std::sync::atomic::Ordering::SeqCst),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cost_usd: usage.cost_usd,
        failed_calls: usage.failed,
        adjudication: inputs
            .record
            .steps
            .get(&StepName::Readjudicate)
            .map(|step| step.fingerprint.clone())
            .unwrap_or_default(),
        before: Some(
            earlier
                .and_then(|e| e.before.clone())
                .unwrap_or_else(|| FixBefore::of(&inputs.qc)),
        ),
    };
    progress(FixProgress {
        stage: FixStage::Saving,
        done: 0,
        total: 1,
    });
    work_dir::write_json(&work.fix_record(), &carried(this_run.clone(), earlier))?;
    let (_, merged) = work_dir::update_corrections(work, |corrections| {
        merge(corrections, &mut this_run.lines, &options.model)
    })?;
    let record = carried(this_run, earlier);
    work_dir::write_json(&work.fix_record(), &record)?;
    let _ = std::fs::remove_dir_all(work.fix_calls());
    progress(FixProgress {
        stage: FixStage::Saving,
        done: 1,
        total: 1,
    });
    Ok(FixOutcome {
        work_dir: work.root().to_path_buf(),
        record,
        changed: merged.applied,
        kept_yours: merged.kept_yours,
    })
}

/// `run` with the lines `earlier` holds that `run` did not ask about, every line by id, and the
/// calls, tokens, cost and failures of both.
fn carried(mut run: FixRecord, earlier: Option<&FixRecord>) -> FixRecord {
    if let Some(earlier) = earlier {
        let asked: HashSet<&str> = run.lines.iter().map(|l| l.id.as_str()).collect();
        let kept: Vec<LineFix> = earlier
            .lines
            .iter()
            .filter(|l| !asked.contains(l.id.as_str()))
            .cloned()
            .collect();
        run.lines.extend(kept);
        run.calls += earlier.calls;
        run.cached_calls += earlier.cached_calls;
        run.input_tokens += earlier.input_tokens;
        run.output_tokens += earlier.output_tokens;
        run.cost_usd += earlier.cost_usd;
        let failed = std::mem::take(&mut run.failed_calls);
        run.failed_calls = earlier.failed_calls.iter().cloned().chain(failed).collect();
    }
    run.lines.sort_by(|a, b| a.id.cmp(&b.id));
    run
}

#[cfg(test)]
#[path = "tests/fix_it.rs"]
mod tests;
