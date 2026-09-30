//! Tasks for in-place replacement: stroke masks, inpainting and lettering composition.
//!
//! **Role:** run each replacement step over the reviewed text document and install its
//! `ReplacementDocument`, loading the inpainting model only in its ONNX Runtime worker.
//! **Position:** pipeline task dispatch above `stages::onscreen_text::replace`.
//! **Signals and state:** `visual/text_mask.json`, `visual/text_inpaint.json`,
//! `visual/text_compose.json` and the PNG files they name.
//! **Invariants:** a job without the localized video writes empty documents and loads nothing;
//! each document is written atomically after its step's files exist.

use std::time::Instant;

use inference::onnx::{Device, lama};
use job_model::StepName;
use job_model::onscreen::{ReplaceStatus, ReplacementDocument, TextDocument};
use stages::onscreen_text::TextResult;
use stages::onscreen_text::replace::{self, inpaint::Inpaint};

use super::{Job, StepProgress, TaskReport, since};
use crate::error::{Context, PipelineError, Result};
use crate::work_dir;

/// Whether the job replaces writing in a localized video.
pub(crate) fn localized(job: &Job) -> bool {
    let text = &job.settings().onscreen_text;
    text.enabled && text.localized_video
}

pub(super) fn run(step: StepName, job: &Job, progress: StepProgress) -> Result<TaskReport> {
    let started = Instant::now();
    let mut report = TaskReport::default();
    if !localized(job) {
        work_dir::write_json(&job.work.text(step), &ReplacementDocument::default())?;
        report.note("disabled", true);
        return Ok(report);
    }
    let reviewed: TextDocument = work_dir::read_json(&job.work.text(StepName::TextReview))?;
    let document = match step {
        StepName::TextMask => {
            let probe = job.probe()?;
            let stream = probe.probe.video.as_ref().ok_or_else(|| {
                PipelineError::new("stroke masks", "the file has no video stream")
            })?;
            let programs = media_io::Programs::beside_current_exe();
            let mut source = replace::source::FfmpegRegions::open(&programs, &job.video(), stream)
                .context("read the video's frame timeline")?;
            replace::mask::extract(&reviewed, &mut source, job.work.root(), progress)
                .context("measure the strokes of visible writing")?
        }
        StepName::TextInpaint => {
            let mut document: ReplacementDocument =
                work_dir::read_json(&job.work.text(StepName::TextMask))?;
            let mut model = LamaModel(
                lama::Lama::open(&job.models()?.join(lama::MODEL), Device::Cuda)
                    .context("open LaMa; download the inpainting model in Settings")?,
            );
            report.load_s = since(started);
            replace::inpaint::inpaint(&mut document, job.work.root(), &mut model, progress)
                .context("fill erased writing")?;
            document
        }
        StepName::TextCompose => {
            let mut document: ReplacementDocument =
                work_dir::read_json(&job.work.text(StepName::TextInpaint))?;
            let fonts = job.models()?.join("latin-fonts");
            replace::compose::compose(&mut document, &reviewed, job.work.root(), &fonts, progress)
                .context("letter English onto filled backgrounds")?;
            document
        }
        _ => return Err(PipelineError::new("replacement step", "unexpected task")),
    };
    document
        .validate()
        .map_err(|e| PipelineError::new("replacement document", e))?;
    report.process_s = (since(started) - report.load_s).max(0.0);
    let baked = document.baked().count();
    let fallback = document
        .texts
        .iter()
        .filter(|text| matches!(text.status, ReplaceStatus::Fallback(_)))
        .count();
    report.note("occurrences", document.texts.len());
    report.note("replaced", baked);
    report.note("fallback", fallback);
    report.note(
        "plates",
        document.texts.iter().map(|t| t.plates.len()).sum::<usize>(),
    );
    work_dir::write_json(&job.work.text(step), &document)?;
    Ok(report)
}

/// The LaMa backend as the inpainting stage sees it.
struct LamaModel(lama::Lama);

impl Inpaint for LamaModel {
    fn side(&self) -> usize {
        lama::SIDE
    }

    fn inpaint(&mut self, rgb: &[u8], mask: &[u8]) -> TextResult<Vec<u8>> {
        self.0
            .inpaint(rgb, mask)
            .map_err(|e| format!("{}: {}", e.context, e.message).into())
    }
}
