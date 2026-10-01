//! The rows of a finished job the probes read, from its database.
//!
//! **Role:** read a job's source video, models folder and probed video stream, one on-screen text
//! or replacement document, and the per-frame shifts of its `frames` rows, each in one read of the
//! job's database.
//! **Position:** used by `mask_probe` and `verify_probe`.
//! **Signals and state:** opens the job's database for the length of each read; a job another
//! process runs is a busy error.
//! **Invariants:** a missing database, row or video stream is an error naming it; nothing is
//! written.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use job_model::StepName;
use job_model::onscreen::{FrameRecord, ReplacementDocument, TextDocument};
use job_model::outputs::{ProbeDecoded, VideoStream};
use stages::localize::motion::Motion;
use worker_channel::address::Table;

/// The job's source video, its models folder when the job names one, and its video stream.
pub struct JobSource {
    pub video: PathBuf,
    pub models: Option<PathBuf>,
    pub stream: VideoStream,
}

/// The source of the job in `work`.
pub fn source(work: &Path) -> Result<JobSource> {
    let (job, probe) = pipeline::work_dir::read_stored(work, |read| {
        Ok((
            read.job_record()?,
            read.output::<ProbeDecoded>(StepName::ProbeDecode, None)?,
        ))
    })?
    .with_context(|| no_database(work))?;
    let job = job.context("the job has no job record")?;
    let stream = probe
        .context("the job has no probe")?
        .probe
        .video
        .context("the probe has no video stream")?;
    Ok(JobSource {
        video: PathBuf::from(job.video),
        models: job.models_dir.map(PathBuf::from),
        stream,
    })
}

/// The on-screen text document `step` of the job in `work` stored.
pub fn text(work: &Path, step: StepName) -> Result<TextDocument> {
    pipeline::work_dir::read_stored(work, |read| read.output(step, None))?
        .with_context(|| no_database(work))?
        .with_context(|| format!("the job has no {step} document"))
}

/// The replacement document `step` of the job in `work` stored.
pub fn replacements(work: &Path, step: StepName) -> Result<ReplacementDocument> {
    pipeline::work_dir::read_stored(work, |read| read.output(step, None))?
        .with_context(|| no_database(work))?
        .with_context(|| format!("the job has no {step} document"))
}

/// The writing's shift in each frame of the job in `work`, from its `frames` rows, read one at a
/// time.
pub fn motion(work: &Path) -> Result<Motion> {
    pipeline::work_dir::read_stored(work, |read| {
        let mut motion = Motion::default();
        read.rows_as::<FrameRecord>(Table::Frames, None, |occurrence, frame, row| {
            motion.add(occurrence, frame, row.plate, row.shift);
            Ok(())
        })?;
        Ok(motion)
    })?
    .with_context(|| no_database(work))
}

fn no_database(work: &Path) -> String {
    format!("{} holds no job database", work.display())
}
