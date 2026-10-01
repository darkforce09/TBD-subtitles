//! Tasks for in-place replacement: stroke masks, inpainting and lettering composition.
//!
//! **Role:** run each replacement step over the reviewed text document and store its
//! `ReplacementDocument`, loading the inpainting model only in its ONNX Runtime worker.
//! **Position:** pipeline task dispatch above `stages::onscreen_text::replace`; every stored
//! input and output goes through the step's `StepIo`.
//! **Signals and state:** reads `outputs/text_review`, the probe and the previous replacement
//! step's document, and composition every `frames` row; stores `outputs/text_mask`,
//! `outputs/text_inpaint` or `outputs/text_compose`, and the stroke masks one `frames` row per
//! frame of every occurrence that keeps its plates, each sent as it is made; writes the mask,
//! source, plate, patch and preview PNGs those documents name; the composition
//! starts each occurrence the sign library holds for another job from that sign's lettering
//! style.
//! **Invariants:** a job without the localized video stores empty documents and loads nothing;
//! every PNG a document names is synced before the document is handed to the store.

use std::time::Instant;

use inference::onnx::{Device, lama};
use job_model::StepName;
use job_model::onscreen::{FrameRecord, ReplaceStatus, ReplacementDocument, TextDocument};
use stages::localize::motion::Motion;
use stages::onscreen_text::TextResult;
use stages::onscreen_text::replace::{self, inpaint::Inpaint};
use worker_channel::address::Table;

use super::{Job, StepIo, StepProgress, TaskReport, since};
use crate::error::{Context, PipelineError, Result};
use crate::library::signs;

/// Whether the job replaces writing in a localized video.
pub(crate) fn localized(job: &Job) -> bool {
    let text = &job.settings().onscreen_text;
    text.enabled && text.localized_video
}

pub(super) fn run(
    step: StepName,
    job: &Job,
    io: &mut StepIo,
    progress: StepProgress,
) -> Result<TaskReport> {
    let started = Instant::now();
    let mut report = TaskReport::default();
    if !localized(job) {
        io.put(step, None, &ReplacementDocument::default())?;
        report.note("disabled", true);
        return Ok(report);
    }
    let document = match step {
        StepName::TextMask => {
            let reviewed: TextDocument = io.get(StepName::TextReview, None)?;
            let probe = io.probe()?;
            let stream = probe.probe.video.as_ref().ok_or_else(|| {
                PipelineError::new("stroke masks", "the file has no video stream")
            })?;
            let programs = media_io::Programs::beside_current_exe();
            let mut source = replace::source::FfmpegRegions::open(&programs, &job.video(), stream)
                .context("read the video's frame timeline")?;
            let mut rows = 0u64;
            let mut put = |id: &str, frame: u64, record: &FrameRecord| {
                rows += 1;
                io.put_frame(Table::Frames, id, frame, record)
                    .map_err(|error| error.to_string().into())
            };
            let document =
                replace::mask::extract(&reviewed, &mut source, job.work.root(), &mut put, progress)
                    .context("measure the strokes of visible writing")?;
            report.note("frame_rows", rows);
            document
        }
        StepName::TextInpaint => {
            let mut document: ReplacementDocument = io.get(StepName::TextMask, None)?;
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
            let reviewed: TextDocument = io.get(StepName::TextReview, None)?;
            let mut document: ReplacementDocument = io.get(StepName::TextInpaint, None)?;
            if let Some(library) = &job.library {
                let known = signs::matches(library, &reviewed, job.work.root(), &job.id())?;
                signs::start_from_styles(&mut document, &known);
                if !known.is_empty() {
                    report.note("library_styles", known.len());
                }
            }
            let fonts = job.models()?.join("latin-fonts");
            let motion = motion(io)?;
            replace::compose::compose(
                &mut document,
                &reviewed,
                job.work.root(),
                &fonts,
                &motion,
                progress,
            )
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
    io.put(step, None, &document)?;
    Ok(report)
}

/// The writing's shift in each frame, folded from every `frames` row, one row at a time.
pub(crate) fn motion(io: &mut StepIo) -> Result<Motion> {
    let mut motion = Motion::default();
    io.frame_rows::<FrameRecord>(Table::Frames, |occurrence, frame, row| {
        let shift = [row.shift[0].to_native(), row.shift[1].to_native()];
        motion.add(occurrence, frame, row.plate.to_native(), shift);
        Ok(())
    })?;
    Ok(motion)
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
