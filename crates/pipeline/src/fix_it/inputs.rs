//! What Fix It reads from a finished job, and whether the job is ready for it.
//!
//! **Role:** check that the job finished and that its subtitles already hold every correction,
//! then read the sheet with the re-decoded alternatives, the lines as the subtitles have them, the
//! corrections, the quality check, the timing, the main engine's words, and the earlier runs'
//! record with what they answered.
//!
//! **Position:** called by `fix_it::fix_job` while it holds the job's store.
//!
//! **Signals and state:** reads the job record, the step records, the line corrections, Fix It's
//! record and the steps' documents from one snapshot of the job's store.
//!
//! **Invariants:** a job whose quality check or output step has not finished, or whose
//! corrections changed since its last run, is refused with a message the owner can act on; an
//! earlier record that is missing, unreadable or not current counts as no record; a finished job
//! whose store lacks a step document Fix It reads is refused, naming the step.

use std::path::Path;

use job_model::StepName;
use job_model::job::{JobRecord, StepRecords};
use job_model::outputs::{
    AdjudicationPass, Aligned, Corrections, EngineTranscript, FixRecord, Line, Redecode, Utterance,
};
use job_model::report::QcReport;
use stages::adjudication::redecode;
use stages::fix_it::Episode;
use stages::fix_it::items::Answered;

use crate::error::{PipelineError, Result};
use crate::tasks::corrected_lines;
use crate::work_dir::JobStore;
use crate::work_dir::corrections::digest_in;
use crate::work_dir::store::keys;

/// Everything Fix It reads, owned.
pub struct Inputs {
    pub record: JobRecord,
    pub steps: StepRecords,
    pub video_name: String,
    pub folder_name: String,
    pub glossary_name: String,
    pub sheet: Vec<Utterance>,
    pub lines: Vec<Line>,
    pub corrections: Corrections,
    pub qc: QcReport,
    pub timing: Aligned,
    pub heard: EngineTranscript,
    /// The earlier runs' record, when it belongs to the job's re-adjudication as it stands.
    pub earlier: Option<FixRecord>,
    /// What the earlier runs answered.
    pub answered: Answered,
}

impl Inputs {
    /// The episode the stage reads, over `glossary`.
    pub fn episode<'a>(&'a self, glossary: &'a [&'a str]) -> Episode<'a> {
        Episode {
            video_name: &self.video_name,
            folder_name: &self.folder_name,
            glossary_name: &self.glossary_name,
            glossary,
            sheet: &self.sheet,
            lines: &self.lines,
            corrections: &self.corrections,
            qc: &self.qc,
            timing: &self.timing,
            heard: &self.heard,
            answered: &self.answered,
        }
    }

    /// The job's glossary terms.
    pub fn glossary(&self) -> Vec<&str> {
        self.record
            .settings
            .glossary
            .iter()
            .map(String::as_str)
            .collect()
    }
}

/// Read what Fix It needs about `video`'s job in `store`, refusing a job that is not ready.
pub fn load(store: &JobStore, video: &Path, glossary_name: &str) -> Result<Inputs> {
    let context = "Fix It";
    let read = store.read()?;
    let not_ready =
        || PipelineError::new(context, "the video has no finished subtitles to fix yet");
    let record: JobRecord = read.job_record()?.ok_or_else(not_ready)?;
    let steps = read.step_records()?;
    let finished = [StepName::Qc, StepName::Output]
        .iter()
        .all(|step| steps.contains_key(step));
    if !finished {
        return Err(PipelineError::new(
            context,
            "the video has no finished subtitles to fix yet",
        ));
    }
    if record.corrections != digest_in(&read, keys::LINE_CORRECTIONS)? {
        return Err(PipelineError::new(
            context,
            "your latest corrections are not in the subtitles yet; wait until they are updated",
        ));
    }
    let output = |step: StepName| missing(context, step);
    let corrections = read.line_corrections()?;
    let sheet: Vec<Utterance> = read
        .output(StepName::DiffSheet, None)?
        .ok_or_else(|| output(StepName::DiffSheet))?;
    let parakeet: Redecode = read
        .output(StepName::RedecodeParakeet, None)?
        .ok_or_else(|| output(StepName::RedecodeParakeet))?;
    let whisper: Redecode = read
        .output(StepName::RedecodeWhisper, None)?
        .ok_or_else(|| output(StepName::RedecodeWhisper))?;
    let sheet = redecode::with_alternatives(&sheet, &[("p", &parakeet), ("w", &whisper)]);
    let adjudicated: AdjudicationPass = read
        .output(StepName::Readjudicate, None)?
        .ok_or_else(|| output(StepName::Readjudicate))?;
    let name = |path: Option<&std::ffi::OsStr>| {
        path.map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let earlier = read
        .fix_record()
        .ok()
        .flatten()
        .filter(|fix: &FixRecord| fix.is_current(&steps));
    let answered = earlier
        .as_ref()
        .map_or_else(Answered::none, Answered::from_record);
    Ok(Inputs {
        video_name: name(video.file_stem()),
        folder_name: name(video.parent().and_then(Path::file_name)),
        glossary_name: glossary_name.to_string(),
        lines: corrected_lines(&adjudicated.lines, &corrections),
        corrections,
        sheet,
        qc: read
            .output(StepName::Qc, None)?
            .ok_or_else(|| output(StepName::Qc))?,
        timing: read
            .output(StepName::Review, None)?
            .ok_or_else(|| output(StepName::Review))?,
        heard: read
            .output(StepName::AsrParakeet, None)?
            .ok_or_else(|| output(StepName::AsrParakeet))?,
        earlier,
        answered,
        record,
        steps,
    })
}

/// The error of a finished job whose `step` document is not in its store.
fn missing(context: &str, step: StepName) -> PipelineError {
    PipelineError::new(
        context,
        format!("the job database holds no {step} document; process the video again"),
    )
}
