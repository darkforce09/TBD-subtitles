//! A finished job's report read from its work directory: `job.json`, `qc.json` and
//! `output.json`.

use std::path::Path;

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::outputs::OutputRecord;
use job_model::report::QcReport;

use crate::job_report::models::report::JobReport;

/// The report of `video`'s job under `work_root`; an error names the file that is missing or
/// broken.
pub(crate) fn load(video: &Path, work_root: &Path) -> Result<JobReport, String> {
    let video = std::fs::canonicalize(video)
        .map_err(|e| format!("cannot find {}: {e}", video.display()))?;
    let work_dir = work_root.join(pipeline::work_dir::job_id(&video));
    let record: JobRecord = read(&work_dir.join("job.json"))?;
    let qc: QcReport = read(&work_dir.join("qc.json"))?;
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
        video,
        work_dir,
        subtitles,
        qc,
        steps,
    })
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))
}

#[cfg(test)]
#[path = "tests/report_loading.rs"]
mod tests;
