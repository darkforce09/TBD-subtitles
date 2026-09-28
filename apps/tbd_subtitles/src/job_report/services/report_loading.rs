//! A finished job's report read from its work directory: `job.json`, `qc.json`, `output.json`,
//! the owner's `review.json` and Fix It's `fix.json`; and the summary its sidebar row shows.
//!
//! **Role:** find the video's work directory as the pipeline names it, read its files, and count
//! its problems and lines worth a listen through `line_counts`, and what Fix It did through
//! `fix_result`.
//!
//! **Position:** called by the application when a finished job is selected or ends, and for the
//! summary of every finished row when the window opens and after each run or correction.
//!
//! **Signals and state:** none; reads files only.
//!
//! **Invariants:** a missing or broken file is an error naming it, but for `output.json`, whose
//! path the settings give, `review.json`, which exists only once a line was corrected, and
//! `fix.json`, which exists only once Fix It ran; a Fix It record counts only while it belongs to
//! the job's re-adjudication as it stands (`FixRecord::is_current`).

use std::path::{Path, PathBuf};

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::outputs::{Corrections, FixRecord, OutputRecord};
use job_model::report::QcReport;
use pipeline::work_dir::WorkDir;
use stages::fix_it::items::Answered;

use crate::job_report::models::report::JobReport;
use crate::job_report::models::summary::RowSummary;
use crate::job_report::services::{fix_result, line_counts};

/// The report of `video`'s job under `work_root`; an error names the file that is missing or
/// broken.
pub(crate) fn load(video: &Path, work_root: &Path) -> Result<JobReport, String> {
    let (video, work_dir) = work_dir(video, work_root)?;
    let record: JobRecord = read(&work_dir.join("job.json"))?;
    let qc: QcReport = read(&work_dir.join("qc.json"))?;
    let corrections = corrections(&work_dir)?;
    let fix = current_fix(&work_dir, Some(&record))?;
    let answered = answered(fix.as_ref());
    let subtitles = read::<OutputRecord>(&work_dir.join("output.json"))
        .map(|output| output.path.into())
        .unwrap_or_else(|_| stages::output::subtitle_path(&video, record.settings.output_format));
    let steps = StepName::ALL
        .iter()
        .filter_map(|step| {
            record
                .steps
                .get(step)
                .map(|done| (*step, done.measure.clone()))
        })
        .collect();
    let problems = line_counts::problems(&qc);
    let fix_result = fix
        .as_ref()
        .filter(|fix| answered_any(fix))
        .map(|fix| fix_result::fix_result(fix, &corrections, &problems));
    Ok(JobReport {
        report_file: work_dir.join("report.md"),
        lines: line_counts::line_counts(&qc, &corrections, &answered),
        fixable: line_counts::fixable(&qc, &corrections, &answered),
        problems,
        fix_result,
        video,
        work_dir,
        subtitles,
        qc,
        corrections,
        steps,
    })
}

/// What the sidebar row of `video`'s finished job under `work_root` says, from its `qc.json`,
/// `review.json` and `fix.json` (with `job.json` to tell whether the Fix It record is current);
/// an error names the file that is missing or broken.
pub(crate) fn summary(video: &Path, work_root: &Path) -> Result<RowSummary, String> {
    let (_, work_dir) = work_dir(video, work_root)?;
    let qc: QcReport = read(&work_dir.join("qc.json"))?;
    let fix = current_fix(&work_dir, None)?;
    Ok(line_counts::summary(
        &qc,
        &corrections(&work_dir)?,
        &answered(fix.as_ref()),
        fix.as_ref().is_some_and(answered_any),
    ))
}

/// The canonical video and its job's work directory, as the pipeline names it.
fn work_dir(video: &Path, work_root: &Path) -> Result<(PathBuf, PathBuf), String> {
    let video = std::fs::canonicalize(video)
        .map_err(|e| format!("cannot find {}: {e}", video.display()))?;
    let work_dir = work_root.join(pipeline::work_dir::job_id(&video));
    Ok((video, work_dir))
}

/// The owner's corrections: none while there is no `review.json`.
fn corrections(work_dir: &Path) -> Result<Corrections, String> {
    let path = work_dir.join("review.json");
    if path.is_file() {
        read(&path)
    } else {
        Ok(Corrections::default())
    }
}

/// Fix It's record while it belongs to the job as it stands, `job` read from `job.json` when not
/// given; none while there is no `fix.json`.
fn current_fix(work_dir: &Path, job: Option<&JobRecord>) -> Result<Option<FixRecord>, String> {
    let path = WorkDir::new(work_dir).fix_record();
    if !path.is_file() {
        return Ok(None);
    }
    let fix: FixRecord = read(&path)?;
    let current = match job {
        Some(job) => fix.is_current(job),
        None => fix.is_current(&read(&work_dir.join("job.json"))?),
    };
    Ok(current.then_some(fix))
}

/// The lines `fix` answered; none without a record.
fn answered(fix: Option<&FixRecord>) -> Answered {
    fix.map_or_else(Answered::none, Answered::from_record)
}

/// Whether `fix`'s runs answered at least one line.
fn answered_any(fix: &FixRecord) -> bool {
    fix.lines.iter().any(|line| line.verdict.answered())
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))
}

#[cfg(test)]
#[path = "tests/report_loading.rs"]
mod tests;
