//! The read-back check task: approve each lettered replacement from its finished picture.
//!
//! **Role:** read the composed replacements, let a local OCR read each baked one back off the
//! finished frames, and install `visual/text_verify.json` with the final statuses and readings.
//! **Position:** pipeline task dispatch above `stages::onscreen_text::replace::verify`; runs in a
//! `tbd-subtitles` ONNX Runtime worker under the GPU lock, PP-OCRv5 on CUDA.
//! **Signals and state:** reads `visual/text_compose.json`, `visual/text_review.json`, the probe,
//! the patch files and the source video; writes `visual/text_verify.json`.
//! **Invariants:** a job without the localized video writes an empty document and loads nothing;
//! a document with nothing baked is passed on unchanged without loading a model; the document is
//! written atomically.

use std::time::Instant;

use job_model::StepName;
use job_model::onscreen::{ReplaceStatus, ReplacementDocument, TextDocument, VerifiedReplacements};
use stages::localize::{colour::Conversion, frame_format};
use stages::onscreen_text::replace::source::FfmpegRegions;
use stages::onscreen_text::replace::verify::{self, LocalOcr, Request, verdict};

use super::{Job, StepProgress, TaskReport, since};
use crate::error::{Context, PipelineError, Result};
use crate::tasks::replace::localized;
use crate::work_dir;

pub(super) fn run(job: &Job, progress: StepProgress) -> Result<TaskReport> {
    let started = Instant::now();
    let mut report = TaskReport::default();
    let output = job.work.text(StepName::TextVerify);
    if !localized(job) {
        work_dir::write_json(&output, &VerifiedReplacements::default())?;
        report.note("disabled", true);
        return Ok(report);
    }
    let composed: ReplacementDocument = work_dir::read_json(&job.work.text(StepName::TextCompose))?;
    let baked = composed.baked().count();
    report.note("baked", baked);
    let verified = if baked == 0 {
        VerifiedReplacements {
            document: composed,
            checks: Vec::new(),
        }
    } else {
        let text: TextDocument = work_dir::read_json(&job.work.text(StepName::TextReview))?;
        let probe = job.probe()?;
        let stream =
            probe.probe.video.as_ref().ok_or_else(|| {
                PipelineError::new("read-back check", "the file has no video stream")
            })?;
        let mut ocr = LocalOcr::open(&job.models()?)
            .context("open PP-OCRv5; download visual models in Settings")?;
        report.load_s = since(started);
        let programs = media_io::Programs::beside_current_exe();
        let mut source = FfmpegRegions::open(&programs, &job.video(), stream)
            .context("read the video's frame timeline")?;
        let request = Request {
            composed: &composed,
            text: &text,
            root: job.work.root(),
            conversion: Conversion::of(stream, frame_format(stream)),
            only: None,
        };
        verify::verify(&request, &mut source, &mut ocr, &mut |_, _| {}, progress)
            .context("read the lettered English back")?
    };
    verified
        .document
        .validate()
        .map_err(|e| PipelineError::new("replacement document", e))?;
    let reason = |wanted: &str| {
        verified
            .document
            .texts
            .iter()
            .filter(|text| matches!(&text.status, ReplaceStatus::Fallback(r) if r == wanted))
            .count()
    };
    report.note("approved", verified.document.baked().count());
    report.note("japanese_left", reason(verdict::JAPANESE_LEFT));
    report.note("unreadable", reason(verdict::UNREADABLE));
    report.note(
        "samples",
        verified
            .checks
            .iter()
            .map(|check| check.readings.len())
            .sum::<usize>(),
    );
    report.process_s = (since(started) - report.load_s).max(0.0);
    work_dir::write_json(&output, &verified)?;
    Ok(report)
}
