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
    let output = stages::output::subtitle_path(
        std::path::Path::new(&record.video),
        record.settings.effective_output_format(),
    );
    let mut text = stages::qc::markdown::render(&qc, record, &output.to_string_lossy(), &dropped);
    if record.settings.onscreen_text.enabled {
        let visual: job_model::onscreen::TextDocument =
            work_dir::read_json(&work.text(job_model::StepName::TextTypeset))?;
        let summary = visual.summary();
        let seconds: f64 = record
            .steps
            .iter()
            .filter(|(step, _)| step.stage() == job_model::StageName::OnscreenText)
            .map(|(_, done)| done.measure.wall_s)
            .sum();
        text.push_str(&format!("\n## On-screen text\n\n{} detected; {} translated; {} nearby; {} unresolved; {} flagged. Visual processing: {:.1} minutes.\n\n",summary.detected,summary.translated,summary.fallback,summary.unresolved,summary.flagged,seconds/60.0));
        for warning in &visual.review_warnings {
            text.push_str(&format!("- {warning}\n"));
        }
        for item in visual
            .occurrences
            .iter()
            .filter(|item| !item.warnings.is_empty() || item.english.is_none())
        {
            text.push_str(&format!(
                "- {} at {:.3}s: {}\n",
                item.id,
                item.start_s,
                item.warnings.join("; ")
            ));
        }
    }
    work_dir::write_text(&work.report(), &text)?;
    Ok(qc)
}
