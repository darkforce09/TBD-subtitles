//! The localized-video task: blend every composed patch over the source frames and encode the
//! result beside the source video.
//!
//! **Role:** write `<video>.localized.mkv` and its record, or record that the job writes none.
//! **Position:** pipeline task dispatch above `stages::localize`.
//! **Signals and state:** reads `visual/text_verify.json` (the replacements the read-back check approved), the probe and the source video, and
//! this step's previous record; writes the localized video through a part file and
//! `visual/localized_video.json`.
//! **Invariants:** the source video is only read; a job without the localized video writes an
//! empty record and starts no encoder; a file at the output path is replaced only when this
//! job's previous record names it, as its current or earlier video; the output appears whole, by rename, or not at all.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use job_model::StepName;
use job_model::onscreen::{LocalizedVideoRecord, ReplacementDocument};
use stages::localize::{self, RenderRequest};

use super::{Job, StepProgress, TaskReport, since};
use crate::error::{Context, PipelineError, Result};
use crate::tasks::replace::localized;
use crate::work_dir;

pub(super) fn run(job: &Job, progress: StepProgress) -> Result<TaskReport> {
    let started = Instant::now();
    let mut report = TaskReport::default();
    let record_path = job.work.text(StepName::LocalizedVideo);
    let previous: Option<LocalizedVideoRecord> = work_dir::read_json(&record_path).ok();
    if !localized(job) {
        let record = LocalizedVideoRecord {
            earlier: previous.and_then(|record| record.path.or(record.earlier)),
            ..LocalizedVideoRecord::default()
        };
        work_dir::write_json(&record_path, &record)?;
        report.note("disabled", true);
        return Ok(report);
    }
    let document: ReplacementDocument = work_dir::read_json(&job.work.text(StepName::TextVerify))?;
    document
        .validate()
        .map_err(|e| PipelineError::new("replacement document", e))?;
    let video = job.video();
    let output = stages::output::localized_video_path(&video);
    check_output(&video, &output, previous.as_ref())?;
    let probe = job.probe()?;
    let stream = probe
        .probe
        .video
        .as_ref()
        .ok_or_else(|| PipelineError::new("localized video", "the file has no video stream"))?;
    let programs = media_io::Programs::beside_current_exe();
    let part = part_path(&output);
    let request = RenderRequest {
        programs: &programs,
        video: &video,
        stream,
        document: &document,
        root: job.work.root(),
        output: &part,
        cancel: None,
    };
    let rendered = match localize::render(&request, progress) {
        Ok(rendered) => rendered,
        Err(error) => {
            let _ = std::fs::remove_file(&part);
            return Err(PipelineError::new("write the localized video", error));
        }
    };
    std::fs::rename(&part, &output)
        .context(format!("move the localized video to {}", output.display()))?;
    let record = LocalizedVideoRecord {
        path: Some(output.to_string_lossy().into_owned()),
        encoder: rendered.encoder.name().to_string(),
        frames: rendered.frames,
        replaced: document.baked().count(),
        earlier: None,
    };
    work_dir::write_json(&record_path, &record)?;
    report.process_s = since(started);
    report.note("path", record.path.as_deref().unwrap_or_default());
    report.note("encoder", &record.encoder);
    report.note("frames", record.frames);
    report.note("replaced", record.replaced);
    Ok(report)
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

/// `<output>.part`: where the encoder writes before the finished file is renamed into place.
fn part_path(output: &Path) -> PathBuf {
    let mut name = OsString::from(output.as_os_str());
    name.push(".part");
    PathBuf::from(name)
}

#[cfg(test)]
#[path = "tests/localized.rs"]
mod tests;
