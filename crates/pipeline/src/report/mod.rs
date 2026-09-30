//! The job report: `report.md` rendered from the quality check and the job record, written after
//! every run so its step table shows the latest times.
//!
//! **Role:** render the quality check, then the on-screen text section and, when the localized
//! video was written, its replacements, fallbacks, path and encoder.
//! **Position:** called by the runner at the end of every job; reads what the steps wrote.
//! **Signals and state:** reads `qc.json`, the dropped sounds, `visual/text_typeset.json`,
//! `visual/text_compose.json` and `visual/localized_video.json`; writes `report.md`.
//! **Invariants:** the localized-video lines appear only when that step ran without recording
//! itself disabled; the report is written whole through a part file.

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::{LocalizedVideoRecord, ReplaceStatus, ReplacementDocument};
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
        if localized_video_ran(record) {
            let composed: ReplacementDocument =
                work_dir::read_json(&work.text(StepName::TextCompose))?;
            let video: LocalizedVideoRecord =
                work_dir::read_json(&work.text(StepName::LocalizedVideo))?;
            text.push_str(&localized_lines(&composed, &video));
        }
    }
    work_dir::write_text(&work.report(), &text)?;
    Ok(qc)
}

/// Whether the localized-video step ran and wrote a video rather than recording itself disabled.
fn localized_video_ran(record: &JobRecord) -> bool {
    record
        .steps
        .get(&StepName::LocalizedVideo)
        .is_some_and(|step| !step.measure.notes.contains_key("disabled"))
}

/// The report's localized-video lines: what was replaced in the picture, what fell back to the
/// localized subtitles, and the file and encoder written.
fn localized_lines(composed: &ReplacementDocument, video: &LocalizedVideoRecord) -> String {
    let fallbacks = composed
        .texts
        .iter()
        .filter(|text| matches!(text.status, ReplaceStatus::Fallback(_)))
        .count();
    format!(
        "\n### Localized video\n\n- Occurrences replaced in the video: {}\n- Fallbacks to the localized subtitles: {}\n- Localized video: {}\n- Encoder: {}\n",
        video.replaced,
        fallbacks,
        video.path.as_deref().unwrap_or("none"),
        video.encoder
    )
}

#[cfg(test)]
#[path = "tests/report.rs"]
mod tests;
