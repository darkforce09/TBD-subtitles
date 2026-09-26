//! The job report: `report.md` rendered from the quality check and the job record, written after
//! every run so its step table shows the latest times.

use job_model::job::JobRecord;
use job_model::report::QcReport;

use crate::error::Result;
use crate::work_dir::{self, WorkDir};

/// Write `report.md` and return the quality check it shows.
pub fn write(work: &WorkDir, record: &JobRecord) -> Result<QcReport> {
    let qc: QcReport = work_dir::read_json(&work.qc())?;
    let dropped: Vec<String> = work_dir::read_json(&work.dropped_sounds()).unwrap_or_default();
    let output = stages::output::subtitle_path(std::path::Path::new(&record.video));
    let text = stages::qc::markdown::render(&qc, record, &output.to_string_lossy(), &dropped);
    work_dir::write_text(&work.report(), &text)?;
    Ok(qc)
}
