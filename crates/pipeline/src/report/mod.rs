//! The job report: `report.md` rendered from the quality check, the job record and the step
//! records, written after every run so its step table shows the latest times.
//!
//! **Role:** render the quality check, then the on-screen text section and, when the localized
//! video was written, its replacements, fallbacks, path, encoder, and the segments re-encoded
//! and frames copied or the reason the whole video was re-encoded.
//! **Position:** called by the runner at the end of every job; reads what the steps stored.
//! **Signals and state:** reads `meta/last_run`, `outputs/qc`, `outputs/cues/dropped_sounds`,
//! `outputs/text_typeset`, `outputs/text_verify` and `outputs/localized_video` from one snapshot
//! of the job's store, through the process's one handle of it; writes `report.md`.
//! **Invariants:** the localized-video lines appear only when that step ran without recording
//! itself disabled; a document the report reads and does not find is an error naming it; the
//! report is written whole through a part file.

use job_model::StepName;
use job_model::job::{JobRecord, StepRecords};
use job_model::onscreen::{
    LocalizedVideoRecord, ReplaceStatus, ReplacementDocument, TextDocument, VerifiedReplacements,
};
use job_model::report::QcReport;
use rkyv::api::high::{HighDeserializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::Error as ArchiveError;

use crate::error::{PipelineError, Result};
use crate::work_dir::store::{StoreRead, keys};
use crate::work_dir::{self, JobStore, WorkDir};

/// Write `report.md` from the job's record and its step records, and return the quality check
/// it shows.
pub fn write(work: &WorkDir, record: &JobRecord, steps: &StepRecords) -> Result<QcReport> {
    // The runner's handle: a job runs in one process, which shares one handle of its database.
    let read = JobStore::open(work)?.read()?;
    let qc: QcReport = stored(&read, StepName::Qc, None)?;
    let dropped: Vec<String> = stored(&read, StepName::Cues, Some(keys::DROPPED_SOUNDS))?;
    let output = stages::output::subtitle_path(
        std::path::Path::new(&record.video),
        record.settings.effective_output_format(),
    );
    let run = read.job_run()?;
    let mut text = stages::qc::markdown::render(
        &qc,
        record,
        steps,
        run.as_ref(),
        &output.to_string_lossy(),
        &dropped,
    );
    if record.settings.onscreen_text.enabled {
        let visual: TextDocument = stored(&read, StepName::TextTypeset, None)?;
        let summary = visual.summary();
        let seconds: f64 = steps
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
        if localized_video_ran(steps) {
            let verified: VerifiedReplacements = stored(&read, StepName::TextVerify, None)?;
            let video: LocalizedVideoRecord = stored(&read, StepName::LocalizedVideo, None)?;
            text.push_str(&localized_lines(&verified.document, &video));
        }
    }
    work_dir::write_text(&work.report(), &text)?;
    Ok(qc)
}

/// `step`'s document `part` in `read`; missing is an error naming it.
fn stored<T>(read: &StoreRead, step: StepName, part: Option<&str>) -> Result<T>
where
    T: rkyv::Archive,
    T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
        + rkyv::Deserialize<T, HighDeserializer<ArchiveError>>,
{
    read.output(step, part)?.ok_or_else(|| {
        PipelineError::new(
            "the job report",
            format!(
                "the job database holds no {} document",
                keys::output_name(step, part)
            ),
        )
    })
}

/// Whether the localized-video step ran and wrote a video rather than recording itself disabled.
fn localized_video_ran(steps: &StepRecords) -> bool {
    steps
        .get(&StepName::LocalizedVideo)
        .is_some_and(|step| !step.measure.notes.contains_key("disabled"))
}

/// The report's localized-video lines: what was replaced in the picture, what was left in
/// Japanese, the file and encoder written, and how much of it was re-encoded and copied, or why
/// the whole video was re-encoded.
fn localized_lines(replacements: &ReplacementDocument, video: &LocalizedVideoRecord) -> String {
    let fallbacks = replacements
        .texts
        .iter()
        .filter(|text| matches!(text.status, ReplaceStatus::Fallback(_)))
        .count();
    let segments = &video.segments;
    let encoded = match &segments.fallback_reason {
        Some(reason) => format!(
            "- Whole video re-encoded ({} frames): {reason}\n",
            segments.frames_reencoded
        ),
        None => format!(
            "- Segments re-encoded: {} ({} frames); frames copied: {}\n",
            segments.segments_reencoded, segments.frames_reencoded, segments.frames_copied
        ),
    };
    format!(
        "\n### Localized video\n\n- Occurrences replaced in the video: {}\n- Occurrences left in Japanese: {}\n- Localized video: {}\n- Encoder: {}\n{encoded}",
        video.replaced,
        fallbacks,
        video.path.as_deref().unwrap_or("none"),
        video.encoder
    )
}

#[cfg(test)]
#[path = "tests/report.rs"]
mod tests;
