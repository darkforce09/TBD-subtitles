//! The localized-video task: blend every composed patch over the source frames and write the
//! result beside the source video, re-encoding only the segments that change when it can.
//!
//! **Role:** write `<video>.localized.mkv` and its record, with how much was re-encoded and
//! copied and why the whole video was, or record that the job writes none.
//! **Position:** pipeline task dispatch above `stages::localize`.
//! **Signals and state:** reads `outputs/text_verify` (the replacements the read-back check
//! approved), the probe, this step's previous record (`outputs/localized_video`) and every `frames`
//! row (each frame blends the patch of its own shift) through the step's `StepIo`, the segment
//! encoder setting, and the source video; writes the localized video through a part file, with
//! the pieces of a segment encode in `visual/localized_pieces/` until it ends, and stores its
//! record as `outputs/localized_video`.
//! **Invariants:** the source video is only read; a job without the localized video stores an
//! empty record and starts no encoder; a file at the output path is replaced only when this
//! job's previous record names it, as its current or earlier video; the output appears whole and
//! synced, by rename, before its record is handed to the store, or not at all.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use job_model::StepName;
use job_model::onscreen::{LocalizedVideoRecord, VerifiedReplacements};
use stages::localize::{self, RenderPhases, RenderRequest, Rendered};

use super::{Job, StepIo, StepProgress, TaskReport, since};
use crate::error::{Context, PipelineError, Result};
use crate::tasks::replace::{localized, motion};
use crate::work_dir::store::keys;

/// The job folder's work folder for the pieces of a segment encode, made and removed by the
/// render.
const PIECES_DIR: &str = "visual/localized_pieces";

pub(super) fn run(job: &Job, io: &mut StepIo, progress: StepProgress) -> Result<TaskReport> {
    let started = Instant::now();
    let mut report = TaskReport::default();
    let previous: Option<LocalizedVideoRecord> =
        io.read(&keys::output_address(StepName::LocalizedVideo, None))?;
    if !localized(job) {
        let record = LocalizedVideoRecord {
            earlier: previous.and_then(|record| record.path.or(record.earlier)),
            ..LocalizedVideoRecord::default()
        };
        io.put(StepName::LocalizedVideo, None, &record)?;
        report.note("disabled", true);
        return Ok(report);
    }
    let verified: VerifiedReplacements = io.get(StepName::TextVerify, None)?;
    let document = verified.document;
    document
        .validate()
        .map_err(|e| PipelineError::new("replacement document", e))?;
    let video = job.video();
    let output = stages::output::localized_video_path(&video);
    check_output(&video, &output, previous.as_ref())?;
    let probe = io.probe()?;
    let stream = probe
        .probe
        .video
        .as_ref()
        .ok_or_else(|| PipelineError::new("localized video", "the file has no video stream"))?;
    let motion = motion(io)?;
    let programs = media_io::Programs::beside_current_exe();
    let part = part_path(&output);
    let pieces = job.work.root().join(PIECES_DIR);
    let request = RenderRequest {
        programs: &programs,
        video: &video,
        stream,
        document: &document,
        motion: &motion,
        root: job.work.root(),
        output: &part,
        encoder: job.settings().onscreen_text.localized_encoder,
        pieces_dir: &pieces,
        cancel: None,
    };
    let rendered = match localize::render(&request, progress) {
        Ok(rendered) => rendered,
        Err(error) => {
            let _ = std::fs::remove_file(&part);
            let _ = std::fs::remove_dir_all(&pieces);
            return Err(PipelineError::new("write the localized video", error));
        }
    };
    install(&part, &output)?;
    let record = record_of(&rendered, &output, document.baked().count());
    io.put(StepName::LocalizedVideo, None, &record)?;
    report.process_s = since(started);
    note_render(&mut report, &record, &rendered.phases);
    Ok(report)
}

/// The record of a localized video written to `output` with `replaced` occurrences drawn in.
fn record_of(rendered: &Rendered, output: &Path, replaced: usize) -> LocalizedVideoRecord {
    LocalizedVideoRecord {
        path: Some(output.to_string_lossy().into_owned()),
        encoder: rendered.encoder.to_string(),
        frames: rendered.frames,
        replaced,
        earlier: None,
        segments: rendered.segments.clone(),
    }
}

/// The step's notes: the file, encoder, frames and replacements; how much was re-encoded and
/// copied, and why the whole video was; and the time of each phase.
fn note_render(report: &mut TaskReport, record: &LocalizedVideoRecord, phases: &RenderPhases) {
    report.note("path", record.path.as_deref().unwrap_or_default());
    report.note("encoder", &record.encoder);
    report.note("frames", record.frames);
    report.note("replaced", record.replaced);
    let segments = &record.segments;
    report.note("segments_reencoded", segments.segments_reencoded);
    report.note("frames_reencoded", segments.frames_reencoded);
    report.note("frames_copied", segments.frames_copied);
    if let Some(reason) = &segments.fallback_reason {
        report.note("fallback_reason", reason);
    }
    for (key, spent) in [
        ("decode_wait_s", phases.decode_wait),
        ("blend_s", phases.blend),
        ("encode_wait_s", phases.encode_wait),
        ("flush_s", phases.flush),
        ("plan_s", phases.plan),
        ("copy_s", phases.copy),
        ("join_s", phases.join),
        ("verify_s", phases.verify),
    ] {
        report.note(key, format!("{:.1}", spent.as_secs_f64()));
    }
}

/// Refuse an output that would overwrite the source, or a file at the output path that this
/// job's `previous` record names neither as its current nor as its earlier localized video.
fn check_output(
    video: &Path,
    output: &Path,
    previous: Option<&LocalizedVideoRecord>,
) -> Result<()> {
    if output == video {
        return Err(PipelineError::new(
            "localized video",
            "the video itself is named .localized.mkv",
        ));
    }
    let ours = previous.is_some_and(|record| {
        [&record.path, &record.earlier]
            .into_iter()
            .flatten()
            .any(|path| Path::new(path) == output)
    });
    if output.exists() && !ours {
        let name = output.file_name().map_or_else(
            || output.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        return Err(PipelineError::new(
            "localized video",
            format!(
                "{name} already exists and was not written by this job; move it away to write the localized video"
            ),
        ));
    }
    Ok(())
}

/// Sync the finished `part`, rename it to `output` and sync the folder, so the video is on disk
/// before the record that names it commits.
fn install(part: &Path, output: &Path) -> Result<()> {
    let synced = std::fs::File::open(part).and_then(|file| file.sync_all());
    if let Err(error) = synced {
        let _ = std::fs::remove_file(part);
        return Err(PipelineError::new(
            "write the localized video",
            format!("cannot sync {}: {error}", part.display()),
        ));
    }
    std::fs::rename(part, output)
        .context(format!("move the localized video to {}", output.display()))?;
    if let Some(folder) = output
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())
    {
        std::fs::File::open(folder)
            .and_then(|folder| folder.sync_all())
            .context(format!("sync {}", folder.display()))?;
    }
    Ok(())
}

/// `<output>.part`: where the encoder writes before the finished file is renamed into place.
fn part_path(output: &Path) -> PathBuf {
    let mut name = OsString::from(output.as_os_str());
    name.push(".part");
    PathBuf::from(name)
}

#[cfg(test)]
#[path = "tests/localized.rs"]
mod tests;
