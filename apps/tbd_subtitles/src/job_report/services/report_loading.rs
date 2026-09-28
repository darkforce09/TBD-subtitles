//! A finished job's report read from its work directory: `job.json`, `qc.json`, `output.json`
//! and the owner's `review.json`; and the summary its sidebar row shows.
//!
//! **Role:** find the video's work directory as the pipeline names it, read its files, and count
//! its problems and lines worth a listen through `line_counts`.
//!
//! **Position:** called by the application when a finished job is selected or ends, and for the
//! summary of every finished row when the window opens and after each run or correction.
//!
//! **Signals and state:** none; reads files only.
//!
//! **Invariants:** a missing or broken file is an error naming it, but for `output.json`, whose
//! path the settings give, and `review.json`, which exists only once the owner corrected a line.

use std::path::{Path, PathBuf};

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::outputs::{Corrections, OutputRecord};
use job_model::report::QcReport;

use crate::job_report::models::report::JobReport;
use crate::job_report::models::summary::RowSummary;
use crate::job_report::services::line_counts;

/// The report of `video`'s job under `work_root`; an error names the file that is missing or
/// broken.
pub(crate) fn load(video: &Path, work_root: &Path) -> Result<JobReport, String> {
    let (video, work_dir) = work_dir(video, work_root)?;
    let record: JobRecord = read(&work_dir.join("job.json"))?;
    let qc: QcReport = read(&work_dir.join("qc.json"))?;
    let corrections = corrections(&work_dir)?;
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
    Ok(JobReport {
        report_file: work_dir.join("report.md"),
        problems: line_counts::problems(&qc),
        lines: line_counts::line_counts(&qc, &corrections),
        video,
        work_dir,
        subtitles,
        qc,
        corrections,
        steps,
    })
}

/// What the sidebar row of `video`'s finished job under `work_root` says, from its `qc.json` and
/// `review.json` alone; an error names the file that is missing or broken.
pub(crate) fn summary(video: &Path, work_root: &Path) -> Result<RowSummary, String> {
    let (_, work_dir) = work_dir(video, work_root)?;
    let qc: QcReport = read(&work_dir.join("qc.json"))?;
    Ok(line_counts::summary(&qc, &corrections(&work_dir)?))
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

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))
}

#[cfg(test)]
#[path = "tests/report_loading.rs"]
mod tests;
