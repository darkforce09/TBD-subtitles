//! What Fix It reads from a finished job, and whether the job is ready for it.
//!
//! **Role:** check that the job finished and that its subtitles already hold every correction,
//! then read the sheet with the re-decoded alternatives, the lines as the subtitles have them, the
//! corrections, the quality check, the timing, the main engine's words, and the earlier runs'
//! record with what they answered.
//!
//! **Position:** called by `fix_it::fix_job` under the job lock.
//!
//! **Signals and state:** reads the work directory only.
//!
//! **Invariants:** a job whose quality check or output step has not finished, or whose
//! corrections changed since its last run, is refused with a message the owner can act on; an
//! earlier record that is missing, unreadable or not current counts as no record.

use std::path::Path;

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::outputs::{
    AdjudicationPass, Aligned, Corrections, EngineTranscript, FixRecord, Line, Redecode, Utterance,
};
use job_model::report::QcReport;
use stages::adjudication::redecode;
use stages::fix_it::Episode;
use stages::fix_it::items::Answered;

use crate::error::{PipelineError, Result};
use crate::tasks::corrected_lines;
use crate::work_dir::{self, WorkDir};

/// Everything Fix It reads, owned.
pub struct Inputs {
    pub record: JobRecord,
    pub video_name: String,
    pub folder_name: String,
    pub glossary_name: String,
    pub sheet: Vec<Utterance>,
    pub lines: Vec<Line>,
    pub corrections: Corrections,
    pub qc: QcReport,
    pub timing: Aligned,
    pub heard: EngineTranscript,
    /// The earlier runs' `fix.json`, when it belongs to the job's re-adjudication as it stands.
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

/// Read what Fix It needs about `video`'s job in `work`, refusing a job that is not ready.
pub fn load(work: &WorkDir, video: &Path, glossary_name: &str) -> Result<Inputs> {
    let context = "Fix It";
    let record: JobRecord = work_dir::read_json(&work.job_json()).map_err(|_| {
        PipelineError::new(context, "the video has no finished subtitles to fix yet")
    })?;
    let finished = [StepName::Qc, StepName::Output]
        .iter()
        .all(|step| record.steps.contains_key(step));
    if !finished {
        return Err(PipelineError::new(
            context,
            "the video has no finished subtitles to fix yet",
        ));
    }
    if record.corrections != work_dir::corrections_digest(work) {
        return Err(PipelineError::new(
            context,
            "your latest corrections are not in the subtitles yet; wait until they are updated",
        ));
    }
    let corrections = work_dir::read_corrections(work)?;
    let sheet: Vec<Utterance> = work_dir::read_json(&work.sheet())?;
    let redecoded = |engine: &str| work_dir::read_json::<Redecode>(&work.redecode(engine)).ok();
    let (parakeet, whisper) = (redecoded("parakeet"), redecoded("whisper"));
    let mut alternatives = Vec::new();
    if let Some(p) = &parakeet {
        alternatives.push(("p", p));
    }
    if let Some(w) = &whisper {
        alternatives.push(("w", w));
    }
    let sheet = redecode::with_alternatives(&sheet, &alternatives);
    let adjudicated: AdjudicationPass = work_dir::read_json(&work.adjudicated())?;
    let name = |path: Option<&std::ffi::OsStr>| {
        path.map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let earlier = work_dir::read_json::<FixRecord>(&work.fix_record())
        .ok()
        .filter(|fix| fix.is_current(&record));
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
        qc: work_dir::read_json(&work.qc())?,
        timing: work_dir::read_json(&work.reviewed())?,
        heard: work_dir::read_json(&work.asr("parakeet"))?,
        earlier,
        answered,
        record,
    })
}
