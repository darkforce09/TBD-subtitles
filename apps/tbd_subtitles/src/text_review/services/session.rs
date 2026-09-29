//! Loading visual review state and persisting the owner's corrections.
//!
//! **Role:** join visual results with the job, probe, output and current corrections.
//! **Position:** called off the window thread by application actions; depends on plain models.
//! **Signals and state:** reads job files and crops; locks and atomically writes visual corrections.
//! **Invariants:** existing corrections are reread under lock; thumbnails have bounded total size;
//! absent visual results never masquerade as a completed scan.

use std::fs::OpenOptions;
use std::path::{Component, Path};

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::{TextCorrections, TextDocument, TextEdit};
use job_model::outputs::{OutputRecord, ProbeDecoded};
use pipeline::work_dir::{self, WorkDir};

use super::player;
use crate::text_review::models::{Event, Picture, Session};

const THUMBNAIL_EDGE: u32 = 128;
const THUMBNAIL_BUDGET: usize = 32 * 1024 * 1024;

/// Read one job's visual results, with bounded representative thumbnails.
pub(crate) fn load(work: &Path) -> Result<Session, String> {
    let work_dir = WorkDir::new(work);
    let path = [StepName::TextTypeset, StepName::TextReview]
        .into_iter()
        .map(|step| work_dir.text(step))
        .find(|path| path.is_file())
        .ok_or_else(|| "This job has no on-screen text results. Enable on-screen translation in Settings and run the video again.".to_string())?;
    let document: TextDocument = read(&path)?;
    let job: JobRecord = read(&work_dir.job_json())?;
    let probe: ProbeDecoded = read(&work_dir.probe())?;
    let output: OutputRecord = read(&work_dir.output_record()).map_err(|error| {
        format!("The exported subtitle record is unavailable. Finish or rerun this video to generate its combined ASS file. {error}")
    })?;
    if !Path::new(&output.path).is_file() {
        return Err(format!(
            "The exported ASS file is missing: {}. Run this video again to regenerate it.",
            output.path
        ));
    }
    let video = probe
        .probe
        .video
        .as_ref()
        .ok_or("This job has no video picture.")?;
    let fps = video
        .fps()
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .ok_or("This video's frame rate is unavailable.")?;
    if !probe.probe.duration_s.is_finite() || probe.probe.duration_s <= 0.0 {
        return Err("This video's duration is unavailable.".into());
    }
    let corrections = corrections(&work_dir)?;
    let selected = document
        .occurrences
        .iter()
        .position(|text| !text.reviewed && (!text.warnings.is_empty() || text.english.is_none()))
        .unwrap_or(0);
    let draft = document.occurrences.get(selected).map(|text| {
        corrections
            .edits
            .get(&text.id)
            .filter(|edit| edit.matches_source(text))
            .cloned()
            .unwrap_or_else(|| TextEdit::from_occurrence(text))
    });
    let position_s = draft.as_ref().map_or(0.0, |draft| draft.start_s);
    let mut held = 0;
    let thumbnails = document
        .occurrences
        .iter()
        .map(|text| {
            if held + (THUMBNAIL_EDGE * THUMBNAIL_EDGE * 3) as usize > THUMBNAIL_BUDGET {
                return None;
            }
            let thumbnail = text.crops.iter().find_map(|path| thumbnail(work, path));
            if let Some(picture) = &thumbnail {
                held += picture.rgb.len();
            }
            thumbnail
        })
        .collect();
    Ok(Session {
        work: work.into(),
        video: job.video.into(),
        ass: output.path.into(),
        flagged_only: document.summary().flagged > 0,
        document,
        corrections,
        duration_s: probe.probe.duration_s,
        fps,
        audio_position: probe.track.audio_position,
        selected,
        draft,
        position_s,
        error: None,
        thumbnails,
    })
}

/// Save, keep, undo or request another reading of the selected occurrence.
pub(crate) fn save(session: &Session, event: &Event) -> Result<(), String> {
    let selected = session.document.occurrences.get(session.selected);
    let edit = match event {
        Event::Save => {
            let selected = selected.ok_or("Select an occurrence before changing it.")?;
            let edit = session
                .draft
                .clone()
                .unwrap_or_else(|| TextEdit::from_occurrence(selected));
            if !edit.matches_source(selected) {
                return Err("This correction does not match the current source text or has no source identity. Reload Check Text and enter the correction again. If the source identity is still unavailable, run the video again before saving.".into());
            }
            edit.validate(session.duration_s)?;
            if edit.presentation.anchor.is_some_and(|anchor| {
                !(0.0..=f64::from(session.document.width)).contains(&anchor.x)
                    || !(0.0..=f64::from(session.document.height)).contains(&anchor.y)
            }) {
                return Err("Text position must be inside the video picture.".into());
            }
            Some(edit)
        }
        Event::Undo | Event::Retry => {
            selected.ok_or("Select an occurrence before changing it.")?;
            None
        }
        Event::DiscardOrphans => None,
        _ => return Err("This action does not save an on-screen text correction.".into()),
    };
    let work = WorkDir::new(&session.work);
    let path = work.text_corrections();
    let parent = path
        .parent()
        .ok_or("The corrections folder is unavailable.")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let lock_path = path.with_extension("json.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|error| format!("Cannot open {}: {error}", lock_path.display()))?;
    lock.lock()
        .map_err(|error| format!("Cannot lock {}: {error}", lock_path.display()))?;
    let mut current = corrections(&work)?;
    match event {
        Event::Save => {
            current.edits.insert(
                selected.expect("validated selection").id.clone(),
                edit.expect("save has an edit"),
            );
        }
        Event::Undo => {
            current
                .edits
                .remove(&selected.expect("validated selection").id);
        }
        // Each request changes the fingerprint, including another retry of the same occurrence.
        Event::Retry => current
            .retry
            .push(selected.expect("validated selection").id.clone()),
        Event::DiscardOrphans => current.edits.retain(|id, _| {
            session
                .document
                .occurrences
                .iter()
                .any(|text| &text.id == id)
        }),
        _ => unreachable!("validated correction action"),
    }
    work_dir::write_json(&path, &current).map_err(|error| error.to_string())?;
    drop(lock);
    Ok(())
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    work_dir::read_json(path).map_err(|error| error.to_string())
}

fn corrections(work: &WorkDir) -> Result<TextCorrections, String> {
    match std::fs::read(work.text_corrections()) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("Cannot read visual corrections: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(TextCorrections::default())
        }
        Err(error) => Err(format!("Cannot read visual corrections: {error}")),
    }
}

fn thumbnail(work: &Path, relative: &Path) -> Option<Picture> {
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let mut reader = image::ImageReader::open(work.join(relative)).ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let image = reader
        .decode()
        .ok()?
        .thumbnail(THUMBNAIL_EDGE, THUMBNAIL_EDGE)
        .to_rgb8();
    Some(player::picture(
        image.width(),
        image.height(),
        image.into_raw(),
    ))
}

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;
